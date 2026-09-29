use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyInfo {
    pub name: String,
    pub version: Option<String>,
    pub is_dev: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestInfo {
    pub ecosystem: String,
    pub file_name: String,
    pub relative_path: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub dependencies_count: usize,
    pub dependencies: Vec<DependencyInfo>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Detect and parse all supported manifests in the given directory.
/// Supports pubspec.yaml (Flutter/Dart), package.json (Node/JS/TS),
/// pyproject.toml (Python), and Cargo.toml (Rust).
pub fn detect_and_parse_manifests(root: &Path) -> Vec<ManifestInfo> {
    let mut manifests = Vec::new();

    // Check root directory
    scan_dir_for_manifests(root, root, &mut manifests);

    // If no manifests found at root, check one level of subdirectories
    // (useful for mono-repos or repos where project files reside in subfolders)
    if manifests.is_empty() {
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if !dir_name.starts_with('.') && dir_name != "target" && dir_name != "node_modules" && dir_name != "build" {
                        scan_dir_for_manifests(&path, root, &mut manifests);
                    }
                }
            }
        }
    }

    manifests
}

fn scan_dir_for_manifests(dir: &Path, root: &Path, manifests: &mut Vec<ManifestInfo>) {
    let candidates = [
        ("pubspec.yaml", parse_pubspec as fn(&Path, &Path) -> Result<ManifestInfo>),
        ("pubspec.yml", parse_pubspec),
        ("package.json", parse_package_json),
        ("pyproject.toml", parse_pyproject_toml),
        ("Cargo.toml", parse_cargo_toml),
    ];

    for (file_name, parser) in candidates {
        let manifest_path = dir.join(file_name);
        if manifest_path.is_file() {
            match parser(&manifest_path, root) {
                Ok(info) => manifests.push(info),
                Err(err) => {
                    eprintln!("Warning: Failed to parse {}: {}", manifest_path.display(), err);
                }
            }
        }
    }
}

/// Helper to compute relative path string
fn rel_path_str(full_path: &Path, root: &Path) -> String {
    full_path
        .strip_prefix(root)
        .unwrap_or(full_path)
        .to_string_lossy()
        .to_string()
}

/// 1. Parse Flutter / Dart pubspec.yaml
pub fn parse_pubspec(file_path: &Path, root: &Path) -> Result<ManifestInfo> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;
    parse_pubspec_str(&content, &rel_path_str(file_path, root))
}

pub fn parse_pubspec_str(content: &str, relative_path: &str) -> Result<ManifestInfo> {
    let val: serde_yaml::Value = serde_yaml::from_str(content)
        .context("Parsing YAML content")?;

    let name = val.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
    let version = val.get("version").and_then(|v| {
        v.as_str().map(ToString::to_string).or_else(|| v.as_i64().map(|n| n.to_string()))
    });
    let description = val.get("description").and_then(|v| v.as_str()).map(ToString::to_string);

    let mut dependencies = Vec::new();

    if let Some(deps) = val.get("dependencies").and_then(|v| v.as_mapping()) {
        for (k, v) in deps {
            if let Some(dep_name) = k.as_str() {
                let ver = extract_yaml_version(v);
                dependencies.push(DependencyInfo {
                    name: dep_name.to_string(),
                    version: ver,
                    is_dev: false,
                });
            }
        }
    }

    if let Some(dev_deps) = val.get("dev_dependencies").and_then(|v| v.as_mapping()) {
        for (k, v) in dev_deps {
            if let Some(dep_name) = k.as_str() {
                let ver = extract_yaml_version(v);
                dependencies.push(DependencyInfo {
                    name: dep_name.to_string(),
                    version: ver,
                    is_dev: true,
                });
            }
        }
    }

    let mut metadata = BTreeMap::new();
    if let Some(env) = val.get("environment") {
        if let Ok(json_env) = serde_json::to_value(env) {
            metadata.insert("environment".to_string(), json_env);
        }
    }
    if let Some(flutter) = val.get("flutter") {
        metadata.insert("flutter_configured".to_string(), serde_json::Value::Bool(flutter.is_mapping()));
    }

    let file_name = Path::new(relative_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "pubspec.yaml".to_string());

    Ok(ManifestInfo {
        ecosystem: "Flutter/Dart".to_string(),
        file_name,
        relative_path: relative_path.to_string(),
        name,
        version,
        description,
        dependencies_count: dependencies.len(),
        dependencies,
        metadata,
    })
}

