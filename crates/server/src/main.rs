mod routes;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use include_dir::{Dir, include_dir};
use state::AppState;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

/// 编译进二进制的适配包（单文件分发；热重载/AI 修复作用于解包副本）
static EMBEDDED_ADAPTERS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../adapters/hnu-cg");

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hnu-cg-helper")
        .join("config.toml")
}

/// 内嵌适配包的内容哈希（用于判断解包副本是否过期）
fn embedded_hash() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for file in EMBEDDED_ADAPTERS.files() {
        file.path().hash(&mut hasher);
        file.contents().hash(&mut hasher);
    }
    hasher.finish()
}

/// 数据目录中的适配包副本：缺失或内嵌版本已更新时重新解包。
/// 同版本内的 AI 修复/热重载改动保留；二进制升级后同步为新副本。
fn ensure_extracted(dir: &PathBuf) -> std::io::Result<()> {
    let stamp = dir.join(".embedded-hash");
    let current = embedded_hash().to_string();
    let up_to_date = dir.join("adapter.toml").is_file()
        && std::fs::read_to_string(&stamp)
            .map(|s| s == current)
            .unwrap_or(false);
    if up_to_date {
        return Ok(());
    }
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    EMBEDDED_ADAPTERS.extract(dir)?;
    std::fs::write(&stamp, current)?;
    tracing::info!("已解包内嵌适配包到 {}", dir.display());
    Ok(())
}

/// 适配包目录解析：环境变量 → 仓库相对路径（开发便利）→ 数据目录解包副本
fn adapters_dir() -> std::io::Result<PathBuf> {
    if let Ok(dir) = std::env::var("HNU_CG_ADAPTERS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let repo_relative = PathBuf::from("adapters/hnu-cg");
    if repo_relative.join("adapter.toml").is_file() {
        return Ok(repo_relative);
    }
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hnu-cg-helper")
        .join("adapters")
        .join("hnu-cg");
    ensure_extracted(&dir)?;
    Ok(dir)
}

/// 失败现场留存目录（用户数据目录，不进仓库）
fn scenes_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hnu-cg-helper")
        .join("failure-scenes")
}

/// Build the frontend serving router with the given application state.
///
/// When `embed-frontend` feature is enabled (default), loads the compiled
/// frontend assets from `frontend/dist/` at compile time and serves them
/// with SPA fallback.
///
/// When the feature is disabled (dev mode), returns an empty router —
/// the Vite dev server at :5173 proxies API calls and serves the frontend.
fn frontend_router(state: AppState) -> Router<AppState> {
    #[cfg(feature = "embed-frontend")]
    {
        tracing::info!("已嵌入前端静态文件");
        memory_serve::load!()
            .index_file(Some("/index.html"))
            .fallback(Some("/index.html"))
            .fallback_status(axum::http::StatusCode::OK)
            .into_router()
            .with_state(state)
    }

    #[cfg(not(feature = "embed-frontend"))]
    {
        tracing::info!("API-only 模式（前端由 Vite 开发服务器提供）");
        Router::new().with_state(state)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hnu_cg_helper_server=debug,info".into()),
        )
        .init();

    let config_path = default_config_path();
    tracing::info!("配置文件路径: {}", config_path.display());

    let adapters_dir = match adapters_dir() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("准备适配包目录失败: {e}");
            std::process::exit(1);
        }
    };
    let adapters = match hnu_cg_helper_adapter::AdapterRegistry::load(&adapters_dir) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("加载站点适配包 {} 失败: {e}", adapters_dir.display());
            std::process::exit(1);
        }
    };
    tracing::info!("已加载站点适配包: {}", adapters_dir.display());

    let state = AppState::new(config_path, adapters, scenes_dir());

    // 适配包文件监听：脚本变更后自动校验热重载（watcher 泄漏存活至进程结束）
    if let Some(watcher) =
        hnu_cg_helper_adapter::registry::watch(state.adapters.clone(), state.engine_cfg.clone())
    {
        Box::leak(Box::new(watcher));
        tracing::info!("适配包热重载监听已启动");
    }

    let port = {
        let config = state.config.read().await;
        config.server_port()
    };

    let cors = CorsLayer::new()
        .allow_origin(["http://localhost:5173".parse().unwrap()])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Auth
        .route("/api/auth/captcha", post(routes::auth::get_captcha))
        .route("/api/auth/login", post(routes::auth::do_login))
        .route("/api/auth/logout", post(routes::auth::logout))
        .route("/api/auth/status", get(routes::auth::auth_status))
        // Courses
        .route("/api/courses", get(routes::course::get_courses))
        .route(
            "/api/courses/{course_id}/assignments",
            get(routes::course::get_assignments),
        )
        .route(
            "/api/courses/{course_id}/assignments/{assign_id}/problems",
            get(routes::course::get_problems),
        )
        .route(
            "/api/courses/{course_id}/assignments/{assign_id}/problems/{pro_num}",
            get(routes::course::get_problem_page),
        )
        .route(
            "/api/courses/{course_id}/assignments/{assign_id}/problems/{pro_num}/submit",
            post(routes::course::submit_problem),
        )
        // Adapter
        .route("/api/adapter/repair", post(routes::course::repair_adapter))
        // AI
        .route("/api/ai/chat", post(routes::ai::chat))
        .route("/api/ai/config", get(routes::ai::get_ai_config))
        .route("/api/ai/config", post(routes::ai::set_ai_config))
        // Frontend (merged before with_state; Router<()> at this point)
        .merge(frontend_router(state.clone()))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!(
        "Server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
