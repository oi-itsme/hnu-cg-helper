//! QuickJS 沙箱引擎：宿主 API 注入 + 资源限额 + 脚本入口调用。
//!
//! 安全模型：QuickJS 是裸 JS 引擎，脚本能用的能力只有这里注入的白名单函数。
//! 不注入网络/文件系统/定时器，模块系统不启用（脚本自包含，无 import）。
//!
//! 元素句柄约定：宿主在脚本运行前把（区域提取后的）文档解析成 DOM，
//! `select`/`selectAll` 返回非负整数句柄；`select` 未命中返回 -1。

use crate::sanitize::build_cleaner;
use ego_tree::NodeId;
use rquickjs::function::Func;
use rquickjs::{Context, Ctx, IntoJs, Runtime};
use scraper::{ElementRef, Html, Selector};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// 引擎错误：区分脚本自身失败与宿主引擎故障
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// 脚本抛异常或超时
    #[error("{0}")]
    Script(String),
    /// 宿主引擎故障（运行时创建失败、输出序列化失败等）
    #[error("引擎故障: {0}")]
    Host(String),
}

/// 引擎运行限额
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 堆内存上限（字节）
    pub memory_limit: usize,
    /// 栈上限（字节）
    pub max_stack_size: usize,
    /// 单次脚本执行墙上时钟上限
    pub timeout: Duration,
    /// 站点根 URL：cleanHtml 以此为基址把相对 img src / a href 改写为绝对地址；
    /// 空串表示不重写
    pub base_url: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            memory_limit: 32 * 1024 * 1024,
            max_stack_size: 1024 * 1024,
            timeout: Duration::from_secs(5),
            base_url: String::new(),
        }
    }
}

struct DomStore {
    doc: Html,
    handles: RefCell<Vec<NodeId>>,
}

thread_local! {
    /// 脚本执行期间的文档存储；仅在当前线程、当前次执行内有效
    static DOM: RefCell<Option<DomStore>> = const { RefCell::new(None) };
    /// cleanHtml 的 URL 重写基址；随本次执行的 EngineConfig 设置
    static CLEANER_BASE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// 以指定消息抛出 JS 异常
fn throw_msg(ctx: &Ctx<'_>, msg: String) -> rquickjs::Error {
    match msg.into_js(ctx) {
        Ok(v) => ctx.throw(v),
        Err(e) => e,
    }
}

fn with_dom<R>(ctx: &Ctx<'_>, f: impl FnOnce(&DomStore) -> R) -> rquickjs::Result<R> {
    DOM.with(|d| {
        let borrowed = d.borrow();
        match borrowed.as_ref() {
            Some(store) => Ok(f(store)),
            None => Err(throw_msg(ctx, "内部错误: DOM 未初始化".to_string())),
        }
    })
}

/// `select(css)` → 元素句柄（非负整数），未命中返回 -1；选择器非法抛异常
fn js_select(ctx: Ctx<'_>, css: String) -> rquickjs::Result<i64> {
    let selector = match Selector::parse(&css) {
        Ok(s) => s,
        Err(e) => return Err(throw_msg(&ctx, format!("非法 CSS 选择器 `{css}`: {e}"))),
    };
    with_dom(&ctx, |store| match store.doc.select(&selector).next() {
        Some(el) => {
            let mut handles = store.handles.borrow_mut();
            handles.push(el.id());
            (handles.len() - 1) as i64
        }
        None => -1,
    })
}

/// `selectAll(css)` → 元素句柄数组
fn js_select_all(ctx: Ctx<'_>, css: String) -> rquickjs::Result<Vec<i64>> {
    let selector = match Selector::parse(&css) {
        Ok(s) => s,
        Err(e) => return Err(throw_msg(&ctx, format!("非法 CSS 选择器 `{css}`: {e}"))),
    };
    with_dom(&ctx, |store| {
        let mut handles = store.handles.borrow_mut();
        store
            .doc
            .select(&selector)
            .map(|el| {
                handles.push(el.id());
                (handles.len() - 1) as i64
            })
            .collect()
    })
}

fn get_element(store: &DomStore, handle: i64) -> Option<ElementRef<'_>> {
    let id = *store.handles.borrow().get(usize::try_from(handle).ok()?)?;
    ElementRef::wrap(store.doc.tree.get(id)?)
}

/// `text(el)` → 元素文本；句柄非法返回 null
fn js_text(handle: i64) -> rquickjs::Result<Option<String>> {
    DOM.with(|d| {
        let borrowed = d.borrow();
        let Some(store) = borrowed.as_ref() else {
            return Ok(None);
        };
        Ok(get_element(store, handle).map(|el| el.text().collect::<String>()))
    })
}

/// `html(el)` → 元素 innerHTML；句柄非法返回 null
fn js_html(handle: i64) -> rquickjs::Result<Option<String>> {
    DOM.with(|d| {
        let borrowed = d.borrow();
        let Some(store) = borrowed.as_ref() else {
            return Ok(None);
        };
        Ok(get_element(store, handle).map(|el| el.inner_html()))
    })
}

