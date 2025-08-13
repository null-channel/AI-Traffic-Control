use ignore::WalkBuilder;
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub is_dir: bool,
}

pub fn list_files(root: &str, max: usize) -> Vec<FileEntry> {
    let mut out = Vec::new();
    for res in WalkBuilder::new(root).hidden(false).git_ignore(true).build() {
        if out.len() >= max { break; }
        if let Ok(dirent) = res {
            let path = dirent.path();
            if path == PathBuf::from(root) { continue; }
            out.push(FileEntry { path: path.to_string_lossy().to_string(), is_dir: path.is_dir() });
        }
    }
    out
}

pub fn search_files(root: &str, pattern: &str, max: usize) -> Vec<FileEntry> {
    let re = Regex::new(pattern).ok();
    let mut out = Vec::new();
    for res in WalkBuilder::new(root).hidden(false).git_ignore(true).build() {
        if out.len() >= max { break; }
        if let (Some(re), Ok(dirent)) = (&re, res) {
            let path = dirent.path();
            let p = path.to_string_lossy();
            if re.is_match(&p) {
                out.push(FileEntry { path: p.to_string(), is_dir: path.is_dir() });
            }
        }
    }
    out
}

fn normalize_root(root: &str) -> Option<PathBuf> {
    let pb = PathBuf::from(root);
    let abs = if pb.is_absolute() { pb } else { std::env::current_dir().ok()?.join(pb) };
    abs.canonicalize().ok()
}

pub fn resolve_under_root(root: &str, rel: &str) -> Option<PathBuf> {
    let root_abs = normalize_root(root)?;
    let joined = root_abs.join(rel);
    let normalized = joined
        .components()
        .fold(PathBuf::new(), |mut acc, comp| {
            match comp {
                Component::ParentDir => { acc.pop(); }
                Component::CurDir => {}
                other => acc.push(other.as_os_str()),
            }
            acc
        });
    let full_path = root_abs.join(&normalized);
    match full_path.canonicalize() {
        Ok(canonical) => {
            if canonical.starts_with(&root_abs) { Some(canonical) } else { None }
        }
        Err(_) => {
            // If the path does not exist yet (e.g., creating a new file), validate the parent
            let parent = full_path.parent().unwrap_or(&root_abs);
            let parent_canon = parent.canonicalize().ok()?;
            if parent_canon.starts_with(&root_abs) { Some(full_path) } else { None }
        }
    }
}

pub fn read_file_under_root(root: &str, rel: &str, max_bytes: usize) -> anyhow::Result<String> {
    let path = resolve_under_root(root, rel).ok_or_else(|| anyhow::anyhow!("path outside root"))?;
    let meta = fs::metadata(&path)?;
    if !meta.is_file() { return Err(anyhow::anyhow!("not a file")); }
    let mut file = fs::File::open(&path)?;
    let mut buf = String::new();
    // Read up to max_bytes as UTF-8 (lossy on invalid sequences)
    let mut bytes = vec![0u8; max_bytes];
    let n = file.read(&mut bytes)?;
    buf = String::from_utf8_lossy(&bytes[..n]).to_string();
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn resolve_denies_path_traversal() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let outside = resolve_under_root(&root, "../etc/passwd");
        assert!(outside.is_none());
    }

    #[test]
    fn read_file_respects_limit() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("a.txt");
        let mut f = fs::File::create(&file_path).unwrap();
        writeln!(f, "hello world").unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let content = read_file_under_root(&root, "a.txt", 5).unwrap();
        assert!(content.len() <= 5);
    }
}


