use axum::{Json, extract::State, http::StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hnu_cg_helper_core::CgToken;
use hnu_cg_helper_core::auth as core_auth;
use hnu_cg_helper_core::error::CoreError;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// 从 Authorization header 提取并还原 CgToken
pub fn extract_token(auth_header: &str) -> Result<CgToken, CoreError> {
    let token_str = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| CoreError::Token("Missing Bearer prefix".into()))?;

    let decoded = BASE64
        .decode(token_str)
        .map_err(|e| CoreError::Token(format!("Base64 decode failed: {e}")))?;

    let json: serde_json::Value = serde_json::from_slice(&decoded)
        .map_err(|e| CoreError::Token(format!("JSON parse failed: {e}")))?;

    core_auth::deserialize_token(&json)
}

#[derive(Serialize)]
pub(crate) struct CaptchaResponse {
    session_id: String,
    captcha_image: String, // base64-encoded PNG
}

/// POST /api/auth/captcha
///
/// 创建登录会话，返回验证码图片（base64 编码的 PNG）
pub async fn get_captcha(
    State(state): State<AppState>,
) -> Result<Json<CaptchaResponse>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let (session, captcha_bytes) = core_auth::create_session()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into())))?;

    let session_id = state.store_session(session).await;

    Ok(Json(CaptchaResponse {
        session_id,
        captcha_image: BASE64.encode(&captcha_bytes),
    }))
}

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    pub session_id: String,
    pub stu_id: String,
    pub password: String,
    pub captcha_code: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    pub token: String, // base64-encoded token JSON
}

/// POST /api/auth/login
///
/// 使用验证码完成登录，返回可缓存的 token。
pub async fn do_login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let session = state.take_session(&req.session_id).await.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: "Session not found or expired".into(),
            }),
        )
    })?;

    let token = core_auth::login(session, &req.stu_id, &req.password, &req.captcha_code)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json((&e).into())))?;

    let token_json = core_auth::serialize_token(&token)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into())))?;

    let encoded = BASE64.encode(serde_json::to_string(&token_json).unwrap());

    Ok(Json(LoginResponse { token: encoded }))
}
