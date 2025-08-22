use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};
use tokio::fs;

use axum::Json;
use serde::Serialize;
use std::fs as std_fs;
use crate::models::AppState;

// 定义仓库信息的结构体
#[derive(Serialize)]
pub struct RepoInfo {
    name: String,
}

// 定义仓库详细信息的结构体
#[derive(Serialize)]
pub struct RepoDetail {
    name: String,
    created_at: String,
    last_commit: String,
    branch_count: i32,
    commit_count: i32,
}

// 获取仓库列表
pub async fn list_repos(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let git_root = &state.git_root;

    // 使用标准库的fs模块读取目录
    let entries = match std_fs::read_dir(git_root) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading git root directory: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<RepoInfo>::new())).into_response();
        }
    };

    // 收集所有仓库信息
    let mut repos = Vec::new();

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let path = entry.path();

        // 检查是否是目录
        let is_dir = match std_fs::metadata(&path) {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => continue,
        };

        if is_dir {
            // 检查是否是Git仓库（包含HEAD文件）
            let head_file = path.join("HEAD");
            if head_file.exists() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        repos.push(RepoInfo {
                            name: name_str.to_string(),
                        });
                    }
                }
            }
        }
    }

    // 返回JSON格式的仓库列表
    (StatusCode::OK, Json(repos)).into_response()
}
// 处理 info/refs 请求
pub async fn info_refs(
    Path(repo_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let service = params.get("service").cloned().unwrap_or_default();
    if service.is_empty() {
        return (StatusCode::BAD_REQUEST, "Invalid request").into_response();
    }

    let repo_path = state.git_root.join(&repo_name);

    // 检查仓库是否存在
    if !repo_path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    // 执行git命令获取refs信息
    let command = service.strip_prefix("git-").unwrap_or(&service);
    let output = match Command::new("git")
        .arg(command)
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&repo_path)
        .output() {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Error executing git command: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    if !output.status.success() {
        eprintln!("Git command failed: {}", String::from_utf8_lossy(&output.stderr));
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
    }

    // 构建响应
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        format!("application/x-{}-advertisement", service).parse().unwrap(),
    );
    
    // 构建正确的Git协议响应
    let server_advert = format!("# service={}\n", service);
    let length = server_advert.len() + 4; // 包括结尾的0000
    let prefix = format!("{:04x}{}", length, server_advert);
    
    let mut body = Vec::new();
    body.extend_from_slice(prefix.as_bytes());
    body.extend_from_slice(b"0000"); // 分隔符
    body.extend_from_slice(&output.stdout);

    (headers, body).into_response()
}

// 处理 RPC 请求
pub async fn rpc(
    Path((repo_name, command)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let repo_path = state.git_root.join(&repo_name);

    // 检查仓库是否存在
    if !repo_path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }
    
    // git-receive-back => receive-pack
    let git_command = if command.starts_with("git-") {
        command.strip_prefix("git-").unwrap_or(&command)
    } else {
        &command
    };

    // 执行git命令
    let mut child = match Command::new("git")
        .arg(git_command)
        .arg("--stateless-rpc") // 使用无状态RPC
        .arg(&repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("Error spawning git command: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to execute git command: {}", e)).into_response();
        }
    };

    // 写入请求体到git命令的stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(body.as_ref()) {
            eprintln!("Error writing to git stdin: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write to git command stdin").into_response();
        }
        // 确保关闭stdin，这很重要
        drop(stdin);
    }

    // 读取git命令的stdout
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Error waiting for git command: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read from git command stdout").into_response();
        }
    };

    if !output.status.success() {
        eprintln!("Git command failed: {}", String::from_utf8_lossy(&output.stderr));
        return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
    }

    // 如果是receive-pack命令，需要更新服务器信息
    if command == "receive-pack" || command == "git-receive-pack" {
        if let Err(e) = Command::new("git")
            .arg("--git-dir")
            .arg(&repo_path)
            .arg("update-server-info")
            .output() {
            eprintln!("Failed to update server info: {}", e);
        }
    }

    // 构建响应
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        format!("application/x-git-{}-result", command).parse().unwrap(),
    );

    // 直接返回git命令的输出，不添加任何额外字符
    (headers, output.stdout).into_response()
}

// 创建仓库
pub async fn create_repo(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let repo_name = match params.get("name") {
        Some(name) => name,
        None => return (StatusCode::BAD_REQUEST, "Repository name is required").into_response(),
    };

    let repo_path = state.git_root.join(repo_name);

    // 检查仓库是否已存在
    if repo_path.exists() {
        return (StatusCode::CONFLICT, "Repository already exists").into_response();
    }

    // 创建裸仓库
    let output = Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&repo_path)
        .output()
        .expect("Failed to create repository");

    if output.status.success() {
        (StatusCode::CREATED, "Repository created successfully").into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create repository").into_response()
    }
}

// 删除仓库
pub async fn delete_repo(
    Path(repo_name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let repo_path = state.git_root.join(&repo_name);

    // 检查仓库是否存在
    if !repo_path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    // 删除仓库
    match fs::remove_dir_all(repo_path).await {
        Ok(_) => (StatusCode::OK, "Repository deleted successfully").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete repository").into_response(),
    }
}

pub async fn get_repo_detail(
    Path(repo_name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let repo_path = state.git_root.join(&repo_name);

    // 检查仓库是否存在
    if !repo_path.exists() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Repository not found"
        }))).into_response();
    }

    // 检查是否是Git仓库
    let head_file = repo_path.join("HEAD");
    if !fs::try_exists(&head_file).await.unwrap_or(false) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Not a valid Git repository"
        }))).into_response();
    }

    // 获取创建时间（使用目录的创建时间作为近似值）
    let created_at = {
        let metadata = std_fs::metadata(&repo_path);
        match metadata {
            Ok(metadata) => {
                if let Ok(created) = metadata.created() {
                    if let Ok(time) = created.duration_since(std::time::UNIX_EPOCH) {
                        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(
                            time.as_secs() as i64, 0
                        );
                        if let Some(dt) = datetime {
                            dt.format("%Y-%m-%d").to_string()
                        } else {
                            "未知".to_string()
                        }
                    } else {
                        "未知".to_string()
                    }
                } else {
                    "未知".to_string()
                }
            },
            Err(_) => "未知".to_string(),
        }
    };

    // 获取最后提交时间（使用git命令）
    let last_commit = {
        let output = Command::new("git")
            .arg("--git-dir")
            .arg(&repo_path)
            .arg("log")
            .arg("-1")
            .arg("--format=%cd")
            .arg("--date=format:%Y-%m-%d")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            },
            _ => "未知".to_string(),
        }
    };

    // 获取分支数量
    let branch_count = {
        let output = Command::new("git")
            .arg("--git-dir")
            .arg(&repo_path)
            .arg("branch")
            .arg("--list")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let branches = String::from_utf8_lossy(&output.stdout);
                branches.lines().count() as i32
            },
            _ => 0,
        }
    };

    // 获取提交数量
    let commit_count = {
        let output = Command::new("git")
            .arg("--git-dir")
            .arg(&repo_path)
            .arg("rev-list")
            .arg("--count")
            .arg("--all")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let count = stdout.trim();
                count.parse::<i32>().unwrap_or(0)
            },
            _ => 0,
        }
    };

    // 构建仓库详情
    let repo_detail = RepoDetail {
        name: repo_name,
        created_at,
        last_commit,
        branch_count,
        commit_count,
    };

    // 返回JSON格式的仓库详情
    (StatusCode::OK, Json(repo_detail)).into_response()
}