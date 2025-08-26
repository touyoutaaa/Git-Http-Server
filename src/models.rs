use std::path::PathBuf;
use serde::Serialize;

pub struct AppState {
    pub git_root : PathBuf
}

// 定义仓库信息的结构体
#[derive(Serialize)]
pub struct RepoInfo {
    pub name: String,
}

// 定义仓库详细信息的结构体
#[derive(Serialize)]
pub struct RepoDetail {
    pub name: String,
    pub created_at: String,
    pub last_commit: String,
    pub branch_count: i32,
    pub commit_count: i32,
}