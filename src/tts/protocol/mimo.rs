//! MiMo (小米) TTS v2.5 协议层
//!
//! 基于 MiMo TTS v2.5 HTTP REST API（OpenAI 兼容 Chat Completions + audio 参数）。
//! 对应官方文档：<https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/audio/speech-synthesis-v2.5>
//!
//! # 协议要点
//!
//! - 端点：`POST /v1/chat/completions`，`api-key` 认证
//! - 非流式：响应 JSON 中 `choices[0].message.audio.data` 为 base64 编码音频
//! - 流式：`text/event-stream`，`choices[0].delta.audio.data` 为 base64 编码音频块，
//!   结束标记为 `finish_reason: "stop"` 或 `[DONE]`
//! - 消息结构：`assistant` 存放合成文本，可选 `user` 存放风格指令

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::tts::error::TtsError;

// ============================== 常量 ==============================

/// MiMo TTS 默认 REST 端点
pub const MIMO_DEFAULT_BASE_URL: &str = "https://api.xiaomimimo.com/v1";

/// MiMo TTS 默认模型
pub const MIMO_DEFAULT_MODEL: &str = "mimo-v2.5-tts";

/// MiMo TTS 默认音色
pub const MIMO_DEFAULT_VOICE: &str = "mimo_default";

// ============================== 请求体 ==============================

/// MiMo TTS `chat/completions` 请求体
///
/// 所有 `Option` 字段为 `None` 时不会出现在序列化结果中，
/// 以严格对齐 MiMo API 语义（未设置即使用服务端默认值）。
#[derive(Debug, Clone, Serialize)]
pub struct MimoSpeechRequest {
    pub model: String,
    pub messages: Vec<MimoMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<MimoAudioParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MimoMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MimoAudioParam {
    pub voice: String,
    pub format: String,
}

// ============================== 非流式响应 ==============================

/// MiMo TTS 非流式响应
#[derive(Debug, Deserialize)]
pub struct MimoChatResponse {
    pub choices: Vec<MimoChoice>,
}

#[derive(Debug, Deserialize)]
pub struct MimoChoice {
    pub message: MimoMessageBody,
}

#[derive(Debug, Deserialize)]
pub struct MimoMessageBody {
    #[serde(default)]
    pub audio: Option<MimoAudioData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MimoAudioData {
    /// base64 编码的完整音频
    pub data: String,
}

// ============================== 流式响应（SSE） ==============================

/// MiMo TTS SSE 单帧
#[derive(Debug, Deserialize)]
pub struct MimoStreamChunk {
    #[serde(default)]
    pub choices: Vec<MimoStreamChoice>,
    #[serde(default)]
    pub error: Option<MimoError>,
}

#[derive(Debug, Deserialize)]
pub struct MimoStreamChoice {
    #[serde(default)]
    pub delta: MimoStreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MimoStreamDelta {
    #[serde(default)]
    pub audio: Option<MimoStreamAudio>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MimoStreamAudio {
    /// base64 编码的音频块
    #[serde(default)]
    pub data: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MimoError {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

// ============================== SSE 事件类型 ==============================

/// 单帧 `data:` 解析后的事件
#[derive(Debug, Clone)]
pub enum MimoStreamEvent {
    /// base64 解码后的音频块
    Audio(Vec<u8>),
    /// 合成结束
    Finish,
    /// 业务错误
    Error(MimoError),
}

// ============================== SSE 行解析器 ==============================

/// SSE 行解析器：累积字节流，按 `\n` 切分出完整行
#[derive(Default)]
pub struct SseLineParser {
    buffer: Vec<u8>,
}

impl SseLineParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入新字节，返回由此产生的完整行（已去除行尾 `\r`/`\n`）
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line_bytes);
            lines.push(s.trim_end_matches(['\r', '\n']).to_string());
        }
        lines
    }

