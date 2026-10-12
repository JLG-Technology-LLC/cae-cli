mod context;
mod git;
mod ledger;
mod manifest;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use colored::*;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};

use crate::context::extract_agent_context;
use crate::git::{get_commit_record, inspect_git_status};
use crate::ledger::{
    delete_ledger, find_ledger, find_ledger_for_current_dir, list_all_ledgers,
    sanitize_project_id, save_ledger, ProjectLedger,
};
use crate::manifest::detect_and_parse_manifests;

#[derive(Parser, Debug)]
#[command(name = "cae")]
#[command(author = "CAE Team")]
#[command(version = "0.1.0")]
#[command(about = "Career Asset Engine CLI - tracks projects, manifests, agent context, and commits", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Track a project repository or directory
    Track {
        /// Path to the project root (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Custom identifier for the project
        #[arg(long)]
        id: Option<String>,

        /// Custom human-readable project name
        #[arg(long)]
        name: Option<String>,

        /// Tags associated with this project
        #[arg(short, long)]
        tag: Vec<String>,

        /// Optional notes for the project
        #[arg(long)]
        notes: Option<String>,
    },

    /// Untrack a project by ID or directory path
    Untrack {
        /// Project ID, filename, or directory path
        target: String,
    },

    /// Show status of tracked project(s)
    Status {
        /// Project ID or path to inspect (defaults to current project or all)
        target: Option<String>,

        /// List all tracked projects
        #[arg(short, long)]
        all: bool,

        /// Output raw JSON format
        #[arg(long)]
        json: bool,
    },

    /// Record a git commit into the project ledger
    RecordCommit {
        /// Project ID or path (defaults to detecting project for current directory)
        #[arg(short, long)]
        project: Option<String>,

        /// Commit message override (defaults to git commit message)
        #[arg(short, long)]
        message: Option<String>,

        /// Supplementary agent or developer note
        #[arg(short, long)]
        note: Option<String>,

        /// Specific commit hash or reference (defaults to HEAD)
        #[arg(short, long)]
        commit: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Track {
            path,
            id,
            name,
            tag,
            notes,
        } => handle_track(path, id, name, tag, notes),

        Commands::Untrack { target } => handle_untrack(&target),

        Commands::Status { target, all, json } => handle_status(target, all, json),

        Commands::RecordCommit {
            project,
            message,
            note,
            commit,
        } => handle_record_commit(project, message, note, commit),
    }
}

