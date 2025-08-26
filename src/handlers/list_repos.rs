use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::{
    fs as std_fs,
    sync::Arc,
};
use crate::models::{AppState, RepoInfo};

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
        let entry: std_fs::DirEntry = match entry_result {
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