    /// 字节流结束时调用，吐出残留的尾行
    pub fn flush(&mut self) -> Vec<String> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let remaining = std::mem::take(&mut self.buffer);
        let s = String::from_utf8_lossy(&remaining);
        let trimmed = s.trim();
        if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        }
    }
}

/// 从一行文本中剥离 `data:` 前缀，返回负载部分；非数据行返回 `None`
pub fn extract_data(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("data:")?;
    Some(rest.trim())
}

// ============================== SSE 数据解析 ==============================

/// 解析一帧 `data:` 负载（JSON 字符串或 `[DONE]`）
pub fn parse_data(data: &str) -> Result<Option<MimoStreamEvent>, TtsError> {
    let data = data.trim();
    if data.is_empty() {
        return Ok(None);
    }
    if data == "[DONE]" {
        return Ok(Some(MimoStreamEvent::Finish));
    }

    let chunk: MimoStreamChunk = serde_json::from_str(data)?;

    // 检查 API 错误
    if let Some(err) = chunk.error {
        if let Some(ref msg) = err.message {
            if !msg.is_empty() {
                return Ok(Some(MimoStreamEvent::Error(err)));
            }
        }
    }

    if let Some(choice) = chunk.choices.first() {
        // 检查 finish_reason
        if let Some(ref reason) = choice.finish_reason {
            if !reason.is_empty() {
                return Ok(Some(MimoStreamEvent::Finish));
            }
        }
        // 提取音频数据
        if let Some(ref audio) = choice.delta.audio {
            if let Some(ref data) = audio.data {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data.as_bytes())
                    .map_err(|e| TtsError::Other(format!("base64 decode error: {e}")))?;
                if !bytes.is_empty() {
                    return Ok(Some(MimoStreamEvent::Audio(bytes)));
                }
            }
        }
    }

    Ok(None)
}

// ============================== 格式映射 ==============================

/// MiMo 支持的音频格式
pub const MIMO_SUPPORTED_FORMATS: &[&str] = &["mp3", "opus", "flac", "wav", "pcm"];

/// 映射通用 format 到 MiMo 接受的格式；未知格式回退 `mp3`
pub fn map_format(format: &str) -> &'static str {
    match format {
        "mp3" => "mp3",
        "opus" | "ogg" | "ogg_opus" => "opus",
        "flac" => "flac",
        "wav" => "wav",
        "pcm" => "pcm",
        _ => "mp3",
    }
}

// ============================== 错误解析 ==============================

/// 解析 MiMo HTTP 错误响应体为 `TtsError`
///
/// 格式：`{"error": {"message": "...", "code": "..."}}`
pub fn parse_error_body(body: &str, status: u16) -> TtsError {
    // 尝试解析 MiMo 错误格式
    if let Ok(chunk) = serde_json::from_str::<MimoStreamChunk>(body) {
        if let Some(err) = chunk.error {
            return TtsError::ServiceError {
                code: err.code.unwrap_or_else(|| status.to_string()),
                message: err
                    .message
                    .unwrap_or_else(|| format!("MiMo TTS HTTP {status}")),
            };
        }
    }
    // 回退：直接提取 error.message
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = val
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            let code = val
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or_default();
            return TtsError::ServiceError {
                code: code.to_string(),
                message: msg.to_string(),
            };
        }
    }
    TtsError::Other(format!("MiMo TTS HTTP {status}: {body}"))
}

