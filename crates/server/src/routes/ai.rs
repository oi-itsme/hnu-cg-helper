use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use hnu_cg_helper_core::ai::{ChatChunk, ChatMessage, ChatRequest};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::state::AppState;

// ── Client request types ─────────────────────────────────────────────

/// Chat request from the frontend — only `messages` is required.
/// The server fills in `api_key`, `base_url`, and `model` from stored config.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatRequestBody {
    pub messages: Vec<ChatMessage>,
}

/// Request body for saving AI config.
#[derive(Debug, Deserialize)]
pub(crate) struct SetAiConfigRequest {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Response for AI config query.
#[derive(Debug, Serialize)]
pub(crate) struct AiConfigResponse {
    pub has_api_key: bool,
    pub base_url: String,
    pub model: String,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /api/ai/chat
///
/// 流式 AI 聊天端点，返回 SSE 流。
/// 客户端只传 `messages`，API key 等由服务端从加密配置中读取。
pub(crate) async fn chat(
    State(state): State<AppState>,
    Json(body): Json<ChatRequestBody>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, axum::Error>>>, StatusCode> {
    // Read AI config from state
    let config = state.config.read().await;
    let api_key = config.ai_api_key().ok_or_else(|| {
        tracing::warn!("AI chat requested but no API key configured");
        StatusCode::BAD_REQUEST
    })?;
    let view = config.ai_config_view();
    drop(config); // release the read lock before the async task

    let request = ChatRequest {
        api_key,
        base_url: view.base_url,
        model: view.model,
        messages: body.messages,
    };

    let (tx, rx) = mpsc::channel::<ChatChunk>(64);

    tokio::spawn(async move {
        if let Err(e) = hnu_cg_helper_core::stream_chat(request, tx).await {
            tracing::error!("AI chat error: {e}");
        }
    });

    let stream = ReceiverStream::new(rx).map(|chunk| {
        let data = serde_json::to_string(&chunk).map_err(|e| {
            tracing::error!("Failed to serialize chunk: {e}");
            axum::Error::new(std::io::Error::other(e))
        })?;
        Ok(Event::default().data(data))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// GET /api/ai/config
///
/// 查询当前 AI 配置。API key 使用 `has_api_key` 布尔值表示，
/// 不会以明文形式返回给前端。
pub(crate) async fn get_ai_config(State(state): State<AppState>) -> Json<AiConfigResponse> {
    let config = state.config.read().await;
    let view = config.ai_config_view();

    Json(AiConfigResponse {
        has_api_key: view.has_api_key,
        base_url: view.base_url,
        model: view.model,
    })
}

/// POST /api/ai/config
///
/// 保存 AI 配置。仅更新请求体中提供的字段。
pub(crate) async fn set_ai_config(
    State(state): State<AppState>,
    Json(body): Json<SetAiConfigRequest>,
) -> Result<Json<AiConfigResponse>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    {
        let mut config = state.config.write().await;

        // If this environment has no keyring and the user tried to set an API key,
        // warn but still process the non-secret fields.
        if !config.keyring_available && body.api_key.is_some() {
            tracing::warn!("OS 密钥环不可用，API Key 无法持久化，本次会话内将使用内存中的值");
        }

        let api_key = body.api_key.as_deref();
        // filter out empty strings to avoid clearing the key unintentionally
        let api_key = api_key.filter(|k| !k.is_empty());

        config
            .set_ai_config(api_key, body.base_url.as_deref(), body.model.as_deref())
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(hnu_cg_helper_core::error::ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;

        config.save_to_disk().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(hnu_cg_helper_core::error::ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    }

    // Return the updated config (acquire a fresh read lock)
    let config = state.config.read().await;
    let view = config.ai_config_view();
    Ok(Json(AiConfigResponse {
        has_api_key: view.has_api_key,
        base_url: view.base_url,
        model: view.model,
    }))
}
