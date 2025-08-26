use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::{
    collections::HashMap,
    process::Command,
    sync::Arc,
};
use crate::models::AppState;

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