fn extract_yaml_version(v: &serde_yaml::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        Some(s.to_string())
    } else if let Some(m) = v.as_mapping() {
        if let Some(sdk) = m.get(serde_yaml::Value::String("sdk".to_string())) {
            Some(format!("sdk: {}", sdk.as_str().unwrap_or("")))
        } else if let Some(ver) = m.get(serde_yaml::Value::String("version".to_string())) {
            ver.as_str().map(ToString::to_string)
        } else {
            m.get(serde_yaml::Value::String("git".to_string()))
                .map(|git| format!("git: {:?}", git))
        }
    } else {
        None
    }
}

/// 2. Parse Node / TypeScript package.json
pub fn parse_package_json(file_path: &Path, root: &Path) -> Result<ManifestInfo> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;
    parse_package_json_str(&content, &rel_path_str(file_path, root))
}

pub fn parse_package_json_str(content: &str, relative_path: &str) -> Result<ManifestInfo> {
    let val: serde_json::Value = serde_json::from_str(content)
        .context("Parsing JSON content")?;

    let name = val.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
    let version = val.get("version").and_then(|v| v.as_str()).map(ToString::to_string);
    let description = val.get("description").and_then(|v| v.as_str()).map(ToString::to_string);

    let mut dependencies = Vec::new();

    if let Some(deps) = val.get("dependencies").and_then(|v| v.as_object()) {
        for (k, v) in deps {
            dependencies.push(DependencyInfo {
                name: k.clone(),
                version: v.as_str().map(ToString::to_string),
                is_dev: false,
            });
        }
    }

    if let Some(dev_deps) = val.get("devDependencies").and_then(|v| v.as_object()) {
        for (k, v) in dev_deps {
            dependencies.push(DependencyInfo {
                name: k.clone(),
                version: v.as_str().map(ToString::to_string),
                is_dev: true,
            });
        }
    }

    let mut metadata = BTreeMap::new();
    if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
        let script_names: Vec<serde_json::Value> = scripts.keys().map(|k| serde_json::Value::String(k.clone())).collect();
        metadata.insert("scripts".to_string(), serde_json::Value::Array(script_names));
    }
    if let Some(license) = val.get("license").and_then(|l| l.as_str()) {
        metadata.insert("license".to_string(), serde_json::Value::String(license.to_string()));
    }
    if let Some(main) = val.get("main").and_then(|m| m.as_str()) {
        metadata.insert("main".to_string(), serde_json::Value::String(main.to_string()));
    }

    let file_name = Path::new(relative_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "package.json".to_string());

    Ok(ManifestInfo {
        ecosystem: "Node/JS".to_string(),
        file_name,
        relative_path: relative_path.to_string(),
        name,
        version,
        description,
        dependencies_count: dependencies.len(),
        dependencies,
        metadata,
    })
}

/// 3. Parse Python pyproject.toml
pub fn parse_pyproject_toml(file_path: &Path, root: &Path) -> Result<ManifestInfo> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;
    parse_pyproject_toml_str(&content, &rel_path_str(file_path, root))
}

