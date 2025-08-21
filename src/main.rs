use std::sync::Arc;
use axum::Router;
use axum::routing::{delete, get, post};
use crate::handlers::{create_repo, delete_repo, info_refs, rpc};
use crate::models::AppState;
use tokio::signal;
mod models;
mod handlers;
mod test;

#[tokio::main]
async fn main() {
    //初始化日志
    tracing_subscriber::fmt::init();

    let current_dir = std::env::current_dir().expect("Failed to get current directory");
   
    let git_root = current_dir.join("git_repo"); // 目前git_root为 git-server/git_repo
    if !git_root.exists() {
        std::fs::create_dir_all(&git_root).expect("创建临时目录失败");
    }
    let state = Arc::new(AppState {
        git_root,
    });
    let app: Router = Router::new()
        .route("/:repo_name/info/refs", get(info_refs))
        .route("/:repo_name/:command", post(rpc))
        .route("/:repo_name", get(create_repo))
        .route("/:repo_name", delete(delete_repo))
        .with_state(state);
    
    tracing::debug!("启动服务");
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("listening on {}", addr);
    
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());
     
    let graceful = server.with_graceful_shutdown(shutdown_signal());
    tracing::info!("服务器启动，按Ctrl+C退出");
    // 启动服务器并等待其完成
    if let Err(e) = graceful.await {
        tracing::error!("服务器错误: {}", e);
    }
    tracing::info!("服务器已优雅退出");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("无法安装Ctrl+C处理器");
        tracing::info!("接收到Ctrl+C信号，开始优雅退出...");
    };
    let terminate = std::future::pending::<()>();

    // 等待任一信号
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
