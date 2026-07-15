use axum::{Json, extract::Path, extract::State, http::StatusCode};
use hnu_cg_helper_core::{
    CgAssignment, CgCourse, CgProblem, CgToken,
    course::{
        get_assignment_list as core_get_assignments, get_course_list as core_get_courses,
        get_problem_list as core_get_problems, get_problem_page as core_get_page,
    },
};

use crate::state::AppState;

/// 从 state 中提取 CgToken
async fn token_from_state(
    state: &AppState,
) -> Result<CgToken, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    state
        .current_token
        .read()
        .await
        .clone()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(hnu_cg_helper_core::error::ErrorResponse {
                    error: "Not authenticated".into(),
                }),
            )
        })
}

/// GET /api/courses
pub async fn get_courses(
    State(state): State<AppState>,
) -> Result<Json<Vec<CgCourse>>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let token = token_from_state(&state).await?;
    let courses = core_get_courses(&token).await.map_err(|e| {
        tracing::error!(error = %e, "获取课程列表失败");
        (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
    })?;
    Ok(Json(courses))
}

/// GET /api/courses/:course_id/assignments
pub async fn get_assignments(
    State(state): State<AppState>,
    Path(course_id): Path<u32>,
) -> Result<Json<Vec<CgAssignment>>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let token = token_from_state(&state).await?;
    let assignments = core_get_assignments(&token, course_id).await.map_err(|e| {
        tracing::error!(error = %e, "获取作业列表失败");
        (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
    })?;
    Ok(Json(assignments))
}

/// GET /api/courses/:course_id/assignments/:assign_id/problems
pub async fn get_problems(
    State(state): State<AppState>,
    Path((_course_id, assign_id)): Path<(u32, u32)>,
) -> Result<Json<Vec<CgProblem>>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let token = token_from_state(&state).await?;
    let problems = core_get_problems(&token, assign_id).await.map_err(|e| {
        tracing::error!(error = %e, "获取题目列表失败");
        (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
    })?;
    Ok(Json(problems))
}

#[derive(serde::Serialize)]
pub(crate) struct ProblemPageResponse {
    pub html: String,
}

/// GET /api/courses/:course_id/assignments/:assign_id/problems/:pro_num
pub async fn get_problem_page(
    State(state): State<AppState>,
    Path((_course_id, assign_id, pro_num)): Path<(u32, u32, u32)>,
) -> Result<Json<ProblemPageResponse>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)>
{
    let token = token_from_state(&state).await?;
    let html = core_get_page(&token, assign_id, pro_num)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "获取题目详情失败");
            (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
        })?;
    Ok(Json(ProblemPageResponse { html }))
}
