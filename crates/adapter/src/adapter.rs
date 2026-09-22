//! 适配器加载与解析管线：检测题型 → 区域提取 → 模式擦除 → 脚本解析。

use crate::engine::{self, EngineConfig};
use crate::error::AdapterError;
use crate::manifest::{AdapterManifest, PageTypeConfig};
use crate::sanitize;
use crate::schema::{ProblemPageOutput, SubmissionPlan};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 一个页面类型的脚本集（解析脚本 + 可选提交脚本）
#[derive(Debug, Clone)]
pub struct PageScripts {
    /// 解析脚本源码（导出 `parse()`）
    pub parse: String,
    /// 提交脚本源码（导出 `buildPlan(input)`）
    pub submit: Option<String>,
}

/// 已加载的适配器：配置 + 全部脚本源码（内存中的不可变快照）
#[derive(Debug, Clone)]
pub struct LoadedAdapter {
    /// 适配包配置
    pub manifest: AdapterManifest,
    /// 页面类型 id → 脚本集
    pub scripts: HashMap<String, PageScripts>,
    /// 适配包目录（fixtures 等资源的根）
    pub dir: PathBuf,
}

impl LoadedAdapter {
    /// 从适配包目录加载：读 adapter.toml + 全部脚本文件
    pub fn load(dir: &Path) -> Result<Self, AdapterError> {
        let manifest_text = std::fs::read_to_string(dir.join("adapter.toml"))?;
        let manifest: AdapterManifest = toml::from_str(&manifest_text)
            .map_err(|e| AdapterError::Manifest(format!("adapter.toml 解析失败: {e}")))?;

        let mut scripts = HashMap::new();
        for pt in &manifest.page_types {
            let parse = std::fs::read_to_string(dir.join(&pt.script))
                .map_err(|e| AdapterError::Manifest(format!("读取脚本 {} 失败: {e}", pt.script)))?;
            let submit = match &pt.submit_script {
                Some(p) => Some(
                    std::fs::read_to_string(dir.join(p))
                        .map_err(|e| AdapterError::Manifest(format!("读取脚本 {p} 失败: {e}")))?,
                ),
                None => None,
            };
            scripts.insert(pt.id.clone(), PageScripts { parse, submit });
        }

        Ok(Self {
            manifest,
            scripts,
            dir: dir.to_path_buf(),
        })
    }

    /// 页面类型嗅探：按声明顺序，原始 HTML 包含全部标记子串者命中
    pub fn detect_page_type(&self, raw_html: &str) -> Option<&PageTypeConfig> {
        self.manifest.page_types.iter().find(|pt| {
            !pt.detect_contains.is_empty()
                && pt.detect_contains.iter().all(|m| raw_html.contains(m))
        })
    }

    /// 固定层预处理：检测 → 区域提取 → 模式擦除。
    /// 返回 (页面类型, 脱敏后的合成文档)。此处失败属于固定层故障。
    pub fn prepare_script_input(
        &self,
        raw_html: &str,
        known_values: &[String],
    ) -> Result<(&PageTypeConfig, String), AdapterError> {
        let page_type = self
            .detect_page_type(raw_html)
            .ok_or(AdapterError::DetectFailed)?;
        let extracted = sanitize::extract_regions(raw_html, &page_type.regions)?;
        let scrubbed = sanitize::scrub(&extracted, known_values);
        Ok((page_type, scrubbed))
    }

    /// 完整解析管线：原始页面 → ProblemPageOutput
    pub fn process_page(
        &self,
        raw_html: &str,
        known_values: &[String],
        cfg: &EngineConfig,
    ) -> Result<ProblemPageOutput, AdapterError> {
        let (page_type, doc) = self.prepare_script_input(raw_html, known_values)?;
        self.run_parse_on_doc(page_type, &doc, cfg)
    }

    /// 在已脱敏的文档上运行解析脚本（fixture 验证与生产共用）
    pub fn run_parse_on_doc(
        &self,
        page_type: &PageTypeConfig,
        doc_html: &str,
        cfg: &EngineConfig,
    ) -> Result<ProblemPageOutput, AdapterError> {
        let scripts = self
            .scripts
            .get(&page_type.id)
            .ok_or_else(|| AdapterError::Manifest(format!("页面类型 {} 缺少脚本", page_type.id)))?;

        let value = engine::run_parse(doc_html, &scripts.parse, cfg).map_err(|e| {
            AdapterError::ScriptFailed {
                page_type: page_type.id.clone(),
                message: e.to_string(),
            }
        })?;

        serde_json::from_value(value).map_err(|e| AdapterError::OutputInvalid {
            page_type: page_type.id.clone(),
            message: e.to_string(),
        })
    }

    /// 运行提交脚本，产出提交计划
    pub fn build_plan(
        &self,
        page_type_id: &str,
        input: &serde_json::Value,
        cfg: &EngineConfig,
    ) -> Result<SubmissionPlan, AdapterError> {
        let scripts = self
            .scripts
            .get(page_type_id)
            .ok_or_else(|| AdapterError::Manifest(format!("未知页面类型 {page_type_id}")))?;
        let submit = scripts.submit.as_ref().ok_or_else(|| {
            AdapterError::Manifest(format!("页面类型 {page_type_id} 没有提交脚本"))
        })?;

        let value =
            engine::run_build_plan(submit, input, cfg).map_err(|e| AdapterError::ScriptFailed {
                page_type: page_type_id.to_string(),
                message: e.to_string(),
            })?;

        serde_json::from_value(value).map_err(|e| AdapterError::OutputInvalid {
            page_type: page_type_id.to_string(),
            message: e.to_string(),
        })
    }
}
