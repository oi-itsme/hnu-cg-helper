//! 开发工具：从 fixtures/raw/ 的原始抓取生成脱敏 fixture 与期望输出。
//!
//! 用法：`cargo run -p hnu-cg-helper-adapter --example gen_fixtures`
//!
//! 原始抓取（含隐私）只存在于 fixtures/raw/（已 gitignore）；
//! 生成的 fixture 是"脚本层所见"的脱敏合成文档，可安全入库。

use hnu_cg_helper_adapter::{EngineConfig, LoadedAdapter};
use std::path::PathBuf;

fn main() {
    // 以 crate 目录为基准定位仓库根
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw_dir = crate_dir.join("../../fixtures/raw");
    let adapter_dir = crate_dir.join("../../adapters/hnu-cg");
    let cfg = EngineConfig::default();

    let adapter = LoadedAdapter::load(&adapter_dir).expect("加载适配包失败");

    let entries = std::fs::read_dir(&raw_dir).expect("读取 fixtures/raw 失败");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("html") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let raw = std::fs::read_to_string(&path).expect("读取原始页面失败");

        // 固定层预处理：检测 → 区域提取 → 模式擦除
        let (page_type, doc) = match adapter.prepare_script_input(&raw, &[]) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("✗ {stem}: 固定层预处理失败: {e}");
                continue;
            }
        };

        let out_dir = adapter_dir.join("fixtures").join(&page_type.id);
        std::fs::create_dir_all(&out_dir).expect("创建 fixture 目录失败");
        let fixture_path = out_dir.join(format!("{stem}.html"));
        std::fs::write(&fixture_path, &doc).expect("写入 fixture 失败");

        // 跑解析脚本，冻结期望输出
        match adapter.run_parse_on_doc(page_type, &doc, &cfg) {
            Ok(output) => {
                let expected = serde_json::to_string_pretty(&output).unwrap();
                std::fs::write(fixture_path.with_extension("expected.json"), expected)
                    .expect("写入期望输出失败");
                println!("✓ {stem} → [{}] fixture + expected.json", page_type.id);
            }
            Err(e) => eprintln!("✗ {stem}: 脚本解析失败: {e}"),
        }
    }
}
