use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::{
    process::Stdio,
    sync::Arc,
};
use tokio::io::AsyncWriteExt;
use crate::models::AppState;

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
    let mut child = match tokio::process::Command::new("git")
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
        if let Err(e) = stdin.write_all(body.as_ref()).await {
            eprintln!("Error writing to git stdin: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write to git command stdin").into_response();
        }
        // 确保关闭stdin，这很重要
        drop(stdin);
    }

    // 读取git命令的stdout
    let output = match child.wait_with_output().await {
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
        if let Err(e) = std::process::Command::new("git")
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