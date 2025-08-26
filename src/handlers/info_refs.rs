use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::{
    collections::HashMap,
    process::Command,
    sync::Arc,
};
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