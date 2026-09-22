//! 固定层脱敏：区域提取 + 模式擦除兜底。
//!
//! 隐私红线：只有经过本模块处理后的 HTML 才能进入脚本层。
//! 区域提取失败（选择器未命中）属于固定层故障，响亮报错、什么都不放行。

use crate::error::AdapterError;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

/// CG AI 助手的外链 URL（内嵌会话 token，必须整串擦除）
static EDUCG_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[A-Za-z0-9.-]*educg\.net[^\s"'<>)]*"#).expect("educg URL 正则")
});

/// URL 参数中的长 token（如 `cgtoken=...`），作为 educg URL 之外的兜底
static TOKEN_PARAM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:cgtoken|token|access_token|ticket)=[A-Za-z0-9_\-]{32,}")
        .expect("token 参数正则")
});

/// 区域提取：按配置的选择器收集子树，拼装成合成文档；任一选择器未命中则整体失败
pub fn extract_regions(raw_html: &str, regions: &[String]) -> Result<String, AdapterError> {
    let doc = Html::parse_document(raw_html);
    let mut out = String::with_capacity(raw_html.len() / 4);
    out.push_str("<!DOCTYPE html><html><body>");
    let mut missing = Vec::new();

    for sel_str in regions {
        let Ok(selector) = Selector::parse(sel_str) else {
            missing.push(sel_str.clone());
            continue;
        };
        let mut hit = false;
        for el in doc.select(&selector) {
            out.push_str(&el.html());
            hit = true;
        }
        if !hit {
            missing.push(sel_str.clone());
        }
    }

    out.push_str("</body></html>");

    if missing.is_empty() {
        Ok(out)
    } else {
        Err(AdapterError::RegionExtractFailed { missing })
    }
}

/// 模式擦除兜底：已知隐私值（学号等）+ educg 外链 + 长 token 参数
pub fn scrub(html: &str, known_values: &[String]) -> String {
    let mut s = html.to_owned();
    for v in known_values {
        if v.len() >= 2 {
            s = s.replace(v.as_str(), "[USER]");
        }
    }
    let s = EDUCG_URL.replace_all(&s, "[CG_AI_URL]");
    TOKEN_PARAM.replace_all(&s, "[CG_TOKEN]").into_owned()
}

/// 题面 HTML 白名单清洗器（cleanHtml 宿主原语的底层）
///
/// 只保留语义标签；表现层属性（style/class/id）一律剥除，
/// 视觉风格完全由 GUI 侧排版样式决定。
///
/// `base_url` 非空时，相对 URL（img src / a href）以它为基址改写为绝对地址——
/// 题面最终在 helper 前端渲染，相对地址必须指向站点源站才不会 404。
pub fn build_cleaner(base_url: &str) -> ammonia::Builder<'static> {
    const TAGS: &[&str] = &[
        "p",
        "pre",
        "code",
        "table",
        "thead",
        "tbody",
        "tfoot",
        "tr",
        "td",
        "th",
        "ul",
        "ol",
        "li",
        "strong",
        "b",
        "em",
        "i",
        "u",
        "s",
        "sub",
        "sup",
        "br",
        "hr",
        "img",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "span",
        "div",
        "a",
    ];
    let mut tag_attrs: std::collections::HashMap<
        &'static str,
        std::collections::HashSet<&'static str>,
    > = std::collections::HashMap::new();
    tag_attrs.insert("img", ["src", "alt", "title"].into_iter().collect());
    tag_attrs.insert("a", ["href", "title"].into_iter().collect());
    tag_attrs.insert("td", ["colspan", "rowspan"].into_iter().collect());
    tag_attrs.insert("th", ["colspan", "rowspan"].into_iter().collect());

    let mut builder = ammonia::Builder::new();
    builder
        .tags(TAGS.iter().copied().collect())
        .tag_attributes(tag_attrs)
        // script/style 连内容一起移除，不允许脚本层看到内联脚本
        .clean_content_tags(["script", "style"].into_iter().collect())
        .link_rel(None);
    if !base_url.is_empty()
        && let Ok(base) = url::Url::parse(base_url)
    {
        builder.url_relative(ammonia::UrlRelative::RewriteWithBase(base));
    }
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_regions_collects_matches() {
        let html = r#"<html><body><nav>导航 用户名</nav><div class="content">题面</div><form name="upload"></form></body></html>"#;
        let out = extract_regions(html, &[".content".into(), "form[name=upload]".into()])
            .expect("应提取成功");
        assert!(out.contains("题面"));
        assert!(!out.contains("用户名"), "外层导航不应进入提取结果");
    }

    #[test]
    fn extract_regions_missing_is_error() {
        let html = "<html><body>nothing</body></html>";
        let err = extract_regions(html, &[".content".into()]).unwrap_err();
        assert!(matches!(err, AdapterError::RegionExtractFailed { .. }));
    }

    #[test]
    fn scrub_replaces_known_values_and_tokens() {
        let html = r#"用户 20230001 提交了 <a href="https://gxllmapi.educg.net?cgtoken=abcXYZ_0123456789abcdefghijklmnopqrstuvwxyz">AI</a>"#;
        let out = scrub(html, &["20230001".to_string()]);
        assert!(!out.contains("20230001"));
        assert!(out.contains("[USER]"));
        assert!(!out.contains("cgtoken="));
        assert!(out.contains("[CG_AI_URL]"));
    }

    #[test]
    fn cleaner_strips_presentation() {
        let cleaner = build_cleaner("");
        let html = r#"<p style="text-indent:35px"><span style="font-size:14px">文本</span></p><pre class="brush:cpp">code</pre><script>evil()</script>"#;
        let out = cleaner.clean(html).to_string();
        assert!(out.contains("文本"));
        assert!(out.contains("<pre>code</pre>"));
        assert!(!out.contains("style="));
        assert!(!out.contains("class="));
        assert!(!out.contains("evil"), "script 内容应被连根移除");
    }

    #[test]
    fn cleaner_rewrites_relative_urls_with_base() {
        let cleaner = build_cleaner("https://cg.hnu.edu.cn");
        let html = r#"<p><img src="/ShowImage?id=1"><a href="assignment/programList.jsp?proNum=2">下一题</a><a href="https://other.example/x">外链</a></p>"#;
        let out = cleaner.clean(html).to_string();
        assert!(
            out.contains(r#"src="https://cg.hnu.edu.cn/ShowImage?id=1""#),
            "img 相对地址应绝对化: {out}"
        );
        assert!(
            out.contains(r#"href="https://cg.hnu.edu.cn/assignment/programList.jsp?proNum=2""#),
            "a 相对地址应绝对化: {out}"
        );
        assert!(
            out.contains(r#"href="https://other.example/x""#),
            "已是绝对地址的外链保持不变: {out}"
        );
    }
}
