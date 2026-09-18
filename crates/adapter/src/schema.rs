use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 一种可选编程语言
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    /// 提交参数中的值（如 `c++`）
    pub value: String,
    /// 展示名
    pub label: String,
}

/// 填空题的一个空位
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    /// 表单字段名（如 `answer1`）
    pub name: String,
}

/// 填空题代码骨架的一段（代码片段与空位按文档顺序交替）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkeletonPart {
    /// 代码片段（已解码的纯文本）
    Code {
        /// 代码内容
        text: String,
    },
    /// 空位
    Gap {
        /// 对应的表单字段名
        name: String,
    },
}

/// 提交描述：告诉 GUI 这道题怎么交（problem-page 脚本解析产出）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubmissionDescriptor {
    /// 普通编程题：multipart 文件上传
    FileUpload {
        /// 可选语言列表
        languages: Vec<Language>,
        /// 选 java 时是否需要填主类名
        needs_main_class: bool,
        /// 题目 ID
        problem_id: u64,
        /// 作业 ID
        assign_id: u64,
    },
    /// 程序填空题：urlencoded 表单，按空位提交
    FillGap {
        /// 可选语言列表（填空题通常为页面写死的一种）
        languages: Vec<Language>,
        /// 空位列表
        gaps: Vec<Gap>,
        /// 代码骨架：代码片段与空位按顺序交替，供 GUI 渲染填空练习
        skeleton: Vec<SkeletonPart>,
        /// 页面 hidden 字段原值（提交时回带）
        hidden_fields: BTreeMap<String, String>,
        /// 题目 ID
        problem_id: u64,
        /// 作业 ID
        assign_id: u64,
    },
}

impl SubmissionDescriptor {
    /// 题目 ID（CG problemID）
    pub fn problem_id(&self) -> u64 {
        match self {
            Self::FileUpload { problem_id, .. } | Self::FillGap { problem_id, .. } => *problem_id,
        }
    }

    /// 作业 ID
    pub fn assign_id(&self) -> u64 {
        match self {
            Self::FileUpload { assign_id, .. } | Self::FillGap { assign_id, .. } => *assign_id,
        }
    }
}

/// 题目页脚本（problem-*.js 的 `parse()`）的输出契约
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProblemPageOutput {
    /// 语义化题面 HTML（经 cleanHtml 白名单清洗，表现层属性已剥除）
    pub statement_html: String,
    /// 题面纯文本（供 AI 上下文使用）
    pub statement_text: String,
    /// 提交描述
    pub submission: SubmissionDescriptor,
}

/// 提交脚本 `buildPlan()` 的输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitInput {
    /// problem-page 脚本产出的提交描述（原样透传给提交脚本）
    pub descriptor: serde_json::Value,
    /// 用户选择的语言
    pub language: String,
    /// java 主类名（需要时）
    #[serde(default)]
    pub main_class: Option<String>,
    /// 普通编程题：源代码内容（由宿主封装为文件）
    #[serde(default)]
    pub code: Option<String>,
    /// 填空题：字段名 → 答案
    #[serde(default)]
    pub answers: Option<BTreeMap<String, String>>,
    /// 耗时（秒），由 GUI 计时
    #[serde(default)]
    pub wtime: u64,
}

/// 提交计划：提交脚本的输出，宿主机械执行其中的 HTTP 请求
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmissionPlan {
    /// HTTP 方法（通常 POST）
    pub method: String,
    /// 请求 URL（可为相对站点根的路径，由宿主解析）
    pub url: String,
    /// 请求体
    pub body: PlanBody,
}

/// 提交计划请求体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanBody {
    /// multipart 文件上传；文件内容由宿主按 `file_field` 附上用户代码
    Multipart {
        /// 文件字段名
        file_field: String,
        /// 额外表单字段
        fields: BTreeMap<String, String>,
    },
    /// application/x-www-form-urlencoded 表单
    Form {
        /// 表单字段
        fields: BTreeMap<String, String>,
    },
}
