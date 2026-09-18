//! AI 修复循环：解析脚本失败时，驱动 LLM 改脚本直到验证通过。
//!
//! 流程：构造提示词（脚本 + 脱敏现场输入 + 错误）→ LLM 全量重写脚本 →
//! 沙箱验证（fixtures + 失败现场复现）→ 绿则落盘热重载，红则带差异重试。
//!
//! 隐私红线：只有脱敏后的脚本输入会进入 LLM 上下文，原始页面绝不外发。

use hnu_cg_helper_adapter::{AdapterRegistry, EngineConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::ai::{self, ChatMessage};
use crate::error::CoreError;

/// 最大修复尝试轮数（预算兜底，防止烧 API 额度）
const MAX_ATTEMPTS: u32 = 3;

/// 现场输入注入提示词的最大长度（防超长上下文）
const SCENE_INPUT_LIMIT: usize = 12_000;

/// 修复进度事件（SSE 推送给前端）
#[derive(Debug, Clone, Serialize)]
pub struct RepairEvent {
    /// 阶段：attempt / validating / done / failed
    pub stage: String,
    /// 当前尝试轮次（1-based）
    pub attempt: u32,
    /// 人类可读消息
    pub message: String,
}

/// 失败现场数据（路由层在解析失败时留存）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    /// 页面类型（固定层成功时）
    pub page_type: Option<String>,
    /// 脱敏后的脚本输入
    pub script_input: Option<String>,
    /// 错误信息
    pub error: String,
    /// 留存时间（RFC 3339）
    pub captured_at: String,
}

/// AI 修复所需的配置
pub struct RepairConfig {
    /// LLM API Key
    pub api_key: String,
    /// LLM base_url
    pub base_url: String,
    /// 模型名
    pub model: String,
}

/// 提取 LLM 回复中的脚本：优先 ``` 围栏代码块，否则整体视为脚本
fn extract_script(reply: &str) -> Option<String> {
    for block in reply.split("```").skip(1).step_by(2) {
        let code = match block.find('\n') {
            Some(pos) => &block[pos + 1..],
            None => continue,
        };
        if code.contains("function parse") || code.contains("function buildPlan") {
            return Some(code.trim().to_string());
        }
    }
    let trimmed = reply.trim();
    if trimmed.contains("function parse") || trimmed.contains("function buildPlan") {
        return Some(trimmed.to_string());
    }
    None
}

fn system_prompt() -> &'static str {
    "你是站点适配框架的脚本修复专家。框架用 QuickJS 沙箱脚本解析网页：\n\
     - 宿主注入的 API：select(css) 返回元素句柄(未命中-1)、selectAll(css) 返回句柄数组、\
     text(el)、html(el)、attr(el, name)、cleanHtml(html) 白名单清洗、log(msg)\n\
     - 脚本必须导出 parse()，返回 { statement_html, statement_text, submission }\n\
     - submission 是带 kind 标签的对象：file_upload 需 languages/needs_main_class/problem_id/assign_id；\
     fill_gap 需 languages/gaps/skeleton/hidden_fields/problem_id/assign_id\n\
     - 输入 HTML 已经过区域提取和脱敏，只含题目相关片段\n\
     你的任务：根据错误信息和输入样本，输出修复后的完整脚本。\
     只输出一个 ```js 代码块，不要解释。"
}

/// 在字符边界截断字符串
fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn build_user_message(script: &str, scene: &SceneData, last_feedback: Option<&str>) -> String {
    let mut msg = format!(
        "当前脚本（有故障）：\n```js\n{script}\n```\n\n故障信息：{}\n",
        scene.error
    );
    if let Some(input) = &scene.script_input {
        let truncated = truncate_str(input, SCENE_INPUT_LIMIT);
        msg.push_str(&format!(
            "\n脚本输入（脱敏后）：\n```html\n{truncated}\n```\n"
        ));
    }
    if let Some(fb) = last_feedback {
        msg.push_str(&format!("\n上一次修复的验证结果（未通过）：\n{fb}\n"));
    }
    msg
}

