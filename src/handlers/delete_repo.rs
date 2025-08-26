use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::fs;
use crate::models::AppState;

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