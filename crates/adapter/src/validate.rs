//! fixture 验证：脚本 × 黄金样本的精确比对（statement 字段空白归一化后）。
//!
//! 这是 AI 自动修复闭环的地基——修复后的脚本必须通过全部 fixture 才能热重载。

use crate::adapter::LoadedAdapter;
use crate::engine::EngineConfig;
use crate::error::AdapterError;
use crate::schema::ProblemPageOutput;
use serde::Serialize;
use std::path::Path;

/// 单个 fixture 的失败详情
#[derive(Debug, Clone, Serialize)]
pub struct FixtureFailure {
    /// fixture 文件名
    pub fixture: String,
    /// 失败描述（脚本错误或字段差异）
    pub detail: String,
}

/// 一个页面类型的验证报告
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// 页面类型 id
    pub page_type: String,
    /// fixture 总数
    pub total: usize,
    /// 通过数
    pub passed: usize,
    /// 失败列表
    pub failures: Vec<FixtureFailure>,
}

impl ValidationReport {
    /// 全部通过（且确实验证过 fixture——空报告不算绿）
    pub fn is_green(&self) -> bool {
        self.total > 0 && self.failures.is_empty()
    }
}

/// 空白归一化：压缩连续空白为单个空格
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 向下对齐到字符边界（防止多字节文本切片 panic）
fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// 向上对齐到字符边界
fn ceil_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// 找出首个差异位置，返回带上下文的摘要
fn diff_summary(expected: &str, actual: &str) -> String {
    let e = expected.as_bytes();
    let a = actual.as_bytes();
    let pos = e
        .iter()
        .zip(a.iter())
        .position(|(x, y)| x != y)
        .unwrap_or(e.len().min(a.len()));
    let e_start = floor_boundary(expected, pos.saturating_sub(30));
    let a_start = floor_boundary(actual, pos.saturating_sub(30));
    let e_end = ceil_boundary(expected, (pos + 30).min(expected.len()));
    let a_end = ceil_boundary(actual, (pos + 30).min(actual.len()));
    format!(
        "首个差异在字节 {pos}；期望 `…{}…`，实际 `…{}…`",
        &expected[e_start..e_end],
        &actual[a_start..a_end]
    )
}

/// 比对实际输出与期望输出，返回差异描述（无差异返回 None）
fn compare_output(expected: &ProblemPageOutput, actual: &ProblemPageOutput) -> Option<String> {
    let e_html = normalize_ws(&expected.statement_html);
    let a_html = normalize_ws(&actual.statement_html);
    if e_html != a_html {
        return Some(format!(
            "statement_html 不匹配: {}",
            diff_summary(&e_html, &a_html)
        ));
    }
    let e_text = normalize_ws(&expected.statement_text);
    let a_text = normalize_ws(&actual.statement_text);
    if e_text != a_text {
        return Some(format!(
            "statement_text 不匹配: {}",
            diff_summary(&e_text, &a_text)
        ));
    }
    if expected.submission != actual.submission {
        let e = serde_json::to_string_pretty(&expected.submission).unwrap_or_default();
        let a = serde_json::to_string_pretty(&actual.submission).unwrap_or_default();
        return Some(format!("submission 不匹配: {}", diff_summary(&e, &a)));
    }
    None
}

/// 验证已加载适配器的全部 fixture
pub fn validate_loaded(adapter: &LoadedAdapter, cfg: &EngineConfig) -> Vec<ValidationReport> {
    let mut reports = Vec::new();

    for page_type in &adapter.manifest.page_types {
        let fixture_dir = adapter.dir.join("fixtures").join(&page_type.id);
        let mut report = ValidationReport {
            page_type: page_type.id.clone(),
            total: 0,
            passed: 0,
            failures: Vec::new(),
        };

        let entries = match std::fs::read_dir(&fixture_dir) {
            Ok(e) => e,
            Err(_) => {
                // fixture 缺失是硬失败：空报告会被 is_green 误判全绿，
                // 导致未验证的脚本被热换入（违反「坏脚本不暴露给用户」不变量）
                tracing::warn!(page_type = %page_type.id, "fixture 目录不存在，验证失败");
                report.failures.push(FixtureFailure {
                    fixture: "-".into(),
                    detail: format!("fixture 目录不存在: {}", fixture_dir.display()),
                });
                reports.push(report);
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }
            report.total += 1;
            let name = entry.file_name().to_string_lossy().into_owned();
            let expected_path = path.with_extension("expected.json");

            let result = validate_one(adapter, page_type, &path, &expected_path, cfg);
            match result {
                Ok(None) => report.passed += 1,
                Ok(Some(diff)) => report.failures.push(FixtureFailure {
                    fixture: name,
                    detail: diff,
                }),
                Err(e) => report.failures.push(FixtureFailure {
                    fixture: name,
                    detail: e.to_string(),
                }),
            }
        }

        if report.total == 0 {
            report.failures.push(FixtureFailure {
                fixture: "-".into(),
                detail: format!("fixture 目录为空: {}", fixture_dir.display()),
            });
        }

        reports.push(report);
    }

    reports
}

fn validate_one(
    adapter: &LoadedAdapter,
    page_type: &crate::manifest::PageTypeConfig,
    fixture: &Path,
    expected_path: &Path,
    cfg: &EngineConfig,
) -> Result<Option<String>, AdapterError> {
    let html = std::fs::read_to_string(fixture)?;
    let expected_text = std::fs::read_to_string(expected_path).map_err(|e| {
        AdapterError::Manifest(format!("缺少期望输出 {}: {e}", expected_path.display()))
    })?;
    let expected: ProblemPageOutput = serde_json::from_str(&expected_text)
        .map_err(|e| AdapterError::Manifest(format!("期望输出不是合法 JSON: {e}")))?;

    let actual = adapter.run_parse_on_doc(page_type, &html, cfg)?;
    Ok(compare_output(&expected, &actual))
}

/// 验证适配包目录（加载后验证）
pub fn validate_dir(dir: &Path, cfg: &EngineConfig) -> Result<Vec<ValidationReport>, AdapterError> {
    let adapter = LoadedAdapter::load(dir)?;
    Ok(validate_loaded(&adapter, cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_summary_handles_multibyte_text() {
        // 首个差异落在中文字符中间时不得 panic
        let expected = "题面内容：给定一个整数 n，求它的阶乘。输出一行。";
        let actual = "题面内容：给定一个整数 n，求它的阶乘！输出一行。";
        let msg = diff_summary(expected, actual);
        assert!(msg.contains("首个差异在字节"));
        // 差异在首尾 30 字节窗口之外的长中文文本
        let long_e = "这是一段非常非常长的中文题面，用来把差异位置推到三十字节窗口之外，期望文本。";
        let long_a = "这是一段非常非常长的中文题面，用来把差异位置推到三十字节窗口之外，实际文本！";
        let _ = diff_summary(long_e, long_a);
        // 前缀完全相同、长度不同
        let _ = diff_summary("中文前缀", "中文前缀多了字");
    }

    #[test]
    fn empty_report_is_not_green() {
        let report = ValidationReport {
            page_type: "p".into(),
            total: 0,
            passed: 0,
            failures: vec![],
        };
        assert!(!report.is_green(), "没有 fixture 的空报告不得视为全绿");
    }
}
