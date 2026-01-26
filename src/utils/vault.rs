use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSaveRequest {
    pub vault_path: String,
    pub file_path: String,
    pub content: String,
    pub overwrite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSaveResponse {
    pub success: bool,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultReadRequest {
    pub vault_path: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultReadResponse {
    pub content: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultListRequest {
    pub vault_path: String,
    pub folder: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultFileInfo {
    pub name: String,
    pub path: String,
    pub is_folder: bool,
    pub size: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultListResponse {
    pub files: Vec<VaultFileInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDeleteRequest {
    pub vault_path: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMoveRequest {
    pub vault_path: String,
    pub from_path: String,
    pub to_path: String,
}

fn sanitize_path(vault_path: &str, file_path: &str) -> Result<PathBuf> {
    let vault = PathBuf::from(vault_path);
    let file = PathBuf::from(file_path);

    if file.is_absolute() || file_path.contains("..") {
        anyhow::bail!("Invalid file path: must be relative and cannot contain '..'");
    }

    let full_path = vault.join(&file);

    if !full_path.starts_with(&vault) {
        anyhow::bail!("Path traversal detected");
    }

    Ok(full_path)
}

pub fn save_file(request: &VaultSaveRequest) -> Result<VaultSaveResponse> {
    let full_path = sanitize_path(&request.vault_path, &request.file_path)?;

    if full_path.exists() && !request.overwrite.unwrap_or(false) {
        return Ok(VaultSaveResponse {
            success: false,
            path: request.file_path.clone(),
            message: "File already exists. Set overwrite=true to replace.".to_string(),
        });
    }

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&full_path, &request.content)?;

    Ok(VaultSaveResponse {
        success: true,
        path: request.file_path.clone(),
        message: "File saved successfully".to_string(),
    })
}

pub fn read_file(request: &VaultReadRequest) -> Result<VaultReadResponse> {
    let full_path = sanitize_path(&request.vault_path, &request.file_path)?;

    if !full_path.exists() {
        return Ok(VaultReadResponse {
            content: String::new(),
            path: request.file_path.clone(),
            exists: false,
        });
    }

    let content = std::fs::read_to_string(&full_path)?;

    Ok(VaultReadResponse {
        content,
        path: request.file_path.clone(),
        exists: true,
    })
}

pub fn list_files(request: &VaultListRequest) -> Result<VaultListResponse> {
    let vault = PathBuf::from(&request.vault_path);
    let folder = request.folder.as_deref().unwrap_or("");
    let target = if folder.is_empty() {
        vault.clone()
    } else {
        vault.join(folder)
    };

    if !target.exists() || !target.is_dir() {
        return Ok(VaultListResponse {
            files: vec![],
            total: 0,
        });
    }

    let mut files = Vec::new();

    for entry in std::fs::read_dir(&target)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if name.starts_with('.') {
            continue;
        }

        if let Some(ref ext) = request.extension {
            if !metadata.is_dir() {
                if let Some(file_ext) = path.extension() {
                    if file_ext.to_str().unwrap_or("") != ext.trim_start_matches('.') {
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }

        let relative_path = path
            .strip_prefix(&vault)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let modified = metadata
            .modified()
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            })
            .unwrap_or(0);

        files.push(VaultFileInfo {
            name,
            path: relative_path,
            is_folder: metadata.is_dir(),
            size: metadata.len(),
            modified,
        });
    }

    files.sort_by(|a, b| {
        if a.is_folder != b.is_folder {
            b.is_folder.cmp(&a.is_folder)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    let total = files.len();
    Ok(VaultListResponse { files, total })
}

pub fn delete_file(request: &VaultDeleteRequest) -> Result<bool> {
    let full_path = sanitize_path(&request.vault_path, &request.file_path)?;

    if !full_path.exists() {
        return Ok(false);
    }

    if full_path.is_dir() {
        std::fs::remove_dir_all(&full_path)?;
    } else {
        std::fs::remove_file(&full_path)?;
    }

    Ok(true)
}

pub fn move_file(request: &VaultMoveRequest) -> Result<bool> {
    let from = sanitize_path(&request.vault_path, &request.from_path)?;
    let to = sanitize_path(&request.vault_path, &request.to_path)?;

    if !from.exists() {
        return Ok(false);
    }

    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&from, &to)?;
    Ok(true)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSearchRequest {
    pub vault_path: String,
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSearchResult {
    pub path: String,
    pub name: String,
    pub snippet: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSearchResponse {
    pub results: Vec<VaultSearchResult>,
    pub total: usize,
}

pub fn search_vault(request: &VaultSearchRequest) -> Result<VaultSearchResponse> {
    let vault = PathBuf::from(&request.vault_path);
    let query_lower = request.query.to_lowercase();
    let limit = request.limit.unwrap_or(50);

    let mut results = Vec::new();

    fn search_dir(
        dir: &Path,
        vault: &Path,
        query: &str,
        results: &mut Vec<VaultSearchResult>,
        limit: usize,
    ) -> Result<()> {
        if results.len() >= limit {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            if results.len() >= limit {
                break;
            }

            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') {
                    search_dir(&path, vault, query, results, limit)?;
                }
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(query) {
                            let relative_path = path
                                .strip_prefix(vault)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .to_string();

                            let name = path
                                .file_stem()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();

                            let snippet = if line.len() > 100 {
                                format!("{}...", &line[..100])
                            } else {
                                line.to_string()
                            };

                            results.push(VaultSearchResult {
                                path: relative_path,
                                name,
                                snippet,
                                line_number: line_num + 1,
                            });

                            if results.len() >= limit {
                                return Ok(());
                            }

                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    search_dir(&vault, &vault, &query_lower, &mut results, limit)?;

    let total = results.len();
    Ok(VaultSearchResponse { results, total })
}
