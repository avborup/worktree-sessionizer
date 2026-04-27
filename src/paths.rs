use std::path::PathBuf;

use anyhow::{Context, Result};

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "wt")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".wt"))
}

pub fn atomic_write(path: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let dir = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("creating temp file in {}", dir.display()))?;
    std::io::Write::write_all(&mut tmp, contents.as_bytes())?;
    tmp.persist(path)
        .with_context(|| format!("persisting to {}", path.display()))?;
    Ok(())
}
