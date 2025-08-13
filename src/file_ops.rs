use crate::discovery::resolve_under_root;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct EditPreview {
    pub before_preview: String,
    pub after_preview: String,
}

#[derive(Debug, Serialize)]
pub struct OperationResult<T> {
    pub applied: bool,
    pub output: T,
}

fn cap_utf8(mut bytes: Vec<u8>, max_bytes: usize) -> String {
    if bytes.len() > max_bytes { bytes.truncate(max_bytes); }
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn write_file_under_root(
    root: &str,
    rel: &str,
    content: &str,
    create: bool,
    dry_run: bool,
    preview_bytes: usize,
) -> anyhow::Result<OperationResult<EditPreview>> {
    let path = resolve_under_root(root, rel).ok_or_else(|| anyhow::anyhow!("path outside root"))?;

    let existed = path.exists();
    if !existed && !create {
        return Err(anyhow::anyhow!("file does not exist (use create=true to create)"));
    }

    let mut before_bytes = Vec::new();
    if existed {
        let mut f = fs::File::open(&path)?;
        f.read_to_end(&mut before_bytes)?;
    }
    let after_bytes = content.as_bytes().to_vec();

    if !dry_run {
        let mut f = fs::File::create(&path)?;
        f.write_all(content.as_bytes())?;
    }

    Ok(OperationResult {
        applied: !dry_run,
        output: EditPreview {
            before_preview: cap_utf8(before_bytes, preview_bytes),
            after_preview: cap_utf8(after_bytes, preview_bytes),
        },
    })
}

pub fn move_file_under_root(
    root: &str,
    from_rel: &str,
    to_rel: &str,
    dry_run: bool,
) -> anyhow::Result<OperationResult<String>> {
    let from = resolve_under_root(root, from_rel).ok_or_else(|| anyhow::anyhow!("source outside root"))?;
    let to = resolve_under_root(root, to_rel).ok_or_else(|| anyhow::anyhow!("dest outside root"))?;
    if !from.exists() { return Err(anyhow::anyhow!("source does not exist")); }
    if !dry_run {
        fs::create_dir_all(to.parent().unwrap_or(PathBuf::new().as_path()))?;
        fs::rename(&from, &to)?;
    }
    Ok(OperationResult { applied: !dry_run, output: format!("{} -> {}", from.display(), to.display()) })
}

pub fn delete_file_under_root(
    root: &str,
    rel: &str,
    dry_run: bool,
) -> anyhow::Result<OperationResult<String>> {
    let path = resolve_under_root(root, rel).ok_or_else(|| anyhow::anyhow!("path outside root"))?;
    if !path.exists() { return Err(anyhow::anyhow!("file does not exist")); }
    if !dry_run {
        if path.is_file() { fs::remove_file(&path)?; } else { fs::remove_dir_all(&path)?; }
    }
    Ok(OperationResult { applied: !dry_run, output: path.display().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn write_dry_run_does_not_modify_file() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        // Pre-create file
        let p = dir.path().join("a.txt");
        fs::write(&p, b"old").unwrap();
        let res = write_file_under_root(&root, "a.txt", "new content", false, true, 32).unwrap();
        assert!(!res.applied);
        let after = fs::read_to_string(&p).unwrap();
        assert_eq!(after, "old");
        assert!(res.output.before_preview.contains("old"));
        assert!(res.output.after_preview.contains("new content"));
    }
}


