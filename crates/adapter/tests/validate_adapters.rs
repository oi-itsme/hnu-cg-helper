//! 全量适配包验证测试：遍历仓库 adapters/ 下所有适配包，跑 fixture 精确比对。
//!
//! CI 直接可用；AI 修复后的脚本也必须通过这里验证的同一套逻辑。

use hnu_cg_helper_adapter::engine::EngineConfig;
use hnu_cg_helper_adapter::{LoadedAdapter, PlanBody, validate};
use std::path::PathBuf;

fn adapters_root() -> PathBuf {
    // 以 crate 目录为基准定位仓库根的 adapters/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../adapters")
}

#[test]
fn validate_all_adapters() {
    let root = adapters_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("adapters 目录不存在，跳过: {}", root.display());
        return;
    };

    let cfg = EngineConfig::default();
    let mut any = false;
    let mut all_green = true;

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join("adapter.toml").exists() {
            continue;
        }
        any = true;
        let reports = validate::validate_dir(&dir, &cfg).expect("适配包加载失败");
        for r in &reports {
            println!(
                "[{}] {}: {}/{} 通过",
                dir.file_name().unwrap().to_string_lossy(),
                r.page_type,
                r.passed,
                r.total
            );
            for f in &r.failures {
                println!("  ✗ {}: {}", f.fixture, f.detail);
            }
            if !r.is_green() {
                all_green = false;
            }
            assert!(r.total > 0, "页面类型 {} 没有 fixture", r.page_type);
        }
    }

    assert!(any, "没有找到任何适配包");
    assert!(all_green, "存在未通过 fixture 验证的适配包");
}

/// 提交脚本冒烟测试：用 fixture 期望输出中的 descriptor 驱动 buildPlan，
/// 验证提交计划的结构完整性（URL、关键字段）。
#[test]
fn submit_scripts_build_valid_plans() {
    let root = adapters_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let cfg = EngineConfig::default();

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join("adapter.toml").exists() {
            continue;
        }
        let adapter = LoadedAdapter::load(&dir).expect("加载适配包失败");

        for pt in &adapter.manifest.page_types {
            let Some(descriptor) = load_first_descriptor(&dir, &pt.id) else {
                continue;
            };
            let input = serde_json::json!({
                "descriptor": descriptor,
                "language": "c++",
                "main_class": null,
                "code": "int main() { return 0; }",
                "answers": { "answer1": "// code" },
                "wtime": 42
            });
            let plan = adapter
                .build_plan(&pt.id, &input, &cfg)
                .unwrap_or_else(|e| panic!("{} buildPlan 失败: {e}", pt.id));

            assert_eq!(plan.method, "POST", "{} 方法应为 POST", pt.id);
            assert!(!plan.url.is_empty(), "{} URL 不应为空", pt.id);
            match &plan.body {
                PlanBody::Multipart { file_field, .. } => {
                    assert!(!file_field.is_empty());
                    assert!(
                        plan.url.contains("problemID="),
                        "{} 上传 URL 应含 problemID",
                        pt.id
                    );
                    assert!(plan.url.contains("wtime=42"), "{} URL 应含 wtime", pt.id);
                }
                PlanBody::Form { fields } => {
                    assert!(
                        fields.contains_key("problemID"),
                        "{} 表单应含 problemID",
                        pt.id
                    );
                    assert_eq!(
                        fields.get("wtime").map(String::as_str),
                        Some("42"),
                        "{} 表单 wtime 应为真实耗时",
                        pt.id
                    );
                }
            }
        }
    }
}

/// 读取某页面类型第一份 fixture 的期望输出，取其中的 submission descriptor
fn load_first_descriptor(dir: &std::path::Path, page_type: &str) -> Option<serde_json::Value> {
    let fixture_dir = dir.join("fixtures").join(page_type);
    let mut entries: Vec<_> = std::fs::read_dir(fixture_dir).ok()?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let text = std::fs::read_to_string(path).ok()?;
            let v: serde_json::Value = serde_json::from_str(&text).ok()?;
            return v.get("submission").cloned();
        }
    }
    None
}
