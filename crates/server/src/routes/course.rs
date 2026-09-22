use axum::{Json, extract::Path, extract::State, http::StatusCode};
use hnu_cg_helper_core::repair;
use hnu_cg_helper_core::repair::SceneData;
use hnu_cg_helper_core::{
    CgAssignment, CgCourse, CgProblem, CgToken,
    course::{
        get_assignment_list as core_get_assignments, get_course_list as core_get_courses,
        get_problem_list as core_get_problems, get_problem_page as core_get_page,
    },
    problem::{
        ProblemFailure, ProblemSuccess, build_submission_plan, execute_submission,
        process_problem_page,
    },
};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::state::AppState;

/// 从 state 中提取 CgToken
async fn token_from_state(
    state: &AppState,
) -> Result<CgToken, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    state.current_token.read().await.clone().ok_or_else(|| {
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

/// 题目页失败的错误响应：repairable 标记是否可尝试 AI 修复
#[derive(Serialize)]
pub(crate) struct ProblemErrorResponse {
    error: String,
    repairable: bool,
    page_type: Option<String>,
}

/// 留存失败现场到本地（供 AI 修复使用；仅存脱敏后内容，目录在用户数据目录）
fn save_scene(state: &AppState, failure: &ProblemFailure) {
    let page_type = failure
        .page_type
        .clone()
        .unwrap_or_else(|| "unknown".into());
    let scene = SceneData {
        page_type: failure.page_type.clone(),
        script_input: failure.script_input.clone(),
        error: failure.error.to_string(),
        captured_at: format!("{:?}", std::time::SystemTime::now()),
    };
    let path = state.scenes_dir.join(format!("{page_type}.json"));
    if let Err(e) = std::fs::create_dir_all(&state.scenes_dir)
        .and_then(|_| std::fs::write(&path, serde_json::to_string_pretty(&scene)?))
    {
        tracing::warn!(error = %e, "失败现场留存失败");
    }
}

/// GET /api/courses/:course_id/assignments/:assign_id/problems/:pro_num
///
/// 题目页结构化输出：原始页面经适配管线（检测 → 脱敏 → 脚本解析）产出
/// `{ page_type, statement_html, statement_text, submission }`。
pub async fn get_problem_page(
    State(state): State<AppState>,
    Path((_course_id, assign_id, pro_num)): Path<(u32, u32, u32)>,
) -> Result<Json<ProblemSuccess>, (StatusCode, Json<ProblemErrorResponse>)> {
    let token = token_from_state(&state).await.map_err(|(code, e)| {
        (
            code,
            Json(ProblemErrorResponse {
                error: e.0.error.clone(),
                repairable: false,
                page_type: None,
            }),
        )
    })?;
    let raw = core_get_page(&token, assign_id, pro_num)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "获取题目页面失败");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProblemErrorResponse {
                    error: e.to_string(),
                    repairable: false,
                    page_type: None,
                }),
            )
        })?;

    let known: Vec<String> = state
        .current_stu_id
        .read()
        .await
        .clone()
        .into_iter()
        .collect();
    let registry = state.adapters.clone();
    let cfg = state.engine_cfg.clone();

    let result =
        tokio::task::spawn_blocking(move || process_problem_page(&registry, &raw, &known, &cfg))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ProblemErrorResponse {
                        error: format!("解析任务失败: {e}"),
                        repairable: false,
                        page_type: None,
                    }),
                )
            })?;

    match result {
        Ok(success) => Ok(Json(success)),
        Err(failure) => {
            tracing::warn!(error = %failure.error, page_type = ?failure.page_type, "题目页解析失败");
            save_scene(&state, &failure);
            Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ProblemErrorResponse {
                    repairable: failure.repairable(),
                    page_type: failure.page_type.clone(),
                    error: failure.error.to_string(),
                }),
            ))
        }
    }
}

/// 提交请求体
#[derive(serde::Deserialize)]
pub(crate) struct SubmitRequest {
    /// 页面类型（决定用哪个提交脚本）
    page_type: String,
    /// GET 题目页返回的 submission 描述（原样回传）；
    /// 类型化反序列化：problem_id/assign_id 为 u64，非法形状直接 400
    descriptor: hnu_cg_helper_adapter::SubmissionDescriptor,
    /// 选择的语言
    language: String,
    /// java 主类名
    main_class: Option<String>,
    /// 普通编程题：源代码
    code: Option<String>,
    /// 填空题：字段名 → 答案
    answers: Option<BTreeMap<String, String>>,
    /// 耗时秒数
    wtime: Option<u64>,
}

/// 提交响应
#[derive(Serialize)]
pub(crate) struct SubmitResponse {
    /// CG 结果页原始 HTML（前端以沙箱 iframe 展示）
    result_html: String,
}