/// 内置音色列表
pub fn mimo_voices() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        ("mimo_default", "MiMo Default", Some("zh-CN")),
        ("default_zh", "Default (Chinese)", Some("zh-CN")),
        ("default_en", "Default (English)", Some("en-US")),
        ("Mia", "Mia", Some("zh-CN")),
        ("Chloe", "Chloe", Some("zh-CN")),
        ("Milo", "Milo", Some("en-US")),
        ("Dean", "Dean", Some("en-US")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------- r1: 请求序列化（None 字段不出现） --------

    #[test]
    fn test_r1_request_skip_none() {
        let req = MimoSpeechRequest {
            model: "mimo-v2.5-tts".into(),
            messages: vec![MimoMessage {
                role: "assistant".into(),
                content: "你好".into(),
            }],
            audio: None,
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"model":"mimo-v2.5-tts","messages":[{"role":"assistant","content":"你好"}]}"#
        );
    }

    // -------- r2: 请求序列化（完整参数） --------

    #[test]
    fn test_r2_request_full() {
        let req = MimoSpeechRequest {
            model: "mimo-v2.5-tts".into(),
            messages: vec![
                MimoMessage {
                    role: "user".into(),
                    content: "明亮自然的声音".into(),
                },
                MimoMessage {
                    role: "assistant".into(),
                    content: "你好世界".into(),
                },
            ],
            audio: Some(MimoAudioParam {
                voice: "Mia".into(),
                format: "mp3".into(),
            }),
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""model":"mimo-v2.5-tts""#));
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""role":"assistant""#));
        assert!(json.contains(r#""audio""#));
        assert!(json.contains(r#""voice":"Mia""#));
        assert!(!json.contains(r#""stream""#));
    }

    // -------- r3: 流式请求 --------

    #[test]
    fn test_r3_request_stream() {
        let req = MimoSpeechRequest {
            model: "mimo-v2.5-tts".into(),
            messages: vec![MimoMessage {
                role: "assistant".into(),
                content: "hi".into(),
            }],
            audio: Some(MimoAudioParam {
                voice: "mimo_default".into(),
                format: "mp3".into(),
            }),
            stream: Some(true),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""stream":true"#));
    }

    // -------- r4: 非流式响应反序列化 --------

    #[test]
    fn test_r4_chat_response_parse() {
        let json = r#"{"choices":[{"message":{"audio":{"data":"SGVsbG8="}}}]}"#;
        let resp: MimoChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        let audio = resp.choices[0].message.audio.as_ref().unwrap();
        assert_eq!(audio.data, "SGVsbG8=");
    }

    // -------- r5: 流式 chunk 解析 --------

    #[test]
    fn test_r5_stream_chunk_parse() {
        let json = r#"{"choices":[{"delta":{"audio":{"data":"SGVsbG8="}}}]}"#;
        let chunk: MimoStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        let data = chunk.choices[0]
            .delta
            .audio
            .as_ref()
            .and_then(|a| a.data.as_ref());
        assert_eq!(data, Some(&"SGVsbG8=".to_string()));
    }

    // -------- r6: SSE 行解析器（跨 chunk 拼接） --------

    #[test]
    fn test_r6_parser_cross_chunk() {
        let mut parser = SseLineParser::new();
        let lines = parser.push(b"data: {\"choices\"");
        assert!(lines.is_empty(), "不完整行不应产出");

        let lines = parser.push(b":[{\"delta\":{}}]}\n\n");
        assert_eq!(lines.len(), 2, "应产出 data 行 + 空行");
        assert!(lines[0].starts_with("data:"));
        assert!(lines[1].is_empty());
    }

    // -------- r7: SSE 行解析器 flush 尾行 --------

    #[test]
    fn test_r7_parser_flush_tail() {
        let mut parser = SseLineParser::new();
        parser.push(b"data: [DONE]");
        let tail = parser.flush();
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0], "data: [DONE]");
    }

    // -------- r8: [DONE] 与 finish_reason --------

    #[test]
    fn test_r8_parse_done_and_finish() {
        assert!(matches!(
            parse_data("[DONE]").unwrap(),
            Some(MimoStreamEvent::Finish)
        ));
        let finish = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        assert!(matches!(
            parse_data(finish).unwrap(),
            Some(MimoStreamEvent::Finish)
        ));
    }

    // -------- r9: error 帧 --------

    #[test]
    fn test_r9_parse_error() {
        let data = r#"{"error":{"code":"invalid_api_key","message":"Incorrect API key"}}"#;
        match parse_data(data).unwrap().unwrap() {
            MimoStreamEvent::Error(err) => {
                assert_eq!(err.message.as_deref(), Some("Incorrect API key"));
                assert_eq!(err.code.as_deref(), Some("invalid_api_key"));
            }
            _ => panic!("expected MimoStreamEvent::Error"),
        }
    }

    // -------- r10: base64 音频解码 --------

    #[test]
    fn test_r10_audio_decode() {
        // "SGVsbG8=" base64 → b"Hello"
        let data = r#"{"choices":[{"delta":{"audio":{"data":"SGVsbG8="}}}]}"#;
        match parse_data(data).unwrap().unwrap() {
            MimoStreamEvent::Audio(bytes) => assert_eq!(bytes, b"Hello"),
            _ => panic!("expected MimoStreamEvent::Audio"),
        }
    }

    // -------- r11: 空音频数据跳过 --------

    #[test]
    fn test_r11_empty_audio_skipped() {
        let data = r#"{"choices":[{"delta":{"audio":{"data":""}}}]}"#;
        assert!(parse_data(data).unwrap().is_none());
    }

    // -------- r12: extract_data --------

    #[test]
    fn test_r12_extract_data() {
        assert_eq!(extract_data("data: hello"), Some("hello"));
        assert_eq!(extract_data("data:hello"), Some("hello"));
        assert_eq!(extract_data(": comment"), None);
        assert_eq!(extract_data(""), None);
        assert_eq!(extract_data("event: x"), None);
    }

    // -------- r13: map_format --------

    #[test]
    fn test_r13_map_format() {
        assert_eq!(map_format("mp3"), "mp3");
        assert_eq!(map_format("wav"), "wav");
        assert_eq!(map_format("pcm"), "pcm");
        assert_eq!(map_format("opus"), "opus");
        assert_eq!(map_format("ogg"), "opus");
        assert_eq!(map_format("ogg_opus"), "opus");
        assert_eq!(map_format("flac"), "flac");
        assert_eq!(map_format("unknown"), "mp3");
    }

    // -------- r14: parse_error_body --------

    #[test]
    fn test_r14_parse_error_body() {
        let body = r#"{"error":{"code":"invalid_api_key","message":"Incorrect API key"}}"#;
        match parse_error_body(body, 401) {
            TtsError::ServiceError { code, message } => {
                assert_eq!(code, "invalid_api_key");
                assert!(message.contains("Incorrect"));
            }
            other => panic!("expected ServiceError, got {other:?}"),
        }
    }

    #[test]
    fn test_r14b_parse_error_body_non_json() {
        let err = parse_error_body("not json", 500);
        assert!(matches!(err, TtsError::Other(_)));
        assert!(err.to_string().contains("500"));
    }

    // -------- r15: mimo_voices --------

    #[test]
    fn test_r15_voices() {
        let voices = mimo_voices();
        assert_eq!(voices.len(), 7);
        assert!(voices.iter().any(|v| v.0 == "mimo_default"));
        assert!(voices.iter().any(|v| v.0 == "Mia"));
        assert!(voices.iter().any(|v| v.0 == "Chloe"));
        assert!(voices.iter().any(|v| v.0 == "Milo"));
        assert!(voices.iter().any(|v| v.0 == "Dean"));
    }

    // -------- r16: 空 choices --------

    #[test]
    fn test_r16_empty_choices() {
        let json = r#"{"choices":[]}"#;
        let chunk: MimoStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices.is_empty());
    }

    // -------- r17: 无 audio 的 delta --------

    #[test]
    fn test_r17_delta_no_audio() {
        let json = r#"{"choices":[{"delta":{"content":"hello"}}]}"#;
        let chunk: MimoStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices[0].delta.audio.is_none());
    }
}
