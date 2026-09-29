use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitCommitRecord {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub branch: String,
    pub recorded_at: String,
    pub changed_files: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitStatusInfo {
    pub is_git_repo: bool,
    pub root_path: Option<String>,
    pub current_branch: Option<String>,
    pub head_commit: Option<String>,
    pub head_short_commit: Option<String>,
    pub head_commit_message: Option<String>,
    pub head_author: Option<String>,
    pub head_date: Option<String>,
    pub remote_origin_url: Option<String>,
    pub is_dirty: bool,
    pub modified_count: usize,
    pub untracked_count: usize,
    pub staged_count: usize,
    pub status_summary: Vec<String>,
}

/// Run a git command in the target directory and return trimmed stdout on success
fn run_git_cmd(repo_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(stdout)
    } else {
        None
    }
}

/// Inspect git status of the project directory.
pub fn inspect_git_status(dir: &Path) -> GitStatusInfo {
    let is_git = run_git_cmd(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);

    if !is_git {
        return GitStatusInfo {
            is_git_repo: false,
            root_path: None,
            current_branch: None,
            head_commit: None,
            head_short_commit: None,
            head_commit_message: None,
            head_author: None,
            head_date: None,
            remote_origin_url: None,
            is_dirty: false,
            modified_count: 0,
            untracked_count: 0,
            staged_count: 0,
            status_summary: Vec::new(),
        };
    }

    let root_path = run_git_cmd(dir, &["rev-parse", "--show-toplevel"]);
    let current_branch = run_git_cmd(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .or_else(|| run_git_cmd(dir, &["branch", "--show-current"]));

    let head_commit = run_git_cmd(dir, &["rev-parse", "HEAD"]);
    let head_short_commit = run_git_cmd(dir, &["rev-parse", "--short", "HEAD"]);
    let head_commit_message = run_git_cmd(dir, &["log", "-1", "--format=%B"]);
    let head_author = run_git_cmd(dir, &["log", "-1", "--format=%an <%ae>"]);
    let head_date = run_git_cmd(dir, &["log", "-1", "--format=%aI"]);
    let remote_origin_url = run_git_cmd(dir, &["config", "--get", "remote.origin.url"]);

    // Parse git status --porcelain
    let mut modified_count = 0;
    let mut untracked_count = 0;
    let mut staged_count = 0;
    let mut status_summary = Vec::new();

    if let Some(porcelain) = run_git_cmd(dir, &["status", "--porcelain"]) {
        for line in porcelain.lines() {
            if line.len() >= 2 {
                let index_status = line.chars().next().unwrap_or(' ');
                let work_status = line.chars().nth(1).unwrap_or(' ');

                if index_status != ' ' && index_status != '?' {
                    staged_count += 1;
                }
                if work_status == '?' && index_status == '?' {
                    untracked_count += 1;
                } else if work_status != ' ' {
                    modified_count += 1;
                }

                if status_summary.len() < 10 {
                    status_summary.push(line.to_string());
                }
            }
        }
    }

    let is_dirty = modified_count > 0 || untracked_count > 0 || staged_count > 0;

    GitStatusInfo {
        is_git_repo: true,
        root_path,
        current_branch,
        head_commit,
        head_short_commit,
        head_commit_message,
        head_author,
        head_date,
        remote_origin_url,
        is_dirty,
        modified_count,
        untracked_count,
        staged_count,
        status_summary,
    }
}

/// Extract commit details for HEAD or a specified commit hash/reference.
pub fn get_commit_record(
    dir: &Path,
    commit_ref: Option<&str>,
    override_msg: Option<&str>,
    note: Option<&str>,
) -> Result<GitCommitRecord> {
    let rev = commit_ref.unwrap_or("HEAD");

    let hash = run_git_cmd(dir, &["rev-parse", rev])
        .context("Failed to get git commit hash")?;

    let short_hash = run_git_cmd(dir, &["rev-parse", "--short", rev])
        .unwrap_or_else(|| hash[..7.min(hash.len())].to_string());

    let author = run_git_cmd(dir, &["log", "-1", "--format=%an <%ae>", rev])
        .unwrap_or_else(|| "Unknown".to_string());

    let date = run_git_cmd(dir, &["log", "-1", "--format=%aI", rev])
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let default_message = run_git_cmd(dir, &["log", "-1", "--format=%B", rev])
        .unwrap_or_default();

    let message = match override_msg {
        Some(m) if !m.trim().is_empty() => m.trim().to_string(),
        _ => default_message.trim().to_string(),
    };

    let branch = run_git_cmd(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "detached".to_string());

    let changed_files = run_git_cmd(dir, &["diff-tree", "--no-commit-id", "--name-only", "-r", &hash])
        .map(|out| out.lines().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let recorded_at = Utc::now().to_rfc3339();

    Ok(GitCommitRecord {
        hash,
        short_hash,
        author,
        date,
        message,
        branch,
        recorded_at,
        changed_files,
        note: note.map(|n| n.trim().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_current_dir() {
        let current_dir = Path::new(".");
        let status = inspect_git_status(current_dir);
        // Current directory may or may not be git, but it shouldn't panic
        println!("Git status: {:?}", status);
    }
}
