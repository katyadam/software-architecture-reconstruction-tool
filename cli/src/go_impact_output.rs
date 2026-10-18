//! Atomic Go report publishing; never write through a report symlink.
use anyhow::{Context, Result, bail};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub fn validate_output_path(project_root: &Path, output: &Path) -> Result<PathBuf> {
    let project = project_root.canonicalize()?;
    let absolute = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    let parent = absolute
        .parent()
        .context("output needs a parent")?
        .canonicalize()
        .context("output parent must already exist")?;
    if parent.starts_with(project) {
        bail!("output must be outside the analyzed checkout")
    }
    // Fail before expensive analysis if the destination directory cannot host
    // the temporary file used by atomic replacement.
    drop(tempfile::NamedTempFile::new_in(&parent).context("output parent is not writable")?);
    let path = parent.join(absolute.file_name().context("output needs a filename")?);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if !meta.file_type().is_file() => {
            bail!("output must not be a symlink or nonregular file")
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    Ok(path)
}
pub fn write_report_atomic<T: serde::Serialize>(output: &Path, report: &T) -> Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(output) {
        if !meta.file_type().is_file() {
            bail!("output became a symlink or nonregular file")
        }
    }
    let mut temp = tempfile::NamedTempFile::new_in(output.parent().context("output parent")?)?;
    serde_json::to_writer_pretty(temp.as_file_mut(), report)?;
    writeln!(temp.as_file_mut())?;
    temp.as_file_mut().sync_all()?;
    temp.persist(output)
        .map_err(|e| e.error)
        .context("unable to atomically replace report")?;
    Ok(())
}
