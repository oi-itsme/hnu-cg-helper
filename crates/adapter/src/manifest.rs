use serde::Deserialize;

/// adapter.toml 顶层结构
#[derive(Debug, Clone, Deserialize)]
pub struct AdapterManifest {
    /// 站点信息
    pub adapter: AdapterInfo,
    /// 页面类型列表，检测时按声明顺序匹配，先中先用
    #[serde(rename = "page_types", default)]
    pub page_types: Vec<PageTypeConfig>,
}

/// 站点元信息
#[derive(Debug, Clone, Deserialize)]
pub struct AdapterInfo {
    /// 站点 id（目录名）
    pub id: String,
    /// 展示名
    pub name: String,
}

/// 一种页面类型的配置
#[derive(Debug, Clone, Deserialize)]
pub struct PageTypeConfig {
    /// 页面类型 id（如 `problem-program`），同时是 fixtures 下的子目录名
    pub id: String,
    /// 页面解析脚本（相对适配包目录），须导出 `parse()`
    pub script: String,
    /// 提交脚本（相对适配包目录），须导出 `buildPlan(input)`
    pub submit_script: Option<String>,
    /// 区域提取选择器：只有命中的子树会进入脚本层，其余整页丢弃
    pub regions: Vec<String>,
    /// 页面类型嗅探标记：原始 HTML 包含全部子串时判定为该类型
    #[serde(default)]
    pub detect_contains: Vec<String>,
}