fn reports_feedback(reports: &[hnu_cg_helper_adapter::ValidationReport]) -> String {
    let mut s = String::new();
    for r in reports {
        for f in &r.failures {
            s.push_str(&format!("[{}] {}: {}\n", r.page_type, f.fixture, f.detail));
        }
    }
    if s.is_empty() {
        "未知验证失败".to_string()
    } else {
        s
    }
}

/// 执行修复循环。通过 `tx` 推送进度；返回是否修复成功。
///
/// 成功时新脚本已落盘并热重载；失败时适配器维持修复前状态。
pub async fn repair_script(
    registry: Arc<AdapterRegistry>,
    engine_cfg: EngineConfig,
    page_type: String,
    scene: SceneData,
    ai_cfg: RepairConfig,
    tx: mpsc::Sender<RepairEvent>,
) -> Result<bool, CoreError> {
    // 固定层失败没有可送 LLM 的脱敏现场，属于开发者边界
    let Some(scene_input) = scene.script_input.clone() else {
        let _ = tx
            .send(RepairEvent {
                stage: "failed".into(),
                attempt: 0,
                message: "固定层（页面结构识别）失败，超出脚本自动修复能力，请反馈开发者".into(),
            })
            .await;
        return Ok(false);
    };

    let current_script = registry
        .current()
        .scripts
        .get(&page_type)
        .map(|s| s.parse.clone())
        .ok_or_else(|| CoreError::Config(format!("页面类型 {page_type} 没有解析脚本")))?;

    let mut messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system_prompt().to_string(),
        },
        ChatMessage {
            role: "user".into(),
            content: build_user_message(&current_script, &scene, None),
        },
    ];

    for attempt in 1..=MAX_ATTEMPTS {
        let _ = tx
            .send(RepairEvent {
                stage: "attempt".into(),
                attempt,
                message: format!("第 {attempt}/{MAX_ATTEMPTS} 轮：请求 AI 重写脚本…"),
            })
            .await;

        let reply =
            ai::chat_once(&ai_cfg.api_key, &ai_cfg.base_url, &ai_cfg.model, &messages).await?;
        let Some(new_script) = extract_script(&reply) else {
            messages.push(ChatMessage {
                role: "assistant".into(),
                content: reply,
            });
            messages.push(ChatMessage {
                role: "user".into(),
                content: "回复中未找到包含 parse 函数的脚本代码块，请重新输出完整脚本。".into(),
            });
            continue;
        };

        let _ = tx
            .send(RepairEvent {
                stage: "validating".into(),
                attempt,
                message: "收到候选脚本，正在沙箱中验证（fixtures + 失败现场）…".into(),
            })
            .await;

        let registry2 = registry.clone();
        let pt = page_type.clone();
        let scene_docs = vec![(page_type.clone(), scene_input.clone())];
        let script_for_check = new_script.clone();
        let cfg = engine_cfg.clone();
        let reports = tokio::task::spawn_blocking(move || {
            registry2.apply_candidate(&pt, Some(script_for_check), None, &scene_docs, &cfg)
        })
        .await
        .map_err(|e| CoreError::Ai(format!("验证任务失败: {e}")))??;

        if reports.iter().all(|r| r.is_green()) {
            let _ = tx
                .send(RepairEvent {
                    stage: "done".into(),
                    attempt,
                    message: "验证全部通过，新脚本已生效。刷新页面即可。".into(),
                })
                .await;
            return Ok(true);
        }

        let feedback = reports_feedback(&reports);
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: reply,
        });
        messages.push(ChatMessage {
            role: "user".into(),
            content: build_user_message(&new_script, &scene, Some(&feedback)),
        });
    }

    let _ = tx
        .send(RepairEvent {
            stage: "failed".into(),
            attempt: MAX_ATTEMPTS,
            message: format!("{MAX_ATTEMPTS} 轮修复均未通过验证，请反馈开发者（附带本日志）。"),
        })
        .await;
    Ok(false)
}
