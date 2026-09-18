//! 题目页管线：抓取原始页面 → 适配框架 → 结构化输出 / 提交执行。

use hnu_cg_helper_adapter::schema::SubmitInput;
use hnu_cg_helper_adapter::{AdapterError, AdapterRegistry, EngineConfig, ProblemPageOutput};
use hnu_query::cg::login::CgToken;
use serde::Serialize;

use crate::error::CoreError;

/// CG 站点根（与 hnu_query 内部一致）
pub const CG_BASE_URL: &str = "https://cg.hnu.edu.cn";

/// 解析成功
#[derive(Debug, Serialize)]
pub struct ProblemSuccess {
    /// 页面类型 id
    pub page_type: String,
    /// 结构化输出
    #[serde(flatten)]
    pub output: ProblemPageOutput,
}

/// 解析失败：携带领修复流程所需的现场信息
#[derive(Debug)]
pub struct ProblemFailure {
    /// 原始错误
    pub error: AdapterError,
    /// 页面类型（检测成功时）
    pub page_type: Option<String>,
    /// 脱敏后的脚本输入（固定层成功时），供 fixture 化与 AI 修复
    pub script_input: Option<String>,
}

impl ProblemFailure {
    /// 是否为脚本层失败（可 AI 自动修复）
    pub fn repairable(&self) -> bool {
        self.error.is_script_layer() && self.script_input.is_some()
    }
}

/// 题目页完整管线：检测 → 脱敏 → 脚本解析
///
/// 同步且含 QuickJS 沙箱执行，调用方应在阻塞上下文中运行（spawn_blocking）。
pub fn process_problem_page(
    registry: &AdapterRegistry,
    raw_html: &str,
    known_values: &[String],
    cfg: &EngineConfig,
) -> Result<ProblemSuccess, ProblemFailure> {
    let adapter = registry.current();
    let (page_type, doc) = match adapter.prepare_script_input(raw_html, known_values) {
        Ok(v) => v,
        Err(error) => {
            return Err(ProblemFailure {
                error,
                page_type: None,
                script_input: None,
            });
        }
    };
    let page_type_id = page_type.id.clone();
    match adapter.run_parse_on_doc(page_type, &doc, cfg) {
        Ok(output) => Ok(ProblemSuccess {
            page_type: page_type_id,
            output,
        }),
        Err(error) => Err(ProblemFailure {
            error,
            page_type: Some(page_type_id),
            script_input: Some(doc),
        }),
    }
}

/// 生成提交计划（沙箱内运行提交脚本）
pub fn build_submission_plan(
    registry: &AdapterRegistry,
    page_type: &str,
    input: &SubmitInput,
    cfg: &EngineConfig,
) -> Result<hnu_cg_helper_adapter::SubmissionPlan, AdapterError> {
    let adapter = registry.current();
    let value = serde_json::to_value(input).map_err(|e| AdapterError::Manifest(e.to_string()))?;
    adapter.build_plan(page_type, &value, cfg)
}

/// 按语言推断上传文件名
fn source_filename(language: &str, main_class: Option<&str>) -> String {
    match language {
        "c" => "main.c".to_string(),
        "c++" => "main.cpp".to_string(),
        "java" => main_class
            .and_then(|c| c.rsplit('.').next())
            .map(|c| format!("{c}.java"))
            .unwrap_or_else(|| "Main.java".to_string()),
        "python" => "main.py".to_string(),
        _ => "main.txt".to_string(),
    }
}

/// 解析提交计划 URL：相对路径以 CG 源站为基址展开；
/// 任何指向 CG 源站之外的 URL（绝对、协议相对、opaque scheme）一律拒绝——
/// 提交请求附带 CG 会话凭证，放行外域即凭证外泄。
fn resolve_plan_url(plan_url: &str) -> Result<String, CoreError> {
    let base = reqwest::Url::parse(CG_BASE_URL).expect("CG_BASE_URL 是合法 URL");
    let url = base
        .join(plan_url)
        .map_err(|e| CoreError::Submit(format!("提交计划 URL 非法: {e}")))?;
    if url.origin() != base.origin() {
        return Err(CoreError::Submit(format!(
            "提交计划 URL 指向站点源站之外，已拦截: {url}"
        )));
    }
    Ok(url.into())
}

/// 执行提交计划：带 CG 会话发送 HTTP 请求，返回结果页原始 HTML
pub async fn execute_submission(
    token: &CgToken,
    plan: &hnu_cg_helper_adapter::SubmissionPlan,
    code: Option<&str>,
    language: &str,
    main_class: Option<&str>,
) -> Result<String, CoreError> {
    use hnu_cg_helper_adapter::PlanBody;

    let url = resolve_plan_url(&plan.url)?;

    let mut req =
        reqwest::Client::new().request(plan.method.parse().unwrap_or(reqwest::Method::POST), &url);
    for (name, value) in token.headers() {
        req = req.header(name, value);
    }

    let resp = match &plan.body {
        PlanBody::Multipart { file_field, fields } => {
            let code = code.ok_or_else(|| CoreError::Submit("缺少提交代码".to_string()))?;
            let filename = source_filename(language, main_class);
            let part = reqwest::multipart::Part::text(code.to_string()).file_name(filename);
            let mut form = reqwest::multipart::Form::new().part(file_field.clone(), part);
            for (k, v) in fields {
                form = form.text(k.clone(), v.clone());
            }
            req.multipart(form).send().await?
        }
        PlanBody::Form { fields } => req.form(fields).send().await?,
    };

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(CoreError::Submit(format!("CG 提交响应异常 HTTP {status}")));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_plan_url_allows_relative_and_same_origin() {
        let u = resolve_plan_url("assignment/showProcessMsg.jsp?problemID=1").unwrap();
        assert_eq!(
            u,
            "https://cg.hnu.edu.cn/assignment/showProcessMsg.jsp?problemID=1"
        );

        let u = resolve_plan_url("/assignment/showProcessMsg.jsp").unwrap();
        assert_eq!(u, "https://cg.hnu.edu.cn/assignment/showProcessMsg.jsp");

        let u = resolve_plan_url("https://cg.hnu.edu.cn/assignment/showProcessMsg.jsp").unwrap();
        assert_eq!(u, "https://cg.hnu.edu.cn/assignment/showProcessMsg.jsp");
    }

    #[test]
    fn resolve_plan_url_rejects_foreign_origins() {
        assert!(resolve_plan_url("https://evil.example/collect").is_err());
        assert!(
            resolve_plan_url("http://cg.hnu.edu.cn/x").is_err(),
            "scheme 降级也拒绝"
        );
        assert!(
            resolve_plan_url("//evil.example/x").is_err(),
            "协议相对 URL 拒绝"
        );
        assert!(resolve_plan_url("https://cg.hnu.edu.cn.evil.example/x").is_err());
        assert!(
            resolve_plan_url("javascript:alert(1)").is_err(),
            "opaque scheme 拒绝"
        );
    }
}
