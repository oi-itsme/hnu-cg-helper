# HTTP API 文档

## 概述

API 与前端 SPA 由同一服务进程提供，监听同一端口（默认 `20365`），同源访问，无需 CORS。

Base URL: `http://localhost:20365/api`

## 认证

登录成功后 token 保存在服务端内存（单用户本地应用），后续请求无需携带凭证。

---

## 端点

### `POST /api/auth/captcha`

创建登录会话，返回验证码图片。

**Response** `200`:
```json
{
  "session_id": "uuid-string",
  "captcha_image": "base64-encoded-png"
}
```

### `POST /api/auth/login`

使用学号、密码和验证码完成登录。

**Request**:
```json
{
  "session_id": "uuid-string",
  "stu_id": "学号",
  "password": "密码",
  "captcha_code": "验证码"
}
```

**Response** `200`:
```json
{
  "success": true
}
```

**Errors**: `401` 验证码错误/密码错误

### `GET /api/courses`

获取课程列表。

**Headers**: `Authorization: Bearer <token>`

**Response** `200`:
```json
[
  { "id": 123, "name": "课程名" }
]
```

### `GET /api/courses/{course_id}/assignments`

获取课程作业列表。

**Response** `200`:
```json
[
  { "id": 456, "name": "作业名" }
]
```

### `GET /api/courses/{course_id}/assignments/{assign_id}/problems`

获取作业题目列表。

**Response** `200`:
```json
[
  { "index": 1, "id": 789, "title": "题目标题", "score": 10.0 }
]
```

### `GET /api/courses/{course_id}/assignments/{assign_id}/problems/{pro_num}`

获取题目页结构化输出（经站点适配框架解析，见 [adapter-framework.md](adapter-framework.md)）。

**Response** `200`:
```json
{
  "page_type": "problem-program",
  "statement_html": "<p>语义化题面...</p>",
  "statement_text": "题面纯文本...",
  "submission": {
    "kind": "file_upload",
    "languages": [{ "value": "c", "label": "c" }],
    "needs_main_class": true,
    "problem_id": 22677,
    "assign_id": 1609
  }
}
```

`submission.kind` 为 `fill_gap` 时字段为 `languages` / `gaps` / `skeleton` / `hidden_fields` / `problem_id` / `assign_id`。

**Errors**: `422` 解析失败，响应体：
```json
{ "error": "错误信息", "repairable": true, "page_type": "problem-program" }
```
`repairable=true` 表示脚本层故障，可调用 `/api/adapter/repair` 尝试 AI 修复。

### `POST /api/courses/{course_id}/assignments/{assign_id}/problems/{pro_num}/submit`

提交作答。服务端用提交脚本生成提交计划并带 CG 会话执行。

**Request**:
```json
{
  "page_type": "problem-program",
  "descriptor": { "kind": "file_upload", "...": "GET 返回的 submission 原样回传" },
  "language": "c++",
  "main_class": null,
  "code": "源代码（file_upload 时）",
  "answers": { "answer1": "..." },
  "wtime": 120
}
```

**Response** `200`:
```json
{ "result_html": "CG 结果页原始 HTML（前端沙箱 iframe 展示）" }
```

**Errors**: `400` 请求体不合法（descriptor 形状/类型不符，problem_id/assign_id 为 u64）；`422` descriptor 与 URL 路径不一致（assign_id 不符，或 problem_id 与题目序号映射不符，需刷新页面重试）

### `POST /api/adapter/repair`

AI 修复端点：读取留存的失败现场，驱动"LLM 改脚本 → 验证 → 热重载"循环，SSE 推送进度。

**Request**:
```json
{ "page_type": "problem-program" }
```

**Response** `200` (text/event-stream):
```
data: {"stage":"attempt","attempt":1,"message":"第 1/3 轮：请求 AI 重写脚本…"}
data: {"stage":"validating","attempt":1,"message":"收到候选脚本，正在沙箱中验证…"}
data: {"stage":"done","attempt":1,"message":"验证全部通过，新脚本已生效。刷新页面即可。"}
```

`stage` 取值：`attempt` / `validating` / `done` / `failed`。

**Errors**: `404` 未知页面类型（page_type 白名单校验，必须是适配包 manifest 中已知的 id）或无该题型失败现场；`400` 未配置 AI API Key


### `POST /api/ai/chat`

流式 AI 聊天 (SSE)。API Key 由服务端托管，请求体不再携带。

**Request**:
```json
{
  "messages": [
    { "role": "user", "content": "问题内容" }
  ]
}
```

`model` 和 `base_url` 使用已保存的服务端配置，无需每次传入。

**Response** `200` (text/event-stream):
```
data: {"content":"你","finish_reason":null}
data: {"content":"好","finish_reason":null}
data: {"content":"","finish_reason":"stop"}
```

### `POST /api/ai/config`

保存 AI 配置（API Key、模型等）。数据经 AES-GCM 加密后持久化到服务端。
当 OS 密钥环不可用时，API Key 仅保存在当前会话内存中。

**Request**:
```json
{
  "api_key": "sk-...",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-v4-flash"
}
```

**Response** `200`:
```json
{
  "has_api_key": true,
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-v4-flash"
}
```

### `GET /api/ai/config`

获取当前 AI 配置（不含 API Key 明文）。API Key 只返回是否已配置的状态。

**Response** `200`:
```json
{
  "has_api_key": true,
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-v4-flash"
}
```
