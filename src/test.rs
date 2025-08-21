#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;
    use crate::handlers::info_refs;
    use crate::models::AppState;

    #[tokio::test]
    async fn test_info_refs() {
        // 创建测试状态
        let temp_dir = tempfile::tempdir().unwrap();
        let git_root = temp_dir.path().to_path_buf();
        let state = Arc::new(AppState { git_root: git_root.clone() });

        // 创建测试仓库
        let repo_name = "test-repo";
        let repo_path = git_root.join(repo_name);
        std::process::Command::new("git")
            .args(&["init", "--bare", repo_path.to_str().unwrap()])
            .output()
            .expect("Failed to create test repository");

        // 创建测试路由
        let app = Router::new()
            .route("/:reponame/info/refs", get(info_refs))
            .with_state(state);

        // 创建测试请求
        let request = Request::builder()
            .uri(format!("/{}/info/refs?service=git-upload-pack", repo_name))
            .body(Body::empty())
            .unwrap();

        // 发送请求并获取响应
        let response = app.oneshot(request).await.unwrap();

        // 验证响应状态码
        assert_eq!(response.status(), StatusCode::OK);

        // 验证响应头
        let headers = response.headers();
        assert_eq!(
            headers.get("Content-Type").unwrap(),
            "application/x-git-upload-pack-advertisement"
        );

        // 可以进一步验证响应体的内容
        // ...
    }
}
