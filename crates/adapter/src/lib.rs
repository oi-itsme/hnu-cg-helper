//! hnu-cg-helper 的站点适配框架。
//!
//! 架构：固定输入（原始页面）→ 固定层脱敏（区域提取 + 模式擦除）→
//! 脚本层（QuickJS 沙箱，AI 可维护、可热重载）→ 固定输出（结构化题面 + 提交计划）。
//!
//! 隐私红线：只有经固定层脱敏的 HTML 才能进入脚本层；
//! 脚本不接触网络，提交由脚本产出"提交计划"、宿主带会话执行。

pub mod adapter;
pub mod engine;
pub mod error;
pub mod manifest;
pub mod registry;
pub mod sanitize;
pub mod schema;
pub mod validate;

pub use adapter::{LoadedAdapter, PageScripts};
pub use engine::{EngineConfig, EngineError};
pub use error::AdapterError;
pub use manifest::{AdapterManifest, PageTypeConfig};
pub use registry::AdapterRegistry;
pub use schema::{
    Gap, Language, PlanBody, ProblemPageOutput, SkeletonPart, SubmissionDescriptor, SubmissionPlan,
    SubmitInput,
};
pub use validate::{FixtureFailure, ValidationReport};
