//! 适配器注册表：持有当前生效的适配器快照，支持校验后热替换。
//!
//! 热重载语义：候选版本必须通过全部 fixture 验证才能换入；
//! 验证失败保留旧版——坏脚本永远不会暴露给用户。

use crate::adapter::LoadedAdapter;
use crate::engine::EngineConfig;
use crate::error::AdapterError;
use crate::validate::{self, ValidationReport};
use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// 适配器注册表
pub struct AdapterRegistry {
    current: ArcSwap<LoadedAdapter>,
}

impl AdapterRegistry {
    /// 加载适配包并构建注册表（启动路径；不做 fixture 验证，仅加载）
    pub fn load(dir: &Path) -> Result<Self, AdapterError> {
        let adapter = LoadedAdapter::load(dir)?;
        Ok(Self {
            current: ArcSwap::from_pointee(adapter),
        })
    }

    /// 当前生效的适配器快照
    pub fn current(&self) -> Arc<LoadedAdapter> {
        self.current.load_full()
    }

    /// 适配包目录
    pub fn dir(&self) -> PathBuf {
        self.current().dir.clone()
    }

    /// 从磁盘重新加载并校验：全绿才换入，失败保留旧版并返回报告
    pub fn reload_validated(
        &self,
        cfg: &EngineConfig,
    ) -> Result<Vec<ValidationReport>, AdapterError> {
        let candidate = LoadedAdapter::load(&self.dir())?;
        self.try_swap(candidate, cfg)
    }

    /// 用内存中的候选脚本换入（AI 修复路径）：先验证 → 落盘 → 换入注册表
    ///
    /// - `page_type_id`：被修复的页面类型
    /// - `parse` / `submit`：候选脚本源码（None 表示沿用磁盘版本）
    /// - `scene_docs`：失败现场留存的脱敏文档（page_type, doc），候选脚本必须能干净解析
    pub fn apply_candidate(
        &self,
        page_type_id: &str,
        parse: Option<String>,
        submit: Option<String>,
        scene_docs: &[(String, String)],
        cfg: &EngineConfig,
    ) -> Result<Vec<ValidationReport>, AdapterError> {
        let mut candidate = self.current().as_ref().clone();
        let scripts = candidate
            .scripts
            .get_mut(page_type_id)
            .ok_or_else(|| AdapterError::Manifest(format!("未知页面类型 {page_type_id}")))?;
        if let Some(p) = &parse {
            scripts.parse = p.clone();
        }
        if let Some(s) = &submit {
            scripts.submit = Some(s.clone());
        }

        // fixture 验证，绿了再跑失败现场复现
        let mut reports = validate::validate_loaded(&candidate, cfg);
        if reports.iter().all(ValidationReport::is_green) {
            for (pt_id, doc) in scene_docs {
                let Some(pt_cfg) = candidate
                    .manifest
                    .page_types
                    .iter()
                    .find(|p| &p.id == pt_id)
                else {
                    continue;
                };
                if let Err(e) = candidate.run_parse_on_doc(pt_cfg, doc, cfg) {
                    let failure = crate::validate::FixtureFailure {
                        fixture: "失败现场复现".to_string(),
                        detail: e.to_string(),
                    };
                    match reports.iter_mut().find(|r| &r.page_type == pt_id) {
                        Some(r) => r.failures.push(failure),
                        None => reports.push(ValidationReport {
                            page_type: pt_id.clone(),
                            total: 1,
                            passed: 0,
                            failures: vec![failure],
                        }),
                    }
                }
            }
        }
        if !reports.iter().all(ValidationReport::is_green) {
            tracing::warn!(page_type = %page_type_id, "候选脚本未通过验证，维持现状");
            return Ok(reports);
        }

        // 落盘
        let page_cfg = candidate
            .manifest
            .page_types
            .iter()
            .find(|p| p.id == page_type_id)
            .ok_or_else(|| AdapterError::Manifest(format!("未知页面类型 {page_type_id}")))?
            .clone();
        let dir = candidate.dir.clone();
        if let Some(p) = parse {
            std::fs::write(dir.join(&page_cfg.script), p)?;
        }
        if let (Some(s), Some(path)) = (submit, &page_cfg.submit_script) {
            std::fs::write(dir.join(path), s)?;
        }

        tracing::info!(page_type = %page_type_id, "修复脚本验证通过，已落盘并热重载");
        self.current.store(Arc::new(candidate));
        Ok(reports)
    }

    /// 尝试换入候选版本：验证全绿才换，返回验证报告
    fn try_swap(
        &self,
        candidate: LoadedAdapter,
        cfg: &EngineConfig,
    ) -> Result<Vec<ValidationReport>, AdapterError> {
        let reports = validate::validate_loaded(&candidate, cfg);
        if reports.iter().all(ValidationReport::is_green) {
            tracing::info!(dir = %candidate.dir.display(), "适配器验证通过，热重载生效");
            self.current.store(Arc::new(candidate));
        } else {
            let failed: Vec<&str> = reports
                .iter()
                .filter(|r| !r.is_green())
                .map(|r| r.page_type.as_str())
                .collect();
            tracing::warn!(page_types = ?failed, "候选适配器验证失败，保留旧版本");
        }
        Ok(reports)
    }
}

/// 启动文件监听：脚本目录变更后自动走校验热重载（开发模式）
///
/// 返回的 watcher 需保持存活；drop 即停止监听。
pub fn watch(
    registry: Arc<AdapterRegistry>,
    cfg: EngineConfig,
) -> Option<notify::RecommendedWatcher> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc;

    let dir = registry.dir();
    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("文件监听启动失败: {e}");
            return None;
        }
    };
    if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
        tracing::error!("监听适配包目录失败 {}: {e}", dir.display());
        return None;
    }

    std::thread::spawn(move || {
        // 简单防抖：收到事件后等 300ms 消化后续事件，再统一重载
        while rx.recv().is_ok() {
            std::thread::sleep(Duration::from_millis(300));
            while rx.try_recv().is_ok() {}
            if let Err(e) = registry.reload_validated(&cfg) {
                tracing::warn!("热重载失败: {e}");
            }
        }
    });

    Some(watcher)
}
