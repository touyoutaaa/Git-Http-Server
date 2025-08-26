// 导出所有处理函数
mod list_repos;
mod info_refs;
mod rpc;
mod create_repo;
mod delete_repo;
mod get_repo_detail;

// 重新导出所有处理函数，使它们可以从handlers模块直接访问
pub use list_repos::list_repos;
pub use info_refs::info_refs;
pub use rpc::rpc;
pub use create_repo::create_repo;
pub use delete_repo::delete_repo;
pub use get_repo_detail::get_repo_detail;