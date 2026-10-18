use super::*;
use models::go_impact::*;
use std::io::Write;

impl GoNativeProvider {
    pub(super) fn metadata(
        &self,
        mode: &str,
        root: Option<&Path>,
        input: Option<Vec<u8>>,
    ) -> Result<Output, String> {
        if !cfg!(unix) || self.timeout.is_zero() {
            return Err("native metadata needs Unix supervision and a positive timeout".into());
        }
        let mut command = Command::new(&self.executable);
        command.args(["--mode", mode, "--timeout", &duration_flag(self.timeout)]);
        if let Some(root) = root {
            command.arg("--source-dir").arg(root);
        }
        if !self.build_tags.is_empty() {
            command.args(["--tags", &self.build_tags]);
        }
        command
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_process_group(&mut command);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let writer = input.map(|data| {
            let mut stdin = child.stdin.take().expect("piped input");
            std::thread::spawn(move || stdin.write_all(&data))
        });
        let output = wait_for_output(child, self.timeout, PROVIDER_ID);
        if let Some(writer) = writer {
            let result = writer
                .join()
                .map_err(|_| "metadata stdin writer panicked".to_string())?;
            if output.is_ok() {
                result.map_err(|e| e.to_string())?;
            }
        }
        output
    }

    pub fn discover(&self, root: &Path) -> GoInventoryOutcome {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let failed = |message: String| GoInventoryOutcome {
            schema_version: 1,
            provider_id: PROVIDER_ID.into(),
            status: GoStageStatus::Failed,
            source_root: root.to_string_lossy().into(),
            source_files: vec![],
            tests: vec![],
            declarations: vec![],
            test_inventory_complete: false,
            scope: None,
            diagnostics: vec![message],
        };
        let output = match self.metadata("inventory", Some(&root), None) {
            Ok(o) => o,
            Err(e) => return failed(format!("Go inventory: {e}")),
        };
        let parsed = serde_json::from_slice::<GoInventoryOutcome>(&output.stdout)
            .map_err(|e| e.to_string())
            .and_then(|out| {
                self.validate_inventory(&root, &out)?;
                if !output.status.success() && out.status != GoStageStatus::Failed {
                    return Err("inventory process failed despite successful JSON".into());
                }
                Ok(out)
            });
        match parsed {
            Ok(out) => out,
            Err(e) => failed(format!(
                "Go inventory protocol: {e}; stderr: {}",
                diagnostic_tail(&String::from_utf8_lossy(&output.stderr))
            )),
        }
    }

    fn validate_inventory(&self, root: &Path, out: &GoInventoryOutcome) -> Result<(), String> {
        if out.schema_version != 1
            || out.provider_id != PROVIDER_ID
            || Path::new(&out.source_root).canonicalize().ok().as_deref() != Some(root)
        {
            return Err("wrong inventory provider/version/root".into());
        }
        if out.status == GoStageStatus::Failed {
            if out.test_inventory_complete || !out.tests.is_empty() || !out.declarations.is_empty()
            {
                return Err("failed inventory claimed complete data".into());
            }
            return Ok(());
        }
        if out.status != GoStageStatus::Ok || !out.test_inventory_complete {
            return Err("inventory is not complete".into());
        }
        let scope = out.scope.as_ref().ok_or("missing inventory scope")?;
        if scope.build_tags != self.build_tags
            || scope.workspace_mode != "off"
            || scope.goos.is_empty()
            || scope.goarch.is_empty()
            || scope.go_version.is_empty()
        {
            return Err("inventory build scope mismatch".into());
        }
        let mut files = BTreeSet::new();
        for path in &out.source_files {
            let p = Path::new(path);
            if !p.is_absolute()
                || !p.is_file()
                || p.canonicalize().ok().is_none_or(|p| !p.starts_with(root))
                || !files.insert(path)
            {
                return Err("foreign/duplicate inventory source file".into());
            }
        }
        let check = |m: &MethodRef| -> Result<(), String> {
            if m.language != Language::Go
                || m.member_name.is_empty()
                || m.declaring_type.is_empty()
                || !m.descriptor.starts_with("func")
                || m.source_line.is_none_or(|n| n == 0)
                || m.source_path.as_ref().is_none_or(|p| !files.contains(p))
            {
                return Err("invalid native inventory identity".into());
            }
            Ok(())
        };
        let mut tests = vec![];
        for test in &out.tests {
            check(&test.method)?;
            if test.execution_package.is_empty() || tests.contains(&test.method) {
                return Err("invalid/duplicate test inventory".into());
            }
            tests.push(test.method.clone());
        }
        let mut declarations = vec![];
        for d in &out.declarations {
            check(&d.method)?;
            if !valid_span(&d.syntax) || declarations.contains(&d.method) {
                return Err("invalid/duplicate source declaration".into());
            }
            declarations.push(d.method.clone());
        }
        Ok(())
    }

    pub fn describe_sources(&self, files: &[GoSourceInput]) -> GoDescriptionOutcome {
        let failed = |message: String| GoDescriptionOutcome {
            schema_version: 1,
            provider_id: PROVIDER_ID.into(),
            status: GoStageStatus::Failed,
            files: vec![],
            diagnostics: vec![message],
        };
        let input = match serde_json::to_vec(&serde_json::json!({"files":files})) {
            Ok(v) => v,
            Err(e) => return failed(e.to_string()),
        };
        let output = match self.metadata("describe", None, Some(input)) {
            Ok(o) => o,
            Err(e) => return failed(format!("Go source description: {e}")),
        };
        let parsed = serde_json::from_slice::<GoDescriptionOutcome>(&output.stdout)
            .map_err(|e| e.to_string())
            .and_then(|out| {
                if out.schema_version != 1 || out.provider_id != PROVIDER_ID {
                    return Err("wrong description provider/version".into());
                }
                if out.status == GoStageStatus::Failed {
                    return Ok(out);
                }
                let requested: BTreeSet<_> = files.iter().map(|f| &f.source_path).collect();
                let mut returned = BTreeSet::new();
                for item in &out.files {
                    if !requested.contains(&item.source_path)
                        || !returned.insert(&item.source_path)
                        || item.declarations.iter().any(|d| !valid_span(d))
                    {
                        return Err("invalid source-description paths/spans".into());
                    }
                }
                if !output.status.success()
                    || out.status != GoStageStatus::Ok
                    || returned != requested
                {
                    return Err("incomplete source-description response".into());
                }
                Ok(out)
            });
        parsed.unwrap_or_else(|e| failed(format!("Go source description protocol: {e}")))
    }
}
fn valid_span(d: &GoSourceDeclaration) -> bool {
    !d.header.is_empty()
        && !d.member_name.is_empty()
        && d.start_line > 0
        && d.end_line >= d.start_line
        && ((d.body_start_line == 0 && d.body_end_line == 0)
            || (d.body_start_line >= d.start_line
                && d.body_end_line >= d.body_start_line
                && d.body_end_line <= d.end_line))
}
