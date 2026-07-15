use hnu_query::cg::{CgAssignment, CgCourse, CgProblem, CgToken};

use crate::error::CoreError;

/// 获取当前账号的课程列表
pub async fn get_course_list(token: &CgToken) -> Result<Vec<CgCourse>, CoreError> {
    let courses = hnu_query::cg::get_course_list(token).await?;
    Ok(courses)
}

/// 获取指定课程的作业列表
pub async fn get_assignment_list(
    token: &CgToken,
    course_id: u32,
) -> Result<Vec<CgAssignment>, CoreError> {
    let assignments = hnu_query::cg::get_assignment_list(token, course_id).await?;
    Ok(assignments)
}

/// 获取作业的题目列表
pub async fn get_problem_list(
    token: &CgToken,
    course_id: u32,
    assign_id: u32,
) -> Result<Vec<CgProblem>, CoreError> {
    let problems = hnu_query::cg::get_problem_list(token, course_id, assign_id).await?;
    Ok(problems)
}

/// 获取题目详情页的原始 HTML
pub async fn get_problem_page(
    token: &CgToken,
    course_id: u32,
    assign_id: u32,
    pro_num: u32,
) -> Result<String, CoreError> {
    let html = hnu_query::cg::get_problem_page(token, course_id, assign_id, pro_num).await?;
    Ok(html)
}
