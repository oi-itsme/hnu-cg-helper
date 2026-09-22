/// Adapter crate 统一错误类型
#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    /// 配置文件解析错误
    #[error("适配器配置错误: {0}")]
    Manifest(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 页面类型检测失败（固定层）
    #[error("页面类型检测失败: 原始页面不匹配任何已知题型")]
    DetectFailed,

    /// 区域提取失败（固定层），列出未命中的选择器
    #[error("区域提取失败，未命中的选择器: {}", missing.join(", "))]
    RegionExtractFailed {
        /// 未命中的 CSS 选择器
        missing: Vec<String>,
    },

    /// 脚本执行失败（脚本层，可进入 AI 修复流程）
    #[error("脚本执行失败 [{page_type}]: {message}")]
    ScriptFailed {
        /// 页面类型 id
        page_type: String,
        /// 错误信息（含 JS 异常内容）
        message: String,
    },

    /// 脚本输出不符合 schema（脚本层，可进入 AI 修复流程）
    #[error("脚本输出不符合 schema [{page_type}]: {message}")]
    OutputInvalid {
        /// 页面类型 id
        page_type: String,
        /// 校验错误信息
        message: String,
    },
}

impl AdapterError {
    /// 是否属于脚本层失败（可尝试 AI 自动修复）。
    ///
    /// 固定层失败（检测/区域提取）超出脚本能力边界，不可自动修复。
    pub fn is_script_layer(&self) -> bool {
        matches!(
            self,
            AdapterError::ScriptFailed { .. } | AdapterError::OutputInvalid { .. }
        )
    }
}