/// POST /api/courses/:course_id/assignments/:assign_id/problems/:pro_num/submit
///
/// 提交作答：提交脚本产出计划，宿主带 CG 会话执行。
pub async fn submit_problem(
    State(state): State<AppState>,
    Path((_course_id, assign_id, pro_num)): Path<(u32, u32, u32)>,
    Json(req): Json<SubmitRequest>,
) -> Result<Json<SubmitResponse>, (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>)> {
    let token = token_from_state(&state).await?;

    // 描述符与 URL 路径交叉校验：防止过期/伪造描述符把代码交到别的题目
    if req.descriptor.assign_id() != u64::from(assign_id) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: "提交描述与当前作业不一致，请刷新页面重试".into(),
            }),
        ));
    }
    let problems = core_get_problems(&token, assign_id).await.map_err(|e| {
        tracing::error!(error = %e, "提交前校验题目列表失败");
        (StatusCode::INTERNAL_SERVER_ERROR, Json((&e).into()))
    })?;
    let expected = problems.iter().find(|p| p.index == pro_num);
    match expected {
        Some(p) if u64::from(p.id) == req.descriptor.problem_id() => {}
        _ => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(hnu_cg_helper_core::error::ErrorResponse {
                    error: "提交描述与当前题目不一致，请刷新页面重试".into(),
                }),
            ));
        }
    }

    let input = hnu_cg_helper_adapter::SubmitInput {
        descriptor: serde_json::to_value(&req.descriptor).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(hnu_cg_helper_core::error::ErrorResponse {
                    error: format!("提交描述序列化失败: {e}"),
                }),
            )
        })?,
        language: req.language.clone(),
        main_class: req.main_class.clone(),
        code: req.code.clone(),
        answers: req.answers,
        wtime: req.wtime.unwrap_or(0),
    };

    let registry = state.adapters.clone();
    let cfg = state.engine_cfg.clone();
    let page_type = req.page_type.clone();
    let plan = tokio::task::spawn_blocking(move || {
        build_submission_plan(&registry, &page_type, &input, &cfg)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: format!("提交任务失败: {e}"),
            }),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: format!("生成提交计划失败: {e}"),
            }),
        )
    })?;

    let result_html = execute_submission(
        &token,
        &plan,
        req.code.as_deref(),
        &req.language,
        req.main_class.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "提交执行失败");
        (StatusCode::BAD_GATEWAY, Json((&e).into()))
    })?;

    Ok(Json(SubmitResponse { result_html }))
}

/// 修复请求体
#[derive(serde::Deserialize)]
pub(crate) struct RepairRequest {
    /// 要修复的页面类型
    page_type: String,
}

/// POST /api/adapter/repair
///
/// AI 修复端点：读取留存的失败现场，驱动修复循环，SSE 推送进度。
pub async fn repair_adapter(
    State(state): State<AppState>,
    Json(req): Json<RepairRequest>,
) -> Result<
    axum::response::Sse<
        impl futures_core::Stream<Item = Result<axum::response::sse::Event, axum::Error>>,
    >,
    (StatusCode, Json<hnu_cg_helper_core::error::ErrorResponse>),
> {
    use axum::response::sse::{Event, KeepAlive};
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::ReceiverStream;

    // page_type 白名单：必须存在于适配包 manifest。
    // 既是业务校验（只有已知页面类型才有修复意义），也杜绝路径穿越——
    // 该值随后会拼进场景文件路径。
    let known = state.adapters.current();
    if !known
        .manifest
        .page_types
        .iter()
        .any(|p| p.id == req.page_type)
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: format!("未知页面类型: {}", req.page_type),
            }),
        ));
    }
    drop(known);

    let scene_path = state.scenes_dir.join(format!("{}.json", req.page_type));
    let scene_text = std::fs::read_to_string(&scene_path).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: "没有找到该页面类型的失败现场".into(),
            }),
        )
    })?;
    let scene: SceneData = serde_json::from_str(&scene_text).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(hnu_cg_helper_core::error::ErrorResponse {
                error: format!("失败现场数据损坏: {e}"),
            }),
        )
    })?;

    let (api_key, view) = {
        let config = state.config.read().await;
        let key = config.ai_api_key().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(hnu_cg_helper_core::error::ErrorResponse {
                    error: "请先在 AI 设置中配置 API Key".into(),
                }),
            )
        })?;
        (key, config.ai_config_view())
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<repair::RepairEvent>(32);
    let registry = state.adapters.clone();
    let engine_cfg = state.engine_cfg.clone();
    let page_type = req.page_type.clone();

    tokio::spawn(async move {
        let ai_cfg = repair::RepairConfig {
            api_key,
            base_url: view.base_url,
            model: view.model,
        };
        if let Err(e) =
            repair::repair_script(registry, engine_cfg, page_type, scene, ai_cfg, tx.clone()).await
        {
            let _ = tx
                .send(repair::RepairEvent {
                    stage: "failed".into(),
                    attempt: 0,
                    message: format!("修复流程出错: {e}"),
                })
                .await;
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        let data = serde_json::to_string(&event)
            .map_err(|e| axum::Error::new(std::io::Error::other(e)))?;
        Ok(Event::default().data(data))
    });

    Ok(axum::response::Sse::new(stream).keep_alive(KeepAlive::default()))
}