pub fn parse_pyproject_toml_str(content: &str, relative_path: &str) -> Result<ManifestInfo> {
    let val: toml::Value = toml::from_str(content)
        .context("Parsing TOML content")?;

    let mut name = None;
    let mut version = None;
    let mut description = None;
    let mut dependencies = Vec::new();
    let mut metadata = BTreeMap::new();

    // Check PEP 621 [project] table first
    if let Some(project) = val.get("project").and_then(|p| p.as_table()) {
        name = project.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
        version = project.get("version").and_then(|v| v.as_str()).map(ToString::to_string);
        description = project.get("description").and_then(|v| v.as_str()).map(ToString::to_string);

        if let Some(requires_python) = project.get("requires-python").and_then(|v| v.as_str()) {
            metadata.insert("requires_python".to_string(), serde_json::Value::String(requires_python.to_string()));
        }

        if let Some(deps) = project.get("dependencies").and_then(|v| v.as_array()) {
            for dep_val in deps {
                if let Some(dep_str) = dep_val.as_str() {
                    let (dname, dver) = parse_python_specifier(dep_str);
                    dependencies.push(DependencyInfo {
                        name: dname,
                        version: dver,
                        is_dev: false,
                    });
                }
            }
        }

        if let Some(opt_deps) = project.get("optional-dependencies").and_then(|v| v.as_table()) {
            for (group_name, group_deps) in opt_deps {
                if let Some(dep_array) = group_deps.as_array() {
                    for dep_val in dep_array {
                        if let Some(dep_str) = dep_val.as_str() {
                            let (dname, dver) = parse_python_specifier(dep_str);
                            dependencies.push(DependencyInfo {
                                name: format!("{}[{}]", dname, group_name),
                                version: dver,
                                is_dev: true,
                            });
                        }
                    }
                }
            }
        }
    }

    // Fallback/check Poetry [tool.poetry]
    if let Some(poetry) = val.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.as_table()) {
        if name.is_none() {
            name = poetry.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
        }
        if version.is_none() {
            version = poetry.get("version").and_then(|v| v.as_str()).map(ToString::to_string);
        }
        if description.is_none() {
            description = poetry.get("description").and_then(|v| v.as_str()).map(ToString::to_string);
        }

        if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
            for (k, v) in deps {
                if k != "python" && !dependencies.iter().any(|d| &d.name == k) {
                    let ver = if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(t) = v.as_table() {
                        t.get("version").and_then(|x| x.as_str()).map(ToString::to_string)
                    } else {
                        None
                    };
                    dependencies.push(DependencyInfo {
                        name: k.clone(),
                        version: ver,
                        is_dev: false,
                    });
                }
            }
        }

        if let Some(dev_deps) = poetry.get("group").and_then(|g| g.get("dev")).and_then(|d| d.get("dependencies")).and_then(|v| v.as_table()) {
            for (k, v) in dev_deps {
                let ver = v.as_str().map(ToString::to_string);
                dependencies.push(DependencyInfo {
                    name: k.clone(),
                    version: ver,
                    is_dev: true,
                });
            }
        }
    }

    let file_name = Path::new(relative_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "pyproject.toml".to_string());

    Ok(ManifestInfo {
        ecosystem: "Python".to_string(),
        file_name,
        relative_path: relative_path.to_string(),
        name,
        version,
        description,
        dependencies_count: dependencies.len(),
        dependencies,
        metadata,
    })
}

fn parse_python_specifier(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim();
    let separators = [">=", "<=", "==", "!=", "~=", ">", "<", ";"];
    for sep in separators {
        if let Some(idx) = spec.find(sep) {
            let name = spec[..idx].trim().to_string();
            let ver = spec[idx..].trim().to_string();
            return (name, Some(ver));
        }
    }
    (spec.to_string(), None)
}

/// 4. Parse Rust Cargo.toml
pub fn parse_cargo_toml(file_path: &Path, root: &Path) -> Result<ManifestInfo> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;
    parse_cargo_toml_str(&content, &rel_path_str(file_path, root))
}

