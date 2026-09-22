use hnu_cg_helper_adapter::{AdapterRegistry, EngineConfig};
use hnu_cg_helper_core::{CgSession, CgToken, ConfigManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 待登录的会话信息
pub struct PendingSession {
    #[allow(dead_code)]
    pub created_at: std::time::Instant,
    /// session 直接存储原始结构以便后续 login 消费
    pub session: Option<CgSession>,
}

/// 应用全局状态
#[derive(Clone)]
pub struct AppState {
    /// 待登录的 session 映射 (session_id → PendingSession)
    pub pending_sessions: Arc<RwLock<HashMap<String, PendingSession>>>,
    /// 当前登录的 CG token（进程内存，重启丢失）
    pub current_token: Arc<RwLock<Option<CgToken>>>,
    /// 当前登录的学号（管线脱敏的已知隐私值）
    pub current_stu_id: Arc<RwLock<Option<String>>>,
    /// 全局配置管理器
    pub config: Arc<RwLock<ConfigManager>>,
    /// 站点适配器注册表（热重载）
    pub adapters: Arc<AdapterRegistry>,
    /// 脚本引擎限额配置
    pub engine_cfg: EngineConfig,
    /// 失败现场留存目录
    pub scenes_dir: PathBuf,
}

impl AppState {
    pub fn new(config_path: PathBuf, adapters: AdapterRegistry, scenes_dir: PathBuf) -> Self {
        let config_manager =
            ConfigManager::load(config_path).expect("Failed to load config manager");

        if !config_manager.keyring_available {
            tracing::warn!("OS 密钥环不可用，API Key 等凭据将需要每次手动输入");
        }

        Self {
            pending_sessions: Arc::new(RwLock::new(HashMap::new())),
            current_token: Arc::new(RwLock::new(None)),
            current_stu_id: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(config_manager)),
            adapters: Arc::new(adapters),
            engine_cfg: EngineConfig {
                // cleanHtml 相对 URL 重写基址（站点源站，core 统一维护）
                base_url: hnu_cg_helper_core::problem::CG_BASE_URL.into(),
                ..EngineConfig::default()
            },
            scenes_dir,
        }
    }

    /// 存储一个待登录的 session，返回 session_id
    pub async fn store_session(&self, session: CgSession) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let mut sessions = self.pending_sessions.write().await;
        sessions.insert(
            id.clone(),
            PendingSession {
                created_at: std::time::Instant::now(),
                session: Some(session),
            },
        );
        id
    }

    /// 取出并消费一个 session（用于登录）
    pub async fn take_session(&self, session_id: &str) -> Option<CgSession> {
        let mut sessions = self.pending_sessions.write().await;
        sessions.get_mut(session_id).and_then(|s| s.session.take())
    }
}
