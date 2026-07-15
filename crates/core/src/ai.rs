use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::CoreError;

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// AI 聊天请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

fn default_base_url() -> String {
    "https://api.deepseek.com".into()
}

fn default_model() -> String {
    "deepseek-v4-flash".into()
}

/// SSE 流中的单个 delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub content: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// 流式 AI 聊天
///
/// 发送请求到 OpenAI 兼容 API（默认 DeepSeek），通过 mpsc channel **实时**返回 SSE chunk。
/// 使用 `bytes_stream()` 增量读取上游响应，每收到一个完整的 SSE 事件立即通过 channel 发送。
/// channel 在流结束或接收端断开时自动关闭。
pub async fn stream_chat(req: ChatRequest, tx: mpsc::Sender<ChatChunk>) -> Result<(), CoreError> {
    use futures::StreamExt;

    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": req.model,
        "messages": req.messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        }).collect::<Vec<_>>(),
        "stream": true,
    });

    let response = client
        .post(format!(
            "{}/chat/completions",
            req.base_url.trim_end_matches('/')
        ))
        .header("Authorization", format!("Bearer {}", req.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CoreError::Ai(format!("请求失败: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(CoreError::Ai(format!("HTTP {status}: {text}")));
    }

    // 增量读取上游 SSE 流，每收到一个完整行就实时转发
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| CoreError::Ai(format!("读取流失败: {e}")))?;
        // SAFETY: SSE data from OpenAI-compatible APIs is UTF-8 text
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // 处理 buffer 中已完整的行（以 \n 分隔）
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            // 消耗已处理的行（含换行符）
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || !line.starts_with("data: ") {
                continue;
            }

            let data = &line[6..]; // 去掉 "data: " 前缀
            if data == "[DONE]" {
                return Ok(());
            }

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(choices) = parsed["choices"].as_array()
                && let Some(delta) = choices.first()
            {
                let content = delta["delta"]["content"].as_str().unwrap_or("").to_string();
                let finish_reason = delta["finish_reason"].as_str().map(String::from);

                let is_stop = finish_reason.is_some();

                if tx
                    .send(ChatChunk {
                        content,
                        finish_reason,
                    })
                    .await
                    .is_err()
                {
                    // receiver dropped, stop streaming
                    return Ok(());
                }

                if is_stop {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}
