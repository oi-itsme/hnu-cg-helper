use hnu_query::cg::{CgSession, CgToken};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Token 的可序列化表示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub cookie: String,
}

/// 创建登录会话，返回 session 和验证码图片
pub async fn create_session() -> Result<(CgSession, Vec<u8>), CoreError> {
    let session = CgSession::new().await?;
    let captcha = session.captcha_image().to_vec();
    Ok((session, captcha))
}

/// 使用学号、密码和验证码完成登录
pub async fn login(
    session: CgSession,
    stu_id: &str,
    password: &str,
    captcha_code: &str,
) -> Result<CgToken, CoreError> {
    let token = session.login(stu_id, password, captcha_code).await?;
    Ok(token)
}

/// 将 CgToken 序列化为 JSON（用于存储/传输）
///
/// 提取 Cookie header 值，存入可序列化的结构体。
pub fn serialize_token(token: &CgToken) -> Result<serde_json::Value, CoreError> {
    let cookie = token
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if cookie.is_empty() {
        return Err(CoreError::Token("Cookie header missing".into()));
    }

    let data = TokenData { cookie };
    serde_json::to_value(data).map_err(|e| CoreError::Token(e.to_string()))
}

/// 从 JSON 反序列化为 CgToken（从存储/传输中恢复）
///
/// 传入的 JSON 必须包含 `cookie` 字段，格式为 `"JSESSIONID=xxx"`。
pub fn deserialize_token(json: &serde_json::Value) -> Result<CgToken, CoreError> {
    let data: TokenData =
        serde_json::from_value(json.clone()).map_err(|e| CoreError::Token(e.to_string()))?;

    if data.cookie.is_empty() {
        return Err(CoreError::Token("Empty cookie value".into()));
    }

    let mut headers = HeaderMap::new();
    let value = HeaderValue::from_str(&data.cookie)
        .map_err(|e| CoreError::Token(format!("Invalid cookie header: {e}")))?;
    headers.insert(COOKIE, value);

    Ok(CgToken::from_headers_unchecked(headers))
}
