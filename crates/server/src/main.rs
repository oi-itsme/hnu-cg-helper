mod routes;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use state::AppState;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hnu-cg-helper")
        .join("config.toml")
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

    let state = AppState::new(config_path);

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
