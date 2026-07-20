//! MiMo (小米) TTS Provider
//!
//! 基于 MiMo TTS v2.5 HTTP REST API 实现语音合成。
//! 使用 OpenAI 兼容的 `chat/completions` + `audio` 参数协议。
//!
//! 与 [`super::qwen`] / [`super::doubao`]（基于 WebSocket）不同，MiMo 走纯
//! HTTP REST：非流式 `POST` 返回 JSON（base64 编码音频）；流式 `POST stream=true`
//! 返回 SSE（base64 音频块）。因此本 provider **不实现** `connect` / `TtsConnection`
//! （无长连接），`connect` 保留 trait 默认行为（返回 `Unsupported`）。
//!
//! 协议细节（请求体序列化、SSE 解析、参数映射、错误解析）见
//! [`crate::tts::protocol::mimo`]。

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_TYPE};

use crate::tts::error::TtsError;
use crate::tts::protocol::mimo::{
    self, MIMO_DEFAULT_BASE_URL, MIMO_DEFAULT_MODEL, MIMO_DEFAULT_VOICE, MimoAudioParam,
    MimoMessage, MimoSpeechRequest,
};
use crate::tts::traits::TtsProvider;
use crate::tts::types::{
    BaseTtsOption, TextStream, TtsAudioStream, TtsRequest, TtsResponse, TtsStreamChunk, TtsVoice,
};
use crate::tts::voice_id::VoiceId;

// ============================== 常量 ==============================

/// MiMo TTS 默认音频格式
const MIMO_DEFAULT_FORMAT: &str = "mp3";

/// HTTP 请求超时
#[cfg(not(test))]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(test)]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

// ============================== MimoTtsOption ==============================

/// MiMo TTS 专属配置
#[derive(Debug, Clone, Default)]
pub struct MimoTtsOption {
    pub base: BaseTtsOption,
    /// 风格/声音描述（可选）。设置后会在请求中添加 `user` message，
    /// 用于指导合成风格（Director Mode 功能）。
    pub style: Option<String>,
}

// ============================== MimoTts ==============================

/// MiMo TTS Provider
pub struct MimoTts {
    api_key: String,
    base_url: String,
    model: String,
    voice: VoiceId,
    format: String,
    style: Option<String>,
    client: reqwest::Client,
}

impl MimoTts {
    /// 将 `Option<String>` 中的空字符串视为 `None`
    fn non_empty(val: Option<String>) -> Option<String> {
        val.filter(|v| !v.is_empty())
    }

    pub fn new(options: MimoTtsOption) -> Self {
        let base = &options.base;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest client with timeout must build");
        Self {
            api_key: base.api_key.clone().unwrap_or_default(),
            base_url: Self::non_empty(base.base_url.clone())
                .unwrap_or_else(|| MIMO_DEFAULT_BASE_URL.into()),
            model: Self::non_empty(base.model.clone())
                .unwrap_or_else(|| MIMO_DEFAULT_MODEL.into()),
            voice: base
                .voice
                .clone()
                .unwrap_or_else(|| VoiceId::from(MIMO_DEFAULT_VOICE)),
            format: Self::non_empty(base.format.clone())
                .unwrap_or_else(|| MIMO_DEFAULT_FORMAT.into()),
            style: options.style.filter(|s| !s.is_empty()),
            client,
        }
    }

    /// 校验必要参数
    fn ensure_valid(&self) -> Result<(), TtsError> {
        if self.api_key.is_empty() {
            return Err(TtsError::InvalidParameter(
                "apiKey is required for MiMo TTS (set `api_key` via `BaseTtsOption`)".into(),
            ));
        }
        Ok(())
    }

    /// 构建请求体
    fn build_request(&self, text: &str, stream: bool) -> MimoSpeechRequest {
        let mut messages = Vec::new();

        // 有 style 时前置 user message
        if let Some(ref style) = self.style {
            if !style.is_empty() {
                messages.push(MimoMessage {
                    role: "user".into(),
                    content: style.clone(),
                });
            }
        }

        // assistant message 存放合成文本
        messages.push(MimoMessage {
            role: "assistant".into(),
            content: text.to_string(),
        });

        let api_format = mimo::map_format(&self.format);

        MimoSpeechRequest {
            model: self.model.clone(),
            messages,
            audio: Some(MimoAudioParam {
                voice: self.voice.to_string(),
                format: api_format.to_string(),
            }),
            stream: if stream { Some(true) } else { None },
        }
    }

