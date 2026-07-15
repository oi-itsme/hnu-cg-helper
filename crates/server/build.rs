fn main() {
    // embed-frontend feature check via env var (set by Cargo)
    let embed = std::env::var("CARGO_FEATURE_EMBED_FRONTEND").is_ok();

    if !embed {
        println!("cargo:warning=未启用 embed-frontend feature，跳过前端嵌入");
        return;
    }

    let dist_path = format!("{}/../../frontend/dist", env!("CARGO_MANIFEST_DIR"));

    let path = std::path::Path::new(&dist_path);

    if !path.exists() || !path.join("index.html").exists() {
        panic!(
            "\n\n\
             === frontend/dist/ 不存在或不完整 ===\n\
             \n\
             请先构建前端:  cd frontend && pnpm build\n\
             期望路径:        {}\n\
             \n\
             开发模式（不嵌入前端，由 Vite 代理）:\n\
               cargo run -p hnu-cg-helper-server --no-default-features\n\
            ",
            dist_path
        );
    }

    println!("cargo:rerun-if-changed={}", dist_path);
    memory_serve::load_directory(path);
}
