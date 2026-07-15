# HTTP API 文档

## 概述

API 与前端 SPA 由同一服务进程提供，监听同一端口（默认 `20365`），同源访问，无需 CORS。

Base URL: `http://localhost:20365/api`

## 认证

除 ai 相关端点外，所有接口均需在 HTTP Header 中携带 token：

```
Authorization: Bearer <base64_encoded_token_json>
```

Token 通过登录接口获取，是 `CgToken` 的 JSON 序列化 + base64 编码。

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
  "token": "base64-encoded-token-json"
}
```

**Errors**: `401` 验证码错误/密码错误

### `GET /api/courses`

获取课程列表。

**Headers**: `Authorization: Bearer <token>`

**Response** `200`:
```json
[
  { "course_id": 123, "course_name": "课程名" }
]
```

### `GET /api/courses/{course_id}/assignments`

获取课程作业列表。

**Response** `200`:
```json
[
  { "assign_id": 456, "assign_name": "作业名" }
]
```

### `GET /api/courses/{course_id}/assignments/{assign_id}/problems`

获取作业题目列表。

**Response** `200`:
```json
[
  { "pro_num": 1, "problem_id": 789, "title": "题目标题", "score": 10.0 }
]
```

### `GET /api/courses/{course_id}/assignments/{assign_id}/problems/{pro_num}`

获取题目详情页 HTML。

**Response** `200`:
```json
{
  "html": "<div>题目内容...</div>"
}
```

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
