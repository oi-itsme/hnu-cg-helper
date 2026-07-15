use hnu_query::Error as HnuError;
use hnu_query::cg::error::{CgError, LoginError};
use serde::Serialize;

/// Core crate 统一错误类型
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    /// hnu_query 库错误
    #[error("CG 系统错误: {0}")]
    Cg(#[from] HnuError<CgError>),

    /// 登录错误
    #[error("登录失败: {0}")]
    Login(#[from] HnuError<LoginError>),

    /// AI API 错误
    #[error("AI 服务错误: {0}")]
    Ai(String),

    /// 网络请求错误
    #[error("网络错误: {0}")]
    Network(#[from] reqwest::Error),

    /// Token 序列化/反序列化错误
    #[error("Token 数据错误: {0}")]
    Token(String),

    /// 配置相关错误
    #[error("配置错误: {0}")]
    Config(String),

    /// 密钥环错误
    #[error("密钥环错误: {0}")]
    Keyring(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

/// API 错误响应体
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl From<&CoreError> for ErrorResponse {
    fn from(err: &CoreError) -> Self {
        ErrorResponse {
            error: err.to_string(),
        }
    }
}
