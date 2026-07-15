# AI 集成说明

## 支持的 API

当前使用 **DeepSeek API**，默认 endpoint 为 `https://api.deepseek.com`。

支持任何 **OpenAI 兼容 API**（如 Ollama、vLLM、OpenRouter 等），用户可在设置中配置自定义 `base_url` 和 `model`。

## SSE 流格式

AI 聊天端点 `POST /api/ai/chat` 返回 SSE（Server-Sent Events）流：

```
data: {"content":"令牌","finish_reason":null}
data: {"content":"数","finish_reason":null}
data: {"content":"","finish_reason":"stop"}
```

每条 `data:` 行包含一个 JSON 对象：
- `content`: 本次 delta 的文本内容（空串表示流结束）
- `finish_reason`: 结束原因，`null` 表示流未结束，`"stop"` 表示正常结束

## 凭据存储

API Key 由服务端加密存储，详见 [credentials.md](credentials.md)。

## 用户配置

1. 在右侧 AI 面板点击「设置」
2. 输入 DeepSeek API Key（格式：`sk-...`）
3. 保存后 API Key 发送到服务端加密存储（AES-GCM），加密密钥由操作系统密钥环保护
4. 支持修改 `base_url` 和 `model`

## 架构

```
前端 (同源 localhost:20365)
  ↓ POST /api/ai/chat (不含 api_key)
server
  ↓ core::config::load() → 解密配置 → 获取 api_key
  ↓ core::ai::stream_chat()
  ↓ reqwest → DeepSeek API (SSE)
  ↓ mpsc channel → SSE response → 前端
```

Core crate 通过 `mpsc` channel 将上游 SSE chunk 转发给 axum SSE handler，实现实时流式传输。
