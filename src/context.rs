use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextType {
    Architecture,
    Readme,
    Design,
    Memory,
    Custom(String),
}

impl std::fmt::Display for ContextType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextType::Architecture => write!(f, "ARCHITECTURE"),
            ContextType::Readme => write!(f, "README"),
            ContextType::Design => write!(f, "DESIGN"),
            ContextType::Memory => write!(f, "MEMORY"),
            ContextType::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextFile {
    pub context_type: ContextType,
    pub relative_path: String,
    pub file_size_bytes: u64,
    pub line_count: usize,
    pub word_count: usize,
    pub sha256: String,
    pub last_modified: Option<String>,
    pub headings: Vec<String>,
    pub summary: Option<String>,
    pub preview: Option<String>,
}

/// Extract all agent context files (ARCHITECTURE, README, DESIGN, MEMORY)
/// from the project root and standard subdirectories (docs/, memory/, .agents/).
pub fn extract_agent_context(root: &Path) -> Vec<ContextFile> {
    let mut context_files = Vec::new();

    let candidate_paths = find_candidate_context_files(root);

    for path in candidate_paths {
        if let Ok(file_info) = process_context_file(&path, root) {
            // Avoid duplicate relative paths
            if !context_files.iter().any(|c: &ContextFile| c.relative_path == file_info.relative_path) {
                context_files.push(file_info);
            }
        }
    }

    // Sort deterministically by relative path
    context_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    context_files
}

fn find_candidate_context_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Search directories: root, docs, memory, .agents, .agents/memory
    let search_dirs = [
        root.to_path_buf(),
        root.join("docs"),
        root.join("doc"),
        root.join("memory"),
        root.join(".agents"),
        root.join(".agents").join("memory"),
    ];

    for dir in &search_dirs {
        if dir.is_dir() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if is_context_file_name(file_name) {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    files
}

fn is_context_file_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_uppercase();

    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let valid_ext = ext.is_empty() || ext == "md" || ext == "markdown" || ext == "txt" || ext == "json";

    if !valid_ext {
        return false;
    }

    stem.contains("ARCHITECTURE")
        || stem.contains("README")
        || stem.contains("DESIGN")
        || stem.contains("MEMORY")
        || stem == "SPEC"
        || stem.contains("PRD")
}

fn classify_context_type(file_name: &str, relative_path: &str) -> ContextType {
    let upper = format!("{}/{}", relative_path, file_name).to_uppercase();
    if upper.contains("ARCHITECTURE") {
        ContextType::Architecture
    } else if upper.contains("README") {
        ContextType::Readme
    } else if upper.contains("DESIGN") {
        ContextType::Design
    } else if upper.contains("MEMORY") {
        ContextType::Memory
    } else {
        ContextType::Custom("DOCS".to_string())
    }
}

pub fn process_context_file(file_path: &Path, root: &Path) -> Result<ContextFile> {
    let metadata = fs::metadata(file_path)
        .with_context(|| format!("Reading metadata for {}", file_path.display()))?;
    let file_size_bytes = metadata.len();

    let content_bytes = fs::read(file_path)
        .with_context(|| format!("Reading bytes for {}", file_path.display()))?;

    // Compute SHA-256
    let mut hasher = Sha256::new();
    hasher.update(&content_bytes);
    let sha256 = hex::encode(hasher.finalize());

    // Compute last modified time
    let last_modified = metadata
        .modified()
        .ok()
        .map(|st| {
            let dt: DateTime<Utc> = st.into();
            dt.to_rfc3339()
        });

    let relative_path = file_path
        .strip_prefix(root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    let file_name = file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let context_type = classify_context_type(&file_name, &relative_path);

    // Parse UTF-8 text for lines, words, headings, summary, preview
    let content_str = String::from_utf8_lossy(&content_bytes);
    let line_count = content_str.lines().count();
    let word_count = content_str.split_whitespace().count();

    let headings = extract_headings(&content_str);
    let summary = extract_summary(&content_str);
    let preview = extract_preview(&content_str, 1500);

    Ok(ContextFile {
        context_type,
        relative_path,
        file_size_bytes,
        line_count,
        word_count,
        sha256,
        last_modified,
        headings,
        summary,
        preview: Some(preview),
    })
}

fn extract_headings(content: &str) -> Vec<String> {
    let mut headings = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            // Check if followed by space e.g. # Title, ## Section
            let stripped = trimmed.trim_start_matches('#').trim();
            if !stripped.is_empty() {
                headings.push(stripped.to_string());
            }
        }
    }
    headings
}

fn extract_summary(content: &str) -> Option<String> {
    let mut in_code_block = false;
    let mut paragraph = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            continue;
        }

        // Skip top headers or badges
        if trimmed.starts_with('#') || trimmed.starts_with("[![") || trimmed.starts_with("<!--") {
            continue;
        }

        if trimmed.is_empty() {
            if !paragraph.is_empty() {
                break;
            }
        } else {
            paragraph.push(trimmed);
        }
    }

    if paragraph.is_empty() {
        None
    } else {
        let text = paragraph.join(" ");
        if text.chars().count() > 300 {
            let truncated: String = text.chars().take(297).collect();
            Some(format!("{}...", truncated.trim()))
        } else {
            Some(text)
        }
    }
}

fn extract_preview(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(max_chars).collect();
        format!("{}\n\n... [truncated]", truncated.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_headings_and_summary() {
        let markdown = r#"# Main Architecture

This document describes the high-level system architecture of CAE.
It covers all frontend and backend services.

## Services
Details on services here.

## Data Model
Details on models here.
"#;
        let headings = extract_headings(markdown);
        assert_eq!(headings, vec!["Main Architecture", "Services", "Data Model"]);

        let summary = extract_summary(markdown);
        assert!(summary.is_some());
        assert!(summary.unwrap().contains("high-level system architecture"));
    }

    #[test]
    fn test_classify_context_type() {
        assert_eq!(classify_context_type("ARCHITECTURE.md", "ARCHITECTURE.md"), ContextType::Architecture);
        assert_eq!(classify_context_type("README.md", "README.md"), ContextType::Readme);
        assert_eq!(classify_context_type("DESIGN.md", "docs/DESIGN.md"), ContextType::Design);
        assert_eq!(classify_context_type("session.md", "memory/session.md"), ContextType::Memory);
    }
}