pub fn parse_cargo_toml_str(content: &str, relative_path: &str) -> Result<ManifestInfo> {
    let val: toml::Value = toml::from_str(content)
        .context("Parsing TOML content")?;

    let mut name = None;
    let mut version = None;
    let mut description = None;
    let mut metadata = BTreeMap::new();
    let mut dependencies = Vec::new();

    if let Some(pkg) = val.get("package").and_then(|p| p.as_table()) {
        name = pkg.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
        version = pkg.get("version").and_then(|v| v.as_str()).map(ToString::to_string);
        description = pkg.get("description").and_then(|v| v.as_str()).map(ToString::to_string);

        if let Some(edition) = pkg.get("edition").and_then(|v| v.as_str()) {
            metadata.insert("edition".to_string(), serde_json::Value::String(edition.to_string()));
        }
        if let Some(license) = pkg.get("license").and_then(|v| v.as_str()) {
            metadata.insert("license".to_string(), serde_json::Value::String(license.to_string()));
        }
    }

    if let Some(deps) = val.get("dependencies").and_then(|d| d.as_table()) {
        for (k, v) in deps {
            let ver = extract_cargo_dep_version(v);
            dependencies.push(DependencyInfo {
                name: k.clone(),
                version: ver,
                is_dev: false,
            });
        }
    }

    if let Some(dev_deps) = val.get("dev-dependencies").and_then(|d| d.as_table()) {
        for (k, v) in dev_deps {
            let ver = extract_cargo_dep_version(v);
            dependencies.push(DependencyInfo {
                name: k.clone(),
                version: ver,
                is_dev: true,
            });
        }
    }

    if let Some(ws) = val.get("workspace").and_then(|w| w.as_table()) {
        if let Some(members) = ws.get("members").and_then(|m| m.as_array()) {
            let member_strings: Vec<serde_json::Value> = members
                .iter()
                .filter_map(|m| m.as_str().map(|s| serde_json::Value::String(s.to_string())))
                .collect();
            metadata.insert("workspace_members".to_string(), serde_json::Value::Array(member_strings));
        }
    }

    let file_name = Path::new(relative_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Cargo.toml".to_string());

    Ok(ManifestInfo {
        ecosystem: "Rust".to_string(),
        file_name,
        relative_path: relative_path.to_string(),
        name,
        version,
        description,
        dependencies_count: dependencies.len(),
        dependencies,
        metadata,
    })
}

fn extract_cargo_dep_version(v: &toml::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        Some(s.to_string())
    } else if let Some(t) = v.as_table() {
        if let Some(ver) = t.get("version").and_then(|x| x.as_str()) {
            Some(ver.to_string())
        } else if let Some(path) = t.get("path").and_then(|x| x.as_str()) {
            Some(format!("path: {}", path))
        } else {
            t.get("git")
                .and_then(|x| x.as_str())
                .map(|git| format!("git: {}", git))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pubspec() {
        let yaml = r#"
name: sample_flutter_app
description: A new Flutter application.
version: 1.2.3+4
environment:
  sdk: '>=3.0.0 <4.0.0'
dependencies:
  flutter:
    sdk: flutter
  cupertino_icons: ^1.0.2
dev_dependencies:
  flutter_test:
    sdk: flutter
"#;
        let info = parse_pubspec_str(yaml, "pubspec.yaml").expect("Failed to parse pubspec");
        assert_eq!(info.ecosystem, "Flutter/Dart");
        assert_eq!(info.name.as_deref(), Some("sample_flutter_app"));
        assert_eq!(info.version.as_deref(), Some("1.2.3+4"));
        assert_eq!(info.dependencies_count, 3);
        assert!(info.dependencies.iter().any(|d| d.name == "cupertino_icons" && !d.is_dev));
        assert!(info.dependencies.iter().any(|d| d.name == "flutter_test" && d.is_dev));
    }

    #[test]
    fn test_parse_package_json() {
        let json = r#"{
  "name": "my-node-app",
  "version": "2.0.0",
  "description": "Awesome node app",
  "scripts": {
    "test": "jest",
    "build": "tsc"
  },
  "dependencies": {
    "express": "^4.18.2"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  }
}"#;
        let info = parse_package_json_str(json, "package.json").expect("Failed to parse package.json");
        assert_eq!(info.ecosystem, "Node/JS");
        assert_eq!(info.name.as_deref(), Some("my-node-app"));
        assert_eq!(info.version.as_deref(), Some("2.0.0"));
        assert_eq!(info.dependencies_count, 2);
        assert!(info.dependencies.iter().any(|d| d.name == "express" && !d.is_dev));
        assert!(info.dependencies.iter().any(|d| d.name == "typescript" && d.is_dev));
    }

    #[test]
    fn test_parse_pyproject() {
        let toml_str = r#"
[project]
name = "fastapi-agent"
version = "0.1.0"
description = "FastAPI Agent Service"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.100.0",
    "pydantic==2.5.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0.0"
]
"#;
        let info = parse_pyproject_toml_str(toml_str, "pyproject.toml").expect("Failed to parse pyproject.toml");
        assert_eq!(info.ecosystem, "Python");
        assert_eq!(info.name.as_deref(), Some("fastapi-agent"));
        assert_eq!(info.version.as_deref(), Some("0.1.0"));
        assert_eq!(info.dependencies_count, 3);
        assert!(info.dependencies.iter().any(|d| d.name == "fastapi" && d.version.as_deref() == Some(">=0.100.0")));
    }

    #[test]
    fn test_parse_cargo_toml() {
        let toml_str = r#"
[package]
name = "cae-cli"
version = "0.1.0"
edition = "2021"
description = "CAE CLI tool"

[dependencies]
clap = "4.5"
serde = { version = "1.0", features = ["derive"] }

[dev-dependencies]
tempfile = "3"
"#;
        let info = parse_cargo_toml_str(toml_str, "Cargo.toml").expect("Failed to parse Cargo.toml");
        assert_eq!(info.ecosystem, "Rust");
        assert_eq!(info.name.as_deref(), Some("cae-cli"));
        assert_eq!(info.version.as_deref(), Some("0.1.0"));
        assert_eq!(info.dependencies_count, 3);
        assert!(info.dependencies.iter().any(|d| d.name == "clap" && !d.is_dev));
        assert!(info.dependencies.iter().any(|d| d.name == "tempfile" && d.is_dev));
    }
}
