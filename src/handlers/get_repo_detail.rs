use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::{
    process::Command,
    fs as std_fs,
    sync::Arc,
};
use tokio::fs;
use crate::models::{AppState, RepoDetail};

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