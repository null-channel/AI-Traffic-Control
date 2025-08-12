use crate::discovery::resolve_under_root;
use git2::{Repository, StatusOptions, DiffFormat};
use serde::Serialize;
use std::path::PathBuf;

fn open_repo(root: &str) -> anyhow::Result<Repository> {
    let root = resolve_under_root(root, ".").ok_or_else(|| anyhow::anyhow!("invalid root"))?;
    let repo = Repository::discover(root)?;
    Ok(repo)
}

#[derive(Debug, Serialize)]
pub struct GitStatusEntry {
    pub path: String,
    pub status: String,
}

pub fn status(root: &str) -> anyhow::Result<Vec<GitStatusEntry>> {
    let repo = open_repo(root)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts))?;
    let mut out = Vec::new();
    for e in statuses.iter() {
        let s = e.status();
        let path = e.path().unwrap_or("").to_string();
        let status = format!("{:?}", s);
        out.push(GitStatusEntry { path, status });
    }
    Ok(out)
}

pub fn diff_porcelain(root: &str) -> anyhow::Result<String> {
    let repo = open_repo(root)?;
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut diff = repo.diff_tree_to_workdir(head.as_ref(), None)?;
    let mut s = String::new();
    diff.print(DiffFormat::Patch, |_, _, l| {
        let c = l.origin();
        let content = std::str::from_utf8(l.content()).unwrap_or("");
        s.push(c);
        s.push_str(content);
        true
    })?;
    Ok(s)
}

pub fn add_all(root: &str) -> anyhow::Result<()> {
    let repo = open_repo(root)?;
    let mut idx = repo.index()?;
    idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    idx.write()?;
    Ok(())
}

pub fn commit(root: &str, message: &str) -> anyhow::Result<String> {
    let repo = open_repo(root)?;
    let sig = repo.signature()?;
    let mut idx = repo.index()?;
    let tree_id = idx.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.as_ref().into_iter().collect();
    let oid = if let Some(p) = parents.first() {
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[*p])?
    } else {
        // initial commit on orphan branch
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?
    };
    Ok(oid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn status_and_commit_work_in_temp_repo() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let _repo = Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        let st = status(&root).unwrap();
        assert!(st.iter().any(|e| e.path.ends_with("a.txt")));
        add_all(&root).unwrap();
        let oid = commit(&root, "test commit").unwrap();
        assert!(!oid.is_empty());
        let diff = diff_porcelain(&root).unwrap();
        assert!(diff.is_empty());
    }
}


