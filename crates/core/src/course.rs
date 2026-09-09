use hnu_query::cg::course::{CgAssignment, CgCourse, CgProblem};
use hnu_query::cg::login::CgToken;

use crate::error::CoreError;

/// 获取当前账号的课程列表
pub async fn get_course_list(token: &CgToken) -> Result<Vec<CgCourse>, CoreError> {
    let courses = hnu_query::cg::get_course_list(token).await?;
    // hnu_query 用 None 表示账号下没有课程，统一展开为空数组
    Ok(courses.unwrap_or_default())
}

/// 获取指定课程的作业列表
pub async fn get_assignment_list(
    token: &CgToken,
    course_id: u32,
) -> Result<Vec<CgAssignment>, CoreError> {
    let assignments = hnu_query::cg::get_assignment_list(token, course_id).await?;
    // hnu_query 用 None 表示该课程没有作业，统一展开为空数组
    Ok(assignments.unwrap_or_default())
}

/// 获取作业的题目列表
pub async fn get_problem_list(
    token: &CgToken,
    assign_id: u32,
) -> Result<Vec<CgProblem>, CoreError> {
    let problems = hnu_query::cg::get_problem_list(token, assign_id).await?;
    // hnu_query 用 None 表示该作业没有题目，统一展开为空数组
    Ok(problems.unwrap_or_default())
}

/// 获取题目详情页的原始 HTML
///
/// `index` 为题目在作业内的序号（1-based），对应 [CgProblem] 的 `index` 字段。
pub async fn get_problem_page(
    token: &CgToken,
    assign_id: u32,
    index: u32,
) -> Result<String, CoreError> {
    let html = hnu_query::cg::get_problem_page(token, assign_id, index).await?;
    Ok(html)
}
