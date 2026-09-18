pub mod ai;
pub mod auth;
pub mod config;
pub mod course;
pub mod error;
pub mod problem;
pub mod repair;

pub use ai::stream_chat;
pub use auth::{create_session, deserialize_token, login, serialize_token};
pub use config::{AiConfigView, ConfigManager};
pub use course::{get_assignment_list, get_course_list, get_problem_list, get_problem_page};
pub use error::CoreError;
pub use hnu_query::cg::course::{CgAssignment, CgCourse, CgProblem};
pub use hnu_query::cg::login::{CgSession, CgToken};
pub use problem::{
    CG_BASE_URL, ProblemFailure, ProblemSuccess, build_submission_plan, execute_submission,
    process_problem_page,
};