// -----------------------------------------------------------------------------
// 1. CAE TRACK
// -----------------------------------------------------------------------------
fn handle_track(
    path: PathBuf,
    custom_id: Option<String>,
    custom_name: Option<String>,
    tags: Vec<String>,
    notes: Option<String>,
) -> Result<()> {
    if !path.exists() {
        bail!("Target path '{}' does not exist.", path.display());
    }

    let canonical_path = fs::canonicalize(&path)
        .with_context(|| format!("Resolving canonical path for {}", path.display()))?;

    if !canonical_path.is_dir() {
        bail!("Target path '{}' is not a directory.", canonical_path.display());
    }

    println!(
        "{} project at {}",
        "Scanning".bold().cyan(),
        canonical_path.display().to_string().yellow()
    );

    // 1. Detect and parse manifests
    let manifests = detect_and_parse_manifests(&canonical_path);

    // 2. Extract agent context files
    let context_files = extract_agent_context(&canonical_path);

    // 3. Inspect git status
    let git_status = inspect_git_status(&canonical_path);

    // Determine project name
    let detected_name = manifests
        .iter()
        .find_map(|m| m.name.clone())
        .unwrap_or_else(|| {
            canonical_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string())
        });

    let project_name = custom_name.unwrap_or(detected_name);

    // Determine project ID
    let project_id = if let Some(cid) = custom_id {
        sanitize_project_id(&cid)
    } else {
        let base_id = sanitize_project_id(&project_name);
        // Disambiguate if existing project has same ID but different path
        if let Ok(Some(existing)) = ledger::load_ledger(&base_id).map(Some) {
            if existing.canonical_path != canonical_path.to_string_lossy() {
                let path_hash = &hex::encode(sha2::Sha256::digest(
                    canonical_path.to_string_lossy().as_bytes(),
                ))[..6];
                format!("{}-{}", base_id, path_hash)
            } else {
                base_id
            }
        } else {
            base_id
        }
    };

    // Collect ecosystems
    let mut ecosystems: Vec<String> = manifests
        .iter()
        .map(|m| m.ecosystem.clone())
        .collect();
    ecosystems.sort();
    ecosystems.dedup();

    // Check if updating existing ledger
    let existing_ledger = ledger::load_ledger(&project_id).ok();
    let tracked_at = existing_ledger
        .as_ref()
        .map(|e| e.tracked_at.clone())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let commits = existing_ledger
        .as_ref()
        .map(|e| e.commits.clone())
        .unwrap_or_default();

    let combined_tags = if tags.is_empty() {
        existing_ledger.as_ref().map(|e| e.tags.clone()).unwrap_or_default()
    } else {
        tags
    };

    let ledger = ProjectLedger {
        id: project_id.clone(),
        name: project_name.clone(),
        canonical_path: canonical_path.to_string_lossy().to_string(),
        tracked_at,
        updated_at: Utc::now().to_rfc3339(),
        ecosystems: ecosystems.clone(),
        git: Some(git_status.clone()),
        manifests: manifests.clone(),
        context_files: context_files.clone(),
        commits,
        tags: combined_tags,
        notes: notes.or_else(|| existing_ledger.and_then(|e| e.notes)),
    };

    let ledger_path = save_ledger(&ledger)?;

    println!(
        "\n{} Successfully tracked project {} ({})",
        "✔".green().bold(),
        project_name.bold().green(),
        project_id.cyan()
    );
    println!("  {:<16} {}", "Location:".dimmed(), ledger.canonical_path);
    println!("  {:<16} {}", "Ledger File:".dimmed(), ledger_path.display());
    println!(
        "  {:<16} {}",
        "Ecosystem(s):".dimmed(),
        if ecosystems.is_empty() {
            "None detected".dimmed().to_string()
        } else {
            ecosystems.join(", ").magenta().to_string()
        }
    );

    // Manifests summary
    if !manifests.is_empty() {
        println!("\n{}", "Manifests Detected:".bold());
        for m in &manifests {
            println!(
                "  • {:<14} {} (version: {}, deps: {})",
                format!("[{}]", m.ecosystem).blue(),
                m.relative_path.yellow(),
                m.version.as_deref().unwrap_or("n/a").cyan(),
                m.dependencies_count.to_string().green()
            );
        }
    }

    // Context files summary
    if !context_files.is_empty() {
        println!("\n{}", "Agent Context Files Extracted:".bold());
        for c in &context_files {
            let heading_preview = if c.headings.is_empty() {
                String::new()
            } else {
                format!(" - headings: [{}]", c.headings.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
            };
            println!(
                "  • {:<14} {} ({} lines, {} bytes){}",
                format!("[{}]", c.context_type).purple(),
                c.relative_path.yellow(),
                c.line_count.to_string().cyan(),
                c.file_size_bytes.to_string().dimmed(),
                heading_preview.dimmed()
            );
        }
    }

    // Git summary
    if git_status.is_git_repo {
        println!("\n{}", "Git Repository:".bold());
        println!(
            "  • Branch: {} | HEAD: {} | Status: {}",
            git_status.current_branch.as_deref().unwrap_or("detached").cyan(),
            git_status.head_short_commit.as_deref().unwrap_or("unknown").yellow(),
            if git_status.is_dirty {
                format!("Dirty ({} modified, {} untracked, {} staged)", git_status.modified_count, git_status.untracked_count, git_status.staged_count).red()
            } else {
                "Clean".green()
            }
        );

        match install_post_commit_hook(&canonical_path) {
            Ok(Some(hook_path)) => {
                println!("  • Git Hook: Installed at {}", hook_path.display().to_string().cyan());
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("  • Git Hook: Warning: could not install post-commit hook: {}", e);
            }
        }
    } else {
        println!("\n{}", "Git Repository: Not a git repository".dimmed());
    }

    println!();
    Ok(())
}

fn install_post_commit_hook(repo_root: &Path) -> Result<Option<PathBuf>> {
    let git_dir = repo_root.join(".git");
    if !git_dir.exists() {
        return Ok(None);
    }
    let hooks_dir = if git_dir.is_dir() {
        git_dir.join("hooks")
    } else {
        return Ok(None);
    };

    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("post-commit");
    let hook_content = "#!/bin/sh\ncae record-commit\n";
    fs::write(&hook_path, hook_content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755));
    }

    Ok(Some(hook_path))
}