    /// 获取请求选项（合并 `TtsRequest.options` 覆盖）
    fn resolve_options(&self, request: &TtsRequest) -> (String, VoiceId, String) {
        let opts = request.options.as_ref();
        let model = opts
            .and_then(|o| o.model.clone())
            .unwrap_or_else(|| self.model.clone());
        let voice = opts
            .and_then(|o| o.voice.clone())
            .unwrap_or_else(|| self.voice.clone());
        let format = opts
            .and_then(|o| o.format.clone())
            .unwrap_or_else(|| self.format.clone());
        (model, voice, format)
    }
}

// ============================== TtsProvider 实现 ==============================

#[async_trait]
#[allow(clippy::result_large_err)]
impl TtsProvider for MimoTts {
    fn name(&self) -> &'static str {
        "mimo"
    }

    async fn synthesize(&self, request: TtsRequest) -> Result<TtsResponse, TtsError> {
        self.ensure_valid()?;

        let (model, voice, format) = self.resolve_options(&request);
        let api_format = mimo::map_format(&format);

        let mut messages = Vec::new();

        // 有 style 时前置 user message
        if let Some(ref style) = self.style {
            if !style.is_empty() {
                messages.push(MimoMessage {
                    role: "user".into(),
                    content: style.clone(),
                });
            }
        }

        messages.push(MimoMessage {
            role: "assistant".into(),
            content: request.text.clone(),
        });

        let body = MimoSpeechRequest {
            model: model.clone(),
            messages,
            audio: Some(MimoAudioParam {
                voice: voice.to_string(),
                format: api_format.to_string(),
            }),
            stream: None,
        };

        let resp = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
            .header("api-key", &self.api_key)
            .header(CONTENT_TYPE, "application/json")
            .body(
                serde_json::to_vec(&body)
                    .map_err(|e| TtsError::Other(format!("serialize MiMo request: {e}")))?,
            )
            .send()
            .await
            .map_err(|e| TtsError::Other(format!("MiMo HTTP request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(mimo::parse_error_body(&text, status.as_u16()));
        }

        let response_body = resp
            .bytes()
            .await
            .map_err(|e| TtsError::Other(format!("MiMo read body failed: {e}")))?;

        let chat_resp: mimo::MimoChatResponse = serde_json::from_slice(&response_body)
            .map_err(|e| TtsError::Other(format!("parse MiMo chat response: {e}")))?;

        let audio_data = chat_resp
            .choices
            .first()
            .and_then(|c| c.message.audio.as_ref())
            .ok_or_else(|| {
                TtsError::Other(
                    "MiMo chat response contains no audio data (expected `choices[0].message.audio.data`)"
                        .into(),
                )
            })?;

        let audio = base64::engine::general_purpose::STANDARD
            .decode(&audio_data.data)
            .map_err(|e| TtsError::Other(format!("base64 decode audio data: {e}")))?;

        if audio.is_empty() {
            return Err(TtsError::NoAudio);
        }

        Ok(TtsResponse {
            audio,
            format: format.clone(),
            duration: None,
        })
    }

    async fn speak_stream(&self, input: TextStream) -> Result<TtsAudioStream, TtsError> {
        self.ensure_valid()?;

        // MiMo 流式 API 的 input 必须一次性发送：缓冲整个文本流。
        // 低延迟优势体现在音频输出侧（首帧 ~400ms），而非文本输入侧。
        let mut text = String::new();
        let mut input = input;
        while let Some(chunk) = input.next().await {
            text.push_str(&chunk);
        }

        let body = self.build_request(&text, true);

        let resp = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
            .header("api-key", &self.api_key)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .body(
                serde_json::to_vec(&body)
                    .map_err(|e| TtsError::Other(format!("serialize MiMo request: {e}")))?,
            )
            .send()
            .await
            .map_err(|e| TtsError::Other(format!("MiMo HTTP request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(mimo::parse_error_body(&text, status.as_u16()));
        }

        let bytes_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut parser = mimo::SseLineParser::new();
            tokio::pin!(bytes_stream);

            while let Some(chunk) = bytes_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(TtsError::Other(format!("MiMo stream read failed: {e}")));
                        return;
                    }
                };

                for line in parser.push(&chunk) {
                    if let Some(data) = mimo::extract_data(&line) {
                        match mimo::parse_data(data) {
                            Ok(Some(event)) => match event {
                                mimo::MimoStreamEvent::Audio(audio) => {
                                    yield Ok(TtsStreamChunk { audio_chunk: audio });
                                }
                                mimo::MimoStreamEvent::Finish => return,
                                mimo::MimoStreamEvent::Error(err) => {
                                    yield Err(TtsError::ServiceError {
                                        code: err.code.unwrap_or_default(),
                                        message: err.message.unwrap_or_else(|| "MiMo streaming error".into()),
                                    });
                                    return;
                                }
                            },
                            Ok(None) => {},
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }
                }
            }

            // 处理流末尾的残留尾行
            for line in parser.flush() {
                if let Some(data) = mimo::extract_data(&line) {
                    match mimo::parse_data(data) {
                        Ok(Some(event)) => match event {
                            mimo::MimoStreamEvent::Audio(audio) => {
                                yield Ok(TtsStreamChunk { audio_chunk: audio });
                            }
                            mimo::MimoStreamEvent::Finish => return,
                            mimo::MimoStreamEvent::Error(err) => {
                                yield Err(TtsError::ServiceError {
                                    code: err.code.unwrap_or_default(),
                                    message: err.message.unwrap_or_else(|| "MiMo streaming error".into()),
                                });
                                return;
                            }
                        },
                        Ok(None) => {},
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn list_voices(&self) -> Result<Vec<TtsVoice>, TtsError> {
        Ok(mimo_system_voices())
    }
}

// ============================== 内部工具 ==============================

/// 系统音色列表
fn mimo_system_voices() -> Vec<TtsVoice> {
    mimo::mimo_voices()
        .into_iter()
        .map(|(id, name, _language)| TtsVoice {
            id: id.into(),
            name: name.into(),
            language: "zh-CN".into(),
            gender: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::types::TtsConnectOption;

    // -------- c1 默认值 --------

    #[test]
    fn test_c1_defaults() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(p.name(), "mimo");
        assert_eq!(p.base_url, MIMO_DEFAULT_BASE_URL);
        assert_eq!(p.model, MIMO_DEFAULT_MODEL);
        assert_eq!(p.voice, MIMO_DEFAULT_VOICE);
        assert_eq!(p.format, "mp3");
        assert!(p.style.is_none());
    }

    // -------- c2 自定义覆盖 --------

    #[test]
    fn test_c2_custom_options() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("key".into()),
                base_url: Some("https://custom.example.com/v1".into()),
                model: Some("mimo-v2.5-tts-voicedesign".into()),
                voice: Some("Mia".into()),
                format: Some("wav".into()),
                ..Default::default()
            },
            style: Some("明亮自然的语调".into()),
        });
        assert_eq!(p.api_key, "key");
        assert_eq!(p.base_url, "https://custom.example.com/v1");
        assert_eq!(p.model, "mimo-v2.5-tts-voicedesign");
        assert_eq!(p.voice, "Mia");
        assert_eq!(p.format, "wav");
        assert_eq!(p.style, Some("明亮自然的语调".into()));
    }

    // -------- c3 api_key 来源 --------

    #[test]
    fn test_c3_api_key_source() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("the-key".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(p.api_key, "the-key");

        let p = MimoTts::new(MimoTtsOption::default());
        assert_eq!(p.api_key, "");
    }

    // -------- c4 model/voice 默认回退 --------

    #[test]
    fn test_c4_model_voice_defaults() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(p.model, MIMO_DEFAULT_MODEL);
        assert_eq!(p.voice, MIMO_DEFAULT_VOICE);
    }

    // -------- v1 空 api_key 校验 --------

    #[test]
    fn test_v1_empty_api_key() {
        let p = MimoTts::new(MimoTtsOption::default());
        assert!(matches!(
            p.ensure_valid(),
            Err(TtsError::InvalidParameter(_))
        ));
    }

    // -------- v2 合法 api_key --------

    #[test]
    fn test_v2_valid_api_key() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("valid".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(p.ensure_valid().is_ok());
    }

    // -------- m1 非流式请求体（无 style 时只发 assistant） --------

    #[test]
    fn test_m1_non_stream_request_no_style() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let req = p.build_request("你好", false);
        assert_eq!(req.stream, None);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "assistant");
        assert_eq!(req.messages[0].content, "你好");
        assert!(req.audio.is_some());
    }

    // -------- m2 非流式请求体（有 style 时发送 user + assistant） --------

    #[test]
    fn test_m2_non_stream_request_with_style() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            style: Some("自然语调".into()),
        });
        let req = p.build_request("hello", false);
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "自然语调");
        assert_eq!(req.messages[1].role, "assistant");
        assert_eq!(req.messages[1].content, "hello");
    }

    // -------- m3 流式请求（stream=true） --------

    #[test]
    fn test_m3_stream_request() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let req = p.build_request("hi", true);
        assert_eq!(req.stream, Some(true));
        assert_eq!(req.messages.len(), 1);
    }

    // -------- m4 format 映射 --------

    #[test]
    fn test_m4_format_mapping() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                format: Some("pcm".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let req = p.build_request("hi", false);
        let audio = req.audio.as_ref().unwrap();
        assert_eq!(audio.format, "pcm");

        // 未知格式回退 mp3
        let p2 = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                format: Some("aac".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let req2 = p2.build_request("hi", false);
        let audio2 = req2.audio.as_ref().unwrap();
        assert_eq!(audio2.format, "mp3");
    }

    // -------- m5 空 style 不发送 user message --------

    #[test]
    fn test_m5_empty_style_skipped() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            style: Some("".into()),
        });
        let req = p.build_request("hi", false);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "assistant");
    }

    // -------- l1 list_voices 返回 7 个系统音色 --------

    #[tokio::test]
    async fn test_l1_list_voices() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let voices = p.list_voices().await.unwrap();
        assert_eq!(voices.len(), 7);
        assert!(voices.iter().any(|v| v.id == "mimo_default"));
        assert!(voices.iter().any(|v| v.id == "Mia"));
        assert!(voices.iter().any(|v| v.id == "Chloe"));
    }

    // -------- s1 connect 返回 Unsupported（默认行为） --------

    #[tokio::test]
    async fn test_s1_connect_unsupported() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                ..Default::default()
            },
            ..Default::default()
        });
        let result = p.connect(TtsConnectOption::default()).await;
        assert!(matches!(result, Err(TtsError::Unsupported(_))));
    }

    // -------- w1 synthesize 空 api_key 校验 --------

    #[tokio::test]
    async fn test_w1_synthesize_no_api_key() {
        let p = MimoTts::new(MimoTtsOption::default());
        let req = TtsRequest {
            text: "hello".into(),
            options: None,
        };
        let result = p.synthesize(req).await;
        assert!(matches!(result, Err(TtsError::InvalidParameter(_))));
    }

    // -------- w2 speak_stream 空 api_key 校验 --------

    #[tokio::test]
    async fn test_w2_speak_stream_no_api_key() {
        let p = MimoTts::new(MimoTtsOption::default());
        let input: TextStream = Box::pin(futures_util::stream::iter(vec!["hello".to_string()]));
        let result = p.speak_stream(input).await;
        assert!(matches!(result, Err(TtsError::InvalidParameter(_))));
    }

    // -------- w3 resolve_options 合并 --------

    #[test]
    fn test_w3_resolve_options() {
        let p = MimoTts::new(MimoTtsOption {
            base: BaseTtsOption {
                api_key: Some("k".into()),
                model: Some("mimo-v2.5-tts".into()),
                voice: Some("mimo_default".into()),
                format: Some("mp3".into()),
                ..Default::default()
            },
            ..Default::default()
        });

        // 无覆盖 → 实例默认值
        let req = TtsRequest {
            text: "hello".into(),
            options: None,
        };
        let (model, voice, format) = p.resolve_options(&req);
        assert_eq!(model, "mimo-v2.5-tts");
        assert_eq!(voice, "mimo_default");
        assert_eq!(format, "mp3");

        // 有覆盖 → 使用覆盖值
        let req = TtsRequest {
            text: "hello".into(),
            options: Some(BaseTtsOption {
                model: Some("mimo-v2.5-tts-voicedesign".into()),
                voice: Some("Mia".into()),
                format: Some("wav".into()),
                ..Default::default()
            }),
        };
        let (model, voice, format) = p.resolve_options(&req);
        assert_eq!(model, "mimo-v2.5-tts-voicedesign");
        assert_eq!(voice, "Mia");
        assert_eq!(format, "wav");
    }
}
