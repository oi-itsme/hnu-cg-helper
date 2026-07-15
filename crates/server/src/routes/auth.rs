use axum::{Json, extract::State, http::StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hnu_cg_helper_core::auth as core_auth;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct AuthStatusResponse {
    pub authenticated: bool,
}

/// GET /api/auth/status
///
/// 检查是否已登录（token 存在且有效）。
pub async fn auth_status(
    State(state): State<AppState>,
) -> Json<AuthStatusResponse> {
    let token = state.current_token.read().await;
    Json(AuthStatusResponse {
        authenticated: token.is_some(),
    })
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
    let (session, captcha_bytes) = core_auth::create_session().await.map_err(|e| {
        tracing::error!(error = %e, "获取验证码失败");
        (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
    })?;

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
    pub success: bool,
}

/// POST /api/auth/login
///
/// 使用验证码完成登录，token 保存在服务器内存中。
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
        .map_err(|e| {
            tracing::error!(error = %e, "登录失败");
            (StatusCode::UNAUTHORIZED, Json((&e).into()))
        })?;

    // 保存到服务器内存
    *state.current_token.write().await = Some(token);

    Ok(Json(LoginResponse { success: true }))
}

/// POST /api/auth/logout
///
/// 清除当前登录状态。
pub async fn logout(
    State(state): State<AppState>,
) -> Json<LoginResponse> {
    *state.current_token.write().await = None;
    Json(LoginResponse { success: true })
}