/// `attr(el, name)` → 属性值，不存在返回 null
fn js_attr(handle: i64, name: String) -> rquickjs::Result<Option<String>> {
    DOM.with(|d| {
        let borrowed = d.borrow();
        let Some(store) = borrowed.as_ref() else {
            return Ok(None);
        };
        Ok(get_element(store, handle).and_then(|el| el.value().attr(&name).map(str::to_owned)))
    })
}

/// `cleanHtml(html)` → 白名单清洗后的语义化 HTML（相对 URL 按 CLEANER_BASE 绝对化）
fn js_clean_html(html: String) -> String {
    CLEANER_BASE.with(|b| build_cleaner(&b.borrow()).clean(&html).to_string())
}

/// `log(msg)` → 进 tracing，供 AI 修复时查看脚本执行日志
fn js_log(msg: String) {
    tracing::info!(target: "adapter_script", "{msg}");
}

fn register_host_api(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    let g = ctx.globals();
    g.set("select", Func::new(js_select))?;
    g.set("selectAll", Func::new(js_select_all))?;
    g.set("text", Func::new(js_text))?;
    g.set("html", Func::new(js_html))?;
    g.set("attr", Func::new(js_attr))?;
    g.set("cleanHtml", Func::new(js_clean_html))?;
    g.set("log", Func::new(js_log))?;
    Ok(())
}

/// 提取 JS 异常内容作为错误信息
fn script_error(ctx: &Ctx<'_>, e: &rquickjs::Error, timed_out: bool) -> EngineError {
    if timed_out {
        return EngineError::Script("脚本执行超时".to_string());
    }
    if matches!(e, rquickjs::Error::Exception) {
        let v = ctx.catch();
        if let Some(obj) = v.as_object()
            && let Ok(msg) = obj.get::<_, String>("message")
        {
            return EngineError::Script(msg);
        }
        return EngineError::Script(format!("{v:?}"));
    }
    EngineError::Script(e.to_string())
}

/// 在沙箱中执行脚本并调用指定入口表达式，入口须返回可 JSON 序列化的值
///
/// - `doc_html`：提供 DOM 查询的文档（区域提取 + 脱敏后的合成文档）；提交脚本可传 `None`
/// - `call_snippet`：调用入口的 JS 表达式，其返回值经 `JSON.stringify` 取回
fn run_script(
    doc_html: Option<&str>,
    script: &str,
    call_snippet: &str,
    cfg: &EngineConfig,
) -> Result<serde_json::Value, EngineError> {
    if let Some(doc) = doc_html {
        DOM.with(|d| {
            *d.borrow_mut() = Some(DomStore {
                doc: Html::parse_document(doc),
                handles: RefCell::new(Vec::new()),
            });
        });
    }
    CLEANER_BASE.with(|b| {
        *b.borrow_mut() = cfg.base_url.clone();
    });
    // 无论成败，离开前清理文档存储
    let result = run_script_inner(script, call_snippet, cfg);
    DOM.with(|d| {
        *d.borrow_mut() = None;
    });
    CLEANER_BASE.with(|b| {
        b.borrow_mut().clear();
    });
    result
}

fn run_script_inner(
    script: &str,
    call_snippet: &str,
    cfg: &EngineConfig,
) -> Result<serde_json::Value, EngineError> {
    let runtime = Runtime::new().map_err(|e| EngineError::Host(e.to_string()))?;
    runtime.set_memory_limit(cfg.memory_limit);
    runtime.set_max_stack_size(cfg.max_stack_size);
    let deadline = Instant::now() + cfg.timeout;
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

    let context = Context::full(&runtime).map_err(|e| EngineError::Host(e.to_string()))?;
    context.with(|ctx| {
        register_host_api(&ctx).map_err(|e| EngineError::Host(e.to_string()))?;

        ctx.eval::<(), _>(script)
            .map_err(|e| script_error(&ctx, &e, false))?;

        let out: String = ctx
            .eval(call_snippet)
            .map_err(|e| script_error(&ctx, &e, Instant::now() >= deadline))?;

        serde_json::from_str(&out)
            .map_err(|e| EngineError::Host(format!("脚本输出不是合法 JSON: {e}")))
    })
}

/// 运行页面解析脚本：调用脚本的 `parse()`，返回其 JSON 输出
pub fn run_parse(
    doc_html: &str,
    script: &str,
    cfg: &EngineConfig,
) -> Result<serde_json::Value, EngineError> {
    run_script(Some(doc_html), script, "JSON.stringify(parse())", cfg)
}

/// 运行提交脚本：调用脚本的 `buildPlan(input)`，返回提交计划 JSON
pub fn run_build_plan(
    script: &str,
    input: &serde_json::Value,
    cfg: &EngineConfig,
) -> Result<serde_json::Value, EngineError> {
    let input_json = serde_json::to_string(input)
        .map_err(|e| EngineError::Host(format!("提交输入序列化失败: {e}")))?;
    // JSON 字符串字面量直接嵌入 JS（JSON 转义是合法的 JS 字符串转义）
    let input_literal =
        serde_json::to_string(&input_json).map_err(|e| EngineError::Host(e.to_string()))?;
    let call = format!("JSON.stringify(buildPlan(JSON.parse({input_literal})))");
    run_script(None, script, &call, cfg)
}
