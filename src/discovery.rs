use ignore::WalkBuilder;
use regex::Regex;
use serde::Serialize;
use std::path::PathBuf;

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


