use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub vault_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusRequest {
    pub config: GitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusResponse {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub ahead: i32,
    pub behind: i32,
    pub modified: Vec<FileStatus>,
    pub staged: Vec<FileStatus>,
    pub untracked: Vec<FileStatus>,
    pub has_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitRequest {
    pub config: GitConfig,
    pub message: String,
    pub add_all: Option<bool>,
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitResponse {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub message: String,
    pub files_committed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPushRequest {
    pub config: GitConfig,
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullRequest {
    pub config: GitConfig,
    pub rebase: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSyncRequest {
    pub config: GitConfig,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationResponse {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogRequest {
    pub config: GitConfig,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogResponse {
    pub entries: Vec<GitLogEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffRequest {
    pub config: GitConfig,
    pub file: Option<String>,
    pub staged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffResponse {
    pub diff: String,
    pub files_changed: usize,
    pub insertions: i32,
    pub deletions: i32,
}

fn run_git_command(vault_path: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(vault_path)
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr)
    }
}

fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

pub fn git_status(request: &GitStatusRequest) -> Result<GitStatusResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        return Ok(GitStatusResponse {
            is_repo: false,
            branch: None,
            ahead: 0,
            behind: 0,
            modified: vec![],
            staged: vec![],
            untracked: vec![],
            has_changes: false,
        });
    }

    let branch = run_git_command(vault_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());

    let (ahead, behind) = get_ahead_behind(vault_path);

    let status_output = run_git_command(vault_path, &["status", "--porcelain=v1"])?;

    let mut modified = vec![];
    let mut staged = vec![];
    let mut untracked = vec![];

    for line in status_output.lines() {
        if line.len() < 3 {
            continue;
        }

        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let file_path = line[3..].to_string();

        if index_status != ' ' && index_status != '?' {
            staged.push(FileStatus {
                path: file_path.clone(),
                status: status_char_to_string(index_status),
                staged: true,
            });
        }

        if worktree_status == 'M' || worktree_status == 'D' {
            modified.push(FileStatus {
                path: file_path.clone(),
                status: status_char_to_string(worktree_status),
                staged: false,
            });
        }

        if index_status == '?' {
            untracked.push(FileStatus {
                path: file_path,
                status: "untracked".to_string(),
                staged: false,
            });
        }
    }

    let has_changes = !modified.is_empty() || !staged.is_empty() || !untracked.is_empty();

    Ok(GitStatusResponse {
        is_repo: true,
        branch,
        ahead,
        behind,
        modified,
        staged,
        untracked,
        has_changes,
    })
}

fn get_ahead_behind(vault_path: &str) -> (i32, i32) {
    let output = run_git_command(
        vault_path,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    );

    match output {
        Ok(s) => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().unwrap_or(0);
                let behind = parts[1].parse().unwrap_or(0);
                (ahead, behind)
            } else {
                (0, 0)
            }
        }
        Err(_) => (0, 0),
    }
}

fn status_char_to_string(c: char) -> String {
    match c {
        'M' => "modified".to_string(),
        'A' => "added".to_string(),
        'D' => "deleted".to_string(),
        'R' => "renamed".to_string(),
        'C' => "copied".to_string(),
        'U' => "unmerged".to_string(),
        '?' => "untracked".to_string(),
        '!' => "ignored".to_string(),
        _ => "unknown".to_string(),
    }
}

pub fn git_commit(request: &GitCommitRequest) -> Result<GitCommitResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    if request.add_all.unwrap_or(false) {
        run_git_command(vault_path, &["add", "-A"])?;
    } else if let Some(ref files) = request.files {
        for file in files {
            run_git_command(vault_path, &["add", file])?;
        }
    }

    let status = run_git_command(vault_path, &["status", "--porcelain"])?;
    let staged_count = status
        .lines()
        .filter(|l| {
            l.len() >= 2
                && l.chars().next().unwrap_or(' ') != ' '
                && l.chars().next().unwrap_or(' ') != '?'
        })
        .count();

    if staged_count == 0 {
        return Ok(GitCommitResponse {
            success: false,
            commit_hash: None,
            message: "Nothing to commit".to_string(),
            files_committed: 0,
        });
    }

    run_git_command(vault_path, &["commit", "-m", &request.message])?;

    let hash = run_git_command(vault_path, &["rev-parse", "--short", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());

    Ok(GitCommitResponse {
        success: true,
        commit_hash: hash,
        message: format!("Committed: {}", request.message),
        files_committed: staged_count,
    })
}

pub fn git_push(request: &GitPushRequest) -> Result<GitOperationResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    let mut args = vec!["push"];
    if request.force.unwrap_or(false) {
        args.push("--force");
    }

    match run_git_command(vault_path, &args) {
        Ok(output) => Ok(GitOperationResponse {
            success: true,
            message: "Push successful".to_string(),
            details: Some(output),
        }),
        Err(e) => Ok(GitOperationResponse {
            success: false,
            message: format!("Push failed: {}", e),
            details: None,
        }),
    }
}

pub fn git_pull(request: &GitPullRequest) -> Result<GitOperationResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    let mut args = vec!["pull"];
    if request.rebase.unwrap_or(false) {
        args.push("--rebase");
    }

    match run_git_command(vault_path, &args) {
        Ok(output) => Ok(GitOperationResponse {
            success: true,
            message: "Pull successful".to_string(),
            details: Some(output),
        }),
        Err(e) => Ok(GitOperationResponse {
            success: false,
            message: format!("Pull failed: {}", e),
            details: None,
        }),
    }
}

pub fn git_sync(request: &GitSyncRequest) -> Result<GitOperationResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    let pull_result = git_pull(&GitPullRequest {
        config: request.config.clone(),
        rebase: Some(true),
    })?;

    if !pull_result.success {
        return Ok(GitOperationResponse {
            success: false,
            message: format!("Sync failed during pull: {}", pull_result.message),
            details: pull_result.details,
        });
    }

    let status = git_status(&GitStatusRequest {
        config: request.config.clone(),
    })?;

    if !status.has_changes {
        return Ok(GitOperationResponse {
            success: true,
            message: "Already up to date, no local changes".to_string(),
            details: None,
        });
    }

    let message = request.message.clone().unwrap_or_else(|| {
        format!(
            "Vault sync: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    });

    let commit_result = git_commit(&GitCommitRequest {
        config: request.config.clone(),
        message,
        add_all: Some(true),
        files: None,
    })?;

    if !commit_result.success {
        return Ok(GitOperationResponse {
            success: false,
            message: "Sync failed: nothing to commit".to_string(),
            details: None,
        });
    }

    let push_result = git_push(&GitPushRequest {
        config: request.config.clone(),
        force: None,
    })?;

    if !push_result.success {
        return Ok(GitOperationResponse {
            success: false,
            message: format!("Sync failed during push: {}", push_result.message),
            details: push_result.details,
        });
    }

    Ok(GitOperationResponse {
        success: true,
        message: format!(
            "Sync complete: {} files committed and pushed",
            commit_result.files_committed
        ),
        details: Some(format!(
            "Commit: {}",
            commit_result.commit_hash.unwrap_or_default()
        )),
    })
}

pub fn git_log(request: &GitLogRequest) -> Result<GitLogResponse> {
    let vault_path = &request.config.vault_path;
    let limit = request.limit.unwrap_or(10);

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    let format = "--format=%H|%h|%an|%ai|%s";
    let limit_arg = format!("-{}", limit);

    let output = run_git_command(vault_path, &["log", &limit_arg, format])?;

    let entries: Vec<GitLogEntry> = output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 5 {
                Some(GitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4..].join("|"),
                })
            } else {
                None
            }
        })
        .collect();

    let total = entries.len();
    Ok(GitLogResponse { entries, total })
}

pub fn git_diff(request: &GitDiffRequest) -> Result<GitDiffResponse> {
    let vault_path = &request.config.vault_path;

    if !is_git_repo(vault_path) {
        anyhow::bail!("Not a git repository");
    }

    let mut args = vec!["diff"];
    if request.staged.unwrap_or(false) {
        args.push("--cached");
    }
    args.push("--stat");

    if let Some(ref file) = request.file {
        args.push("--");
        args.push(file);
    }

    let stat_output = run_git_command(vault_path, &args)?;

    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for line in stat_output.lines() {
        if line.contains("file changed") || line.contains("files changed") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if (*part == "insertion" || *part == "insertions" || part.starts_with("insertion"))
                    && i > 0
                {
                    insertions = parts[i - 1].parse().unwrap_or(0);
                }
                if (*part == "deletion" || *part == "deletions" || part.starts_with("deletion"))
                    && i > 0
                {
                    deletions = parts[i - 1].parse().unwrap_or(0);
                }
                if part.ends_with("changed") && i > 0 {
                    files_changed = parts[i - 1].parse().unwrap_or(0);
                }
            }
        }
    }

    let mut diff_args = vec!["diff"];
    if request.staged.unwrap_or(false) {
        diff_args.push("--cached");
    }
    if let Some(ref file) = request.file {
        diff_args.push("--");
        diff_args.push(file);
    }

    let diff = run_git_command(vault_path, &diff_args)?;

    Ok(GitDiffResponse {
        diff,
        files_changed,
        insertions,
        deletions,
    })
}

pub fn git_init(config: &GitConfig) -> Result<GitOperationResponse> {
    let vault_path = &config.vault_path;

    if is_git_repo(vault_path) {
        return Ok(GitOperationResponse {
            success: false,
            message: "Already a git repository".to_string(),
            details: None,
        });
    }

    match run_git_command(vault_path, &["init"]) {
        Ok(output) => Ok(GitOperationResponse {
            success: true,
            message: "Git repository initialized".to_string(),
            details: Some(output),
        }),
        Err(e) => Ok(GitOperationResponse {
            success: false,
            message: format!("Failed to initialize: {}", e),
            details: None,
        }),
    }
}
