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

use crate::models::AppState;

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
