use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::context::ContextFile;
use crate::git::{GitCommitRecord, GitStatusInfo};
use crate::manifest::ManifestInfo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectLedger {
    pub id: String,
    pub name: String,
    pub canonical_path: String,
    pub tracked_at: String,
    pub updated_at: String,
    pub ecosystems: Vec<String>,
    pub git: Option<GitStatusInfo>,
    pub manifests: Vec<ManifestInfo>,
    pub context_files: Vec<ContextFile>,
    pub commits: Vec<GitCommitRecord>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
}

/// Normalize a home directory path, stripping macOS App Sandbox container suffixes if present.
pub fn normalize_home_dir(home: PathBuf) -> PathBuf {
    let home_str = home.to_string_lossy();
    if let Some(idx) = home_str.find("/Library/Containers/") {
        let base = &home_str[..idx];
        if !base.is_empty() {
            return PathBuf::from(base);
        }
    }
    home
}

/// Directory where project ledgers are stored: ~/.cae/projects/
pub fn get_ledger_dir() -> Result<PathBuf> {
    let dir = if let Some(custom_dir) =
        std::env::var_os("CAE_PROJECTS_DIR").filter(|v| !v.is_empty())
    {
        PathBuf::from(custom_dir)
    } else {
        let raw_home = dirs::home_dir().context("Could not determine user home directory")?;
        let home = normalize_home_dir(raw_home);
        home.join(".cae").join("projects")
    };
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Creating ledger directory at {}", dir.display()))?;
    }
    Ok(dir)
}

pub fn get_ledger_path(id: &str) -> Result<PathBuf> {
    let dir = get_ledger_dir()?;
    let file_name = if id.ends_with(".json") {
        id.to_string()
    } else {
        format!("{}.json", id)
    };
    Ok(dir.join(file_name))
}

pub fn save_ledger(ledger: &ProjectLedger) -> Result<PathBuf> {
    let path = get_ledger_path(&ledger.id)?;
    let json_data = serde_json::to_string_pretty(ledger)
        .context("Serializing project ledger to JSON")?;
    fs::write(&path, json_data)
        .with_context(|| format!("Writing ledger file to {}", path.display()))?;
    Ok(path)
}

pub fn load_ledger(id: &str) -> Result<ProjectLedger> {
    let path = get_ledger_path(id)?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Reading ledger file from {}", path.display()))?;
    let ledger: ProjectLedger = serde_json::from_str(&content)
        .with_context(|| format!("Deserializing ledger file from {}", path.display()))?;
    Ok(ledger)
}

pub fn delete_ledger(id: &str) -> Result<Option<PathBuf>> {
    let path = get_ledger_path(id)?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("Deleting ledger file at {}", path.display()))?;
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

pub fn list_all_ledgers() -> Result<Vec<ProjectLedger>> {
    let dir = get_ledger_dir()?;
    let mut ledgers = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(ledger) = serde_json::from_str::<ProjectLedger>(&content) {
                        ledgers.push(ledger);
                    }
                }
            }
        }
    }

    // Sort by updated_at descending
    ledgers.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(ledgers)
}

/// Find a ledger matching either the ID, ID without extension, or matching canonical path.
pub fn find_ledger(query: &str) -> Result<Option<ProjectLedger>> {
    // 1. Direct match by ID
    let candidate_path = get_ledger_path(query)?;
    if candidate_path.exists() {
        return load_ledger(query).map(Some);
    }

    // 2. Query might be a path or substring of ID
    let all = list_all_ledgers()?;
    
    // Exact canonical path match
    if let Ok(canonical) = fs::canonicalize(query) {
        let canonical_str = canonical.to_string_lossy().to_string();
        for ledger in &all {
            if ledger.canonical_path == canonical_str {
                return Ok(Some(ledger.clone()));
            }
        }
    }

    // Match by ID ignoring case or extension
    let clean_query = query.trim_end_matches(".json").to_lowercase();
    for ledger in &all {
        if ledger.id.to_lowercase() == clean_query {
            return Ok(Some(ledger.clone()));
        }
    }

    // Match by path prefix/substring
    for ledger in &all {
        if ledger.canonical_path.contains(query) {
            return Ok(Some(ledger.clone()));
        }
    }

    Ok(None)
}

/// Try to find a ledger for the current working directory
pub fn find_ledger_for_current_dir() -> Result<Option<ProjectLedger>> {
    let current_dir = std::env::current_dir().context("Getting current working directory")?;
    let canonical = fs::canonicalize(&current_dir).unwrap_or(current_dir);
    let canonical_str = canonical.to_string_lossy().to_string();

    let all = list_all_ledgers()?;
    
    // Check exact match first
    for ledger in &all {
        if ledger.canonical_path == canonical_str {
            return Ok(Some(ledger.clone()));
        }
    }

    // Check if current_dir is inside a tracked project path
    for ledger in &all {
        if canonical_str.starts_with(&ledger.canonical_path) {
            return Ok(Some(ledger.clone()));
        }
    }

    Ok(None)
}

/// Sanitize a string into a clean project ID slug (e.g. "My Project" -> "my-project")
pub fn sanitize_project_id(raw: &str) -> String {
    let mut result = String::new();
    let mut prev_dash = false;

    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if (ch == '-' || ch == '_' || ch == ' ' || ch == '.') && !prev_dash {
            result.push('-');
            prev_dash = true;
        }
    }

    let trimmed = result.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_project_id() {
        assert_eq!(sanitize_project_id("AICareerAssetEngine"), "aicareerassetengine");
        assert_eq!(sanitize_project_id("FastAPI Agent"), "fastapi-agent");
        assert_eq!(sanitize_project_id("career_asset_engine"), "career-asset-engine");
        assert_eq!(sanitize_project_id("my...cool_project!"), "my-cool-project");
    }

    #[test]
    fn test_normalize_home_dir() {
        let sandboxed = PathBuf::from(
            "/Users/jlgtechnology/Library/Containers/com.careerengine.careerAssetEngine/Data",
        );
        assert_eq!(
            normalize_home_dir(sandboxed),
            PathBuf::from("/Users/jlgtechnology")
        );

        let normal = PathBuf::from("/Users/jlgtechnology");
        assert_eq!(normalize_home_dir(normal.clone()), normal);
    }
}