fn remove_post_commit_hook(repo_root: &Path) -> Result<bool> {
    let hook_path = repo_root.join(".git").join("hooks").join("post-commit");
    if hook_path.exists() {
        if let Ok(content) = fs::read_to_string(&hook_path) {
            if content.contains("cae record-commit") {
                fs::remove_file(&hook_path)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// -----------------------------------------------------------------------------
// 2. CAE UNTRACK
// -----------------------------------------------------------------------------
fn handle_untrack(target: &str) -> Result<()> {
    let ledger_opt = find_ledger(target)?;

    let ledger = match ledger_opt {
        Some(l) => l,
        None => {
            // Check if user specified a file path directly
            let target_path = Path::new(target);
            if target_path.exists() {
                if let Ok(canon) = fs::canonicalize(target_path) {
                    if let Some(l) = find_ledger(&canon.to_string_lossy())? {
                        l
                    } else {
                        bail!("No tracked project found matching target '{}'.", target);
                    }
                } else {
                    bail!("No tracked project found matching target '{}'.", target);
                }
            } else {
                bail!("No tracked project found matching target '{}'.", target);
            }
        }
    };

    let repo_path = Path::new(&ledger.canonical_path);
    if let Ok(true) = remove_post_commit_hook(repo_path) {
        println!("  Removed git hook: .git/hooks/post-commit");
    }

    match delete_ledger(&ledger.id)? {
        Some(path) => {
            println!(
                "{} Untracked project '{}' ({})",
                "✔".green().bold(),
                ledger.name.bold(),
                ledger.id.cyan()
            );
            println!("  Deleted ledger file: {}", path.display().to_string().dimmed());
        }
        None => {
            println!("Project '{}' was not found in ledger.", ledger.id);
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// 3. CAE STATUS
// -----------------------------------------------------------------------------
fn handle_status(target: Option<String>, show_all: bool, json_output: bool) -> Result<()> {
    if show_all {
        return print_all_projects_status(json_output);
    }

    let ledger_opt = match target {
        Some(ref t) => find_ledger(t)?,
        None => find_ledger_for_current_dir()?,
    };

    match ledger_opt {
        Some(ledger) => {
            if json_output {
                let json = serde_json::to_string_pretty(&ledger)?;
                println!("{}", json);
                return Ok(());
            }

            print_detailed_project_status(&ledger);
        }
        None => {
            // If target wasn't found or no target provided outside a tracked project, show list
            println!(
                "{}\n",
                "No specific project specified or current directory is not tracked.".yellow()
            );
            print_all_projects_status(json_output)?;
            println!(
                "\n{} Run '{}' to track a project.",
                "Tip:".dimmed(),
                "cae track <path>".cyan()
            );
        }
    }

    Ok(())
}

fn print_all_projects_status(json_output: bool) -> Result<()> {
    let projects = list_all_ledgers()?;

    if json_output {
        let json = serde_json::to_string_pretty(&projects)?;
        println!("{}", json);
        return Ok(());
    }

    if projects.is_empty() {
        println!("{}", "No projects are currently tracked in CAE.".yellow());
        println!("Run '{}' to track your first project.", "cae track <path>".cyan());
        return Ok(());
    }

    println!(
        "{}",
        format!("Tracked Projects ({})", projects.len()).bold().cyan()
    );
    println!("{:-<95}", "");
    println!(
        "{:<20} {:<22} {:<15} {:<18} {:<12}",
        "ID".bold(),
        "NAME".bold(),
        "ECOSYSTEMS".bold(),
        "BRANCH / HEAD".bold(),
        "COMMITS".bold()
    );
    println!("{:-<95}", "");

    for p in &projects {
        let eco = if p.ecosystems.is_empty() {
            "-".to_string()
        } else {
            p.ecosystems.join(", ")
        };

        let git_info = if let Some(g) = &p.git {
            if g.is_git_repo {
                let br = g.current_branch.as_deref().unwrap_or("detached");
                let sha = g.head_short_commit.as_deref().unwrap_or("n/a");
                format!("{}:{}", br, sha)
            } else {
                "non-git".to_string()
            }
        } else {
            "n/a".to_string()
        };

        let commit_count = format!("{} commits", p.commits.len());

        println!(
            "{:<20} {:<22} {:<15} {:<18} {:<12}",
            p.id.cyan(),
            truncate_str(&p.name, 20),
            truncate_str(&eco, 14).magenta(),
            git_info.yellow(),
            commit_count.green()
        );
        println!("  └─ {}", p.canonical_path.dimmed());
    }
    println!("{:-<95}", "");

    Ok(())
}

fn print_detailed_project_status(ledger: &ProjectLedger) {
    println!("{}", "==================================================================".dimmed());
    println!(
        "Project: {} ({})",
        ledger.name.bold().green(),
        ledger.id.cyan()
    );
    println!("{}", "==================================================================".dimmed());
    println!("  Path:        {}", ledger.canonical_path.yellow());
    println!("  Tracked At:  {}", ledger.tracked_at.dimmed());
    println!("  Updated At:  {}", ledger.updated_at.dimmed());
    if !ledger.tags.is_empty() {
        println!("  Tags:        {}", ledger.tags.join(", ").blue());
    }
    if let Some(notes) = &ledger.notes {
        println!("  Notes:       {}", notes);
    }

    // Git Status
    let project_path = Path::new(&ledger.canonical_path);
    let live_git = if project_path.is_dir() {
        inspect_git_status(project_path)
    } else {
        ledger.git.clone().unwrap_or(git::GitStatusInfo {
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
        })
    };

    println!("\n{}", "Git Repository Status:".bold());
    if live_git.is_git_repo {
        println!(
            "  Branch:      {}",
            live_git.current_branch.as_deref().unwrap_or("detached").cyan()
        );
        println!(
            "  HEAD Commit: {} ({})",
            live_git.head_short_commit.as_deref().unwrap_or("n/a").yellow(),
            live_git.head_commit.as_deref().unwrap_or("n/a").dimmed()
        );
        if let Some(author) = &live_git.head_author {
            println!("  Author:      {}", author);
        }
        if let Some(msg) = &live_git.head_commit_message {
            let first_line = msg.lines().next().unwrap_or("");
            println!("  Message:     {}", first_line);
        }
        if let Some(remote) = &live_git.remote_origin_url {
            println!("  Remote:      {}", remote.dimmed());
        }

        if live_git.is_dirty {
            println!(
                "  Working Tree: {}",
                format!(
                    "DIRTY ({} modified, {} untracked, {} staged)",
                    live_git.modified_count, live_git.untracked_count, live_git.staged_count
                )
                .red()
            );
            for s in live_git.status_summary.iter().take(5) {
                println!("    {}", s.dimmed());
            }
            if live_git.status_summary.len() > 5 {
                println!(
                    "    ... and {} more",
                    live_git.status_summary.len() - 5
                );
            }
        } else {
            println!("  Working Tree: {}", "CLEAN".green());
        }
    } else {
        println!("  Not a git repository or directory missing.");
    }

    // Manifests
    println!("\n{}", format!("Manifests ({}):", ledger.manifests.len()).bold());
    if ledger.manifests.is_empty() {
        println!("  None found.");
    } else {
        for m in &ledger.manifests {
            println!(
                "  • {:<12} {} (version: {})",
                format!("[{}]", m.ecosystem).blue(),
                m.relative_path.yellow(),
                m.version.as_deref().unwrap_or("n/a").cyan()
            );
            if let Some(desc) = &m.description {
                let first_line = desc.lines().next().unwrap_or("");
                println!("    Description: {}", truncate_str(first_line, 70).dimmed());
            }
            println!(
                "    Dependencies ({} total):",
                m.dependencies_count
            );
            let sample_deps: Vec<String> = m
                .dependencies
                .iter()
                .take(6)
                .map(|d| {
                    if let Some(v) = &d.version {
                        format!("{} ({})", d.name, v)
                    } else {
                        d.name.clone()
                    }
                })
                .collect();
            if !sample_deps.is_empty() {
                println!("      {}", sample_deps.join(", ").dimmed());
            }
        }
    }

    // Context Files
    println!(
        "\n{}",
        format!("Agent Context Files ({}):", ledger.context_files.len()).bold()
    );
    if ledger.context_files.is_empty() {
        println!("  None found.");
    } else {
        for c in &ledger.context_files {
            println!(
                "  • {:<14} {} ({} lines, sha: {})",
                format!("[{}]", c.context_type).purple(),
                c.relative_path.yellow(),
                c.line_count.to_string().cyan(),
                c.sha256[..8].dimmed()
            );
            if let Some(summary) = &c.summary {
                println!("    Summary: {}", truncate_str(summary, 80).dimmed());
            }
            if !c.headings.is_empty() {
                let sample_headings: Vec<String> = c.headings.iter().take(4).cloned().collect();
                println!("    Headings: {}", sample_headings.join(" | ").dimmed());
            }
        }
    }

    // Recorded Commits
    println!(
        "\n{}",
        format!("Recorded Commits in Ledger ({}):", ledger.commits.len()).bold()
    );
    if ledger.commits.is_empty() {
        println!("  No commits recorded yet. Run 'cae record-commit' to record one.");
    } else {
        for c in ledger.commits.iter().take(10) {
            println!(
                "  • {} [{}] {} ({})",
                c.short_hash.yellow(),
                c.branch.cyan(),
                truncate_str(&c.message, 50),
                c.date.dimmed()
            );
            if let Some(note) = &c.note {
                println!("    Note: {}", note.green());
            }
            if !c.changed_files.is_empty() {
                println!(
                    "    Changed files ({}): {}",
                    c.changed_files.len(),
                    c.changed_files.iter().take(4).cloned().collect::<Vec<_>>().join(", ").dimmed()
                );
            }
        }
        if ledger.commits.len() > 10 {
            println!(
                "  ... and {} older recorded commits",
                ledger.commits.len() - 10
            );
        }
    }
    println!();
}

// -----------------------------------------------------------------------------
// 4. CAE RECORD-COMMIT
// -----------------------------------------------------------------------------
fn handle_record_commit(
    project_query: Option<String>,
    message_override: Option<String>,
    note: Option<String>,
    commit_ref: Option<String>,
) -> Result<()> {
    let ledger_opt = match project_query {
        Some(ref q) => find_ledger(q)?,
        None => find_ledger_for_current_dir()?,
    };

    let mut ledger = match ledger_opt {
        Some(l) => l,
        None => {
            bail!("Could not identify project. Please specify --project <id_or_path> or run inside a tracked project directory.");
        }
    };

    let project_path = Path::new(&ledger.canonical_path);
    if !project_path.exists() {
        bail!(
            "Project path '{}' does not exist on disk.",
            ledger.canonical_path
        );
    }

    println!(
        "Recording commit for project {} ({})",
        ledger.name.bold().green(),
        ledger.id.cyan()
    );

    // Extract commit from git
    let commit_record = get_commit_record(
        project_path,
        commit_ref.as_deref(),
        message_override.as_deref(),
        note.as_deref(),
    )?;

    // Check if commit already recorded
    let already_exists_idx = ledger
        .commits
        .iter()
        .position(|c| c.hash == commit_record.hash);

    if let Some(idx) = already_exists_idx {
        println!(
            "{} Commit {} was already recorded in ledger. Updating note/metadata.",
            "ℹ".blue().bold(),
            commit_record.short_hash.yellow()
        );
        ledger.commits[idx] = commit_record.clone();
    } else {
        // Prepend newest commit
        ledger.commits.insert(0, commit_record.clone());
    }

    // Refresh manifests, context, and git status
    ledger.manifests = detect_and_parse_manifests(project_path);
    ledger.context_files = extract_agent_context(project_path);
    ledger.git = Some(inspect_git_status(project_path));
    ledger.updated_at = Utc::now().to_rfc3339();

    save_ledger(&ledger)?;

    println!(
        "\n{} Successfully recorded commit in ledger:",
        "✔".green().bold()
    );
    println!("  Commit:       {} ({})", commit_record.short_hash.yellow(), commit_record.hash.dimmed());
    println!("  Branch:       {}", commit_record.branch.cyan());
    println!("  Author:       {}", commit_record.author);
    println!("  Date:         {}", commit_record.date.dimmed());
    println!("  Message:      {}", commit_record.message);
    if let Some(n) = &commit_record.note {
        println!("  Note:         {}", n.green());
    }
    println!(
        "  Files:        {} changed",
        commit_record.changed_files.len().to_string().cyan()
    );
    println!(
        "  Total Ledger Commits: {}",
        ledger.commits.len().to_string().bold()
    );
    println!();

    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_len {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated.trim_end())
    }
}
