/// Returns `true` when a repo-relative path belongs to a file that can never affect
/// compilation, test execution, or runtime behaviour, regardless of its content.
///
/// Changes to inert paths produce no `ChangedElement` and never trigger a conservative
/// fallback.  The criteria are:
///
/// * **Documentation extensions** — `.md`, `.rst`, `.adoc` are never loaded by a build
///   tool or runtime.
/// * **Known-inert stems** — `LICENSE`, `NOTICE`, `CHANGELOG`, `AUTHORS`, `CONTRIBUTORS`
///   (with any extension, e.g. `LICENSE.txt`).
/// * **Known-inert dotfiles** — `.gitignore`, `.gitattributes`, `.mailmap`, `CODEOWNERS`,
///   `.editorconfig`.
/// * **AI/agent tooling directories** — `.claude/`, `.codex/`, `.cursor/`, `.amazonq/`,
///   `.codeium/`, `.continue/`, `.aider/`, `.mentat/`.  These directories may contain
///   `.py`, `.ts`, or `.js` files that are development helpers, not production code.
/// * **IDE directories** — `.idea/`, `.vscode/`.
/// * **GitHub CI/meta** — `.github/`.  Workflow YAML and repo-management files here are
///   never loaded by the application test runtime.
pub(super) fn is_inert_path(path: &str) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let (stem, ext) = name
        .rsplit_once('.')
        .map(|(s, e)| (s, e))
        .unwrap_or((name, ""));

    if matches!(ext, "md" | "rst" | "adoc") {
        return true;
    }

    if matches!(
        stem,
        "LICENSE" | "NOTICE" | "CHANGELOG" | "AUTHORS" | "CONTRIBUTORS"
    ) {
        return true;
    }

    if matches!(
        name,
        ".gitignore" | ".gitattributes" | ".mailmap" | "CODEOWNERS" | ".editorconfig"
    ) {
        return true;
    }

    [
        ".claude/",
        ".codex/",
        ".cursor/",
        ".amazonq/",
        ".codeium/",
        ".continue/",
        ".aider/",
        ".mentat/",
        ".idea/",
        ".vscode/",
        ".github/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentation_extensions_are_inert() {
        assert!(is_inert_path("README.md"));
        assert!(is_inert_path("docs/ARCHITECTURE.md"));
        assert!(is_inert_path("orders/docs/api.rst"));
        assert!(is_inert_path("spec.adoc"));
    }

    #[test]
    fn known_inert_stems_are_inert_regardless_of_extension() {
        assert!(is_inert_path("LICENSE"));
        assert!(is_inert_path("LICENSE.txt"));
        assert!(is_inert_path("NOTICE"));
        assert!(is_inert_path("NOTICE.md"));
        assert!(is_inert_path("CHANGELOG"));
        assert!(is_inert_path("CHANGELOG.md"));
        assert!(is_inert_path("AUTHORS"));
        assert!(is_inert_path("CONTRIBUTORS"));
    }

    #[test]
    fn known_inert_dotfiles_are_inert() {
        assert!(is_inert_path(".gitignore"));
        assert!(is_inert_path(".gitattributes"));
        assert!(is_inert_path(".mailmap"));
        assert!(is_inert_path("CODEOWNERS"));
        assert!(is_inert_path(".editorconfig"));
    }

    #[test]
    fn ai_tooling_directories_are_inert_regardless_of_extension() {
        // .py files in .claude/ are hooks/skills, not production code
        assert!(is_inert_path(".claude/agents/researcher.md"));
        assert!(is_inert_path(".claude/hooks/pre-tool.py"));
        assert!(is_inert_path(".claude/settings.json"));
        assert!(is_inert_path(".codex/config.json"));
        assert!(is_inert_path(".cursor/rules/style.mdc"));
        assert!(is_inert_path(".amazonq/dev-agent.json"));
        assert!(is_inert_path(".codeium/config.yaml"));
        assert!(is_inert_path(".continue/config.ts"));
        assert!(is_inert_path(".aider/.aider.conf.yml"));
        assert!(is_inert_path(".mentat/config.yaml"));
    }

    #[test]
    fn ide_and_github_directories_are_inert() {
        assert!(is_inert_path(".idea/workspace.xml"));
        assert!(is_inert_path(".vscode/settings.json"));
        assert!(is_inert_path(".github/workflows/ci.yml"));
        assert!(is_inert_path(".github/CODEOWNERS"));
        assert!(is_inert_path(".github/pull_request_template.md"));
    }

    #[test]
    fn source_and_build_files_are_not_inert() {
        assert!(!is_inert_path("src/main/java/Service.java"));
        assert!(!is_inert_path("service/app.py"));
        assert!(!is_inert_path("main.go"));
        assert!(!is_inert_path("pom.xml"));
        assert!(!is_inert_path("build.gradle"));
        assert!(!is_inert_path("settings.gradle"));
        assert!(!is_inert_path("src/main/resources/application.yml"));
        assert!(!is_inert_path("src/main/resources/application.properties"));
    }

    #[test]
    fn ambiguous_extensions_are_not_inert() {
        // requirements.txt is a Python dependency spec
        assert!(!is_inert_path("requirements.txt"));
        // config YAML/JSON may be loaded at runtime
        assert!(!is_inert_path("config.yaml"));
        assert!(!is_inert_path("data.json"));
        assert!(!is_inert_path("schema.sql"));
        // scripts/ outside known-inert prefixes is not automatically inert
        assert!(!is_inert_path("scripts/helper.py"));
        assert!(!is_inert_path("tools/build.sh"));
    }
}
