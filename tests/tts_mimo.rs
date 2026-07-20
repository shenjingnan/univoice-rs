//! MiMo (小米) TTS Provider 集成测试
//!
//! 使用 [`MockMimoHttpServer`] 在本地 HTTP mock 上验证 `MimoTts` 的端到端行为，
//! 不依赖真实 API Key 与网络。

use futures_util::StreamExt;

use common::mock_mimo_http_server::{MimoMockCommand, MockMimoHttpServer};
use univoice::tts::provider::{MimoTts, MimoTtsOption};
use univoice::tts::{BaseTtsOption, TextStream, TtsError, TtsProvider, TtsRequest};

mod common;

/// 创建指向 mock server 的 MimoTts 实例
fn make_provider(http_url: &str) -> MimoTts {
    MimoTts::new(MimoTtsOption {
        base: BaseTtsOption {
            api_key: Some("test-key".into()),
            base_url: Some(http_url.to_string()),
            ..Default::default()
        },
        ..Default::default()
    })
}

// -------- i1 非流式 happy path --------

#[tokio::test]
async fn test_i1_synthesize_happy_path() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    // base64("HelloWorld") = "SGVsbG9Xb3JsZA=="
    let resp_body = r#"{"choices":[{"message":{"audio":{"data":"SGVsbG9Xb3JsZA=="}}}]}"#;
    server.send_command(MimoMockCommand::RespondJson {
        body: resp_body.into(),
    });

    let resp = tts
        .synthesize(TtsRequest {
            text: "你好".into(),
            options: None,
        })
        .await
        .expect("synthesize should succeed");

    assert_eq!(resp.audio, b"HelloWorld");
    assert_eq!(resp.format, "mp3");
}

// -------- i2 流式 happy path（多帧 base64 + stop） --------

#[tokio::test]
async fn test_i2_stream_happy_path() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    // "SGVsbG8=" → b"Hello"，"V29ybGQ=" → b"World"
    let frame1 = r#"{"choices":[{"delta":{"audio":{"data":"SGVsbG8="}}}]}"#;
    let frame2 = r#"{"choices":[{"delta":{"audio":{"data":"V29ybGQ="}}}]}"#;
    let frame3 = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
    server.send_command(MimoMockCommand::RespondSse {
        frames: vec![frame1.into(), frame2.into(), frame3.into()],
    });

    let input: TextStream = Box::pin(futures_util::stream::iter(vec!["你好".to_string()]));
    let mut stream = tts.speak_stream(input).await.expect("speak_stream ok");

    let mut chunks = Vec::new();
    while let Some(r) = stream.next().await {
        chunks.push(r.expect("chunk ok"));
    }
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].audio_chunk, b"Hello");
    assert_eq!(chunks[1].audio_chunk, b"World");
}

// -------- i3 HTTP 错误 → ServiceError --------

#[tokio::test]
async fn test_i3_http_error() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    server.send_command(MimoMockCommand::RespondError {
        status: 400,
        code: "invalid_api_key".into(),
        message: "Incorrect API key provided".into(),
    });

    let result = tts
        .synthesize(TtsRequest {
            text: "x".into(),
            options: None,
        })
        .await;
    match result {
        Err(TtsError::ServiceError { code, message }) => {
            assert_eq!(code, "invalid_api_key");
            assert!(message.contains("Incorrect"));
        }
        other => panic!("expected ServiceError, got {other:?}"),
    }
}

// -------- i4 SSE 中途 error 帧 --------

#[tokio::test]
async fn test_i4_sse_error_frame() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    let err_frame = r#"{"error":{"code":"rate_limit","message":"Too many requests"}}"#;
    server.send_command(MimoMockCommand::RespondSse {
        frames: vec![err_frame.into()],
    });

    let input: TextStream = Box::pin(futures_util::stream::iter(vec!["x".to_string()]));
    let mut stream = tts.speak_stream(input).await.expect("speak_stream ok");
    let first = stream.next().await.expect("should receive an event");
    match first {
        Err(TtsError::ServiceError { code, .. }) => assert_eq!(code, "rate_limit"),
        other => panic!("expected ServiceError, got {other:?}"),
    }
}

// -------- i5 空音频 → NoAudio --------

#[tokio::test]
async fn test_i5_empty_audio() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    // 空 base64
    let resp_body = r#"{"choices":[{"message":{"audio":{"data":""}}}]}"#;
    server.send_command(MimoMockCommand::RespondJson {
        body: resp_body.into(),
    });

    let result = tts
        .synthesize(TtsRequest {
            text: "x".into(),
            options: None,
        })
        .await;
    assert!(matches!(result, Err(TtsError::NoAudio)));
}

// -------- i6 跨 chunk 的 SSE 帧（行拼接） --------

#[tokio::test]
async fn test_i6_cross_chunk_sse() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    server.send_command(MimoMockCommand::RespondSseSplit {
        chunks: vec![
            "data: {\"choices\":[".into(),
            "{\"delta\":{\"audio\":{\"data\":\"AA==\"}}}]}\n\n".into(),
            "data: {\"choices\":[".into(),
            "{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n".into(),
        ],
    });

    let input: TextStream = Box::pin(futures_util::stream::iter(vec!["x".to_string()]));
    let mut stream = tts.speak_stream(input).await.expect("speak_stream ok");

    let mut chunks = Vec::new();
    while let Some(r) = stream.next().await {
        match r {
            Ok(c) => chunks.push(c),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
    // 仅第一帧含音频；stop 帧不产出
    assert_eq!(chunks.len(), 1);
    // base64 "AA==" → 单字节 0x00
    assert_eq!(chunks[0].audio_chunk, b"\x00");
}

// -------- i7 [DONE] 哨兵结束 --------

#[tokio::test]
async fn test_i7_done_sentinel() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);
    let tts = make_provider(&url);

    // MiMo 流式支持 [DONE] 哨兵结束
    let frame1 = r#"{"choices":[{"delta":{"audio":{"data":"SGVsbG8="}}}]}"#;
    server.send_command(MimoMockCommand::RespondSse {
        frames: vec![frame1.into(), "[DONE]".into()],
    });

    let input: TextStream = Box::pin(futures_util::stream::iter(vec!["hi".to_string()]));
    let mut stream = tts.speak_stream(input).await.expect("speak_stream ok");

    let mut count = 0;
    while let Some(r) = stream.next().await {
        assert!(r.is_ok(), "should not error on [DONE]");
        count += 1;
    }
    assert_eq!(count, 1, "should get audio before [DONE]");
}

// -------- i8 非流式带 style --------

#[tokio::test]
async fn test_i8_synthesize_with_style() {
    let server = MockMimoHttpServer::start().await;
    let url = format!("http://{}", server.addr);

    // 使用带 style 的 provider
    let tts = MimoTts::new(MimoTtsOption {
        base: BaseTtsOption {
            api_key: Some("test-key".into()),
            base_url: Some(url.to_string()),
            ..Default::default()
        },
        style: Some("明亮自然的语调".into()),
    });

    let resp_body = r#"{"choices":[{"message":{"audio":{"data":"SGVsbG8="}}}]}"#;
    server.send_command(MimoMockCommand::RespondJson {
        body: resp_body.into(),
    });

    let resp = tts
        .synthesize(TtsRequest {
            text: "你好".into(),
            options: None,
        })
        .await
        .expect("synthesize with style should succeed");

    assert_eq!(resp.audio, b"Hello");
}

// ============================================================================
// 真实 API 测试（需要 MIMO_API_KEY 环境变量）
// ============================================================================

/// 使用真实 MiMo API 进行非流式合成（需要网络和 MIMO_API_KEY）
///
/// 通过 `MIMO_API_KEY`、`MIMO_BASE_URL`、`MIMO_TTS_MODEL` 环境变量配置。
/// 使用 `cargo test --test tts_mimo real_api -- --nocapture` 运行。
#[tokio::test]
async fn test_real_api_synthesize() {
    let api_key = match std::env::var("MIMO_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("⚠️  跳过真实 API 测试：MIMO_API_KEY 未设置");
            return;
        }
    };

    let base_url =
        std::env::var("MIMO_BASE_URL").unwrap_or_else(|_| "https://api.xiaomimimo.com/v1".into());
    let model = std::env::var("MIMO_TTS_MODEL").unwrap_or_else(|_| "mimo-v2.5-tts".into());

    let tts = MimoTts::new(MimoTtsOption {
        base: BaseTtsOption {
            api_key: Some(api_key),
            base_url: Some(base_url),
            model: Some(model.clone()),
            voice: Some("mimo_default".into()),
            ..Default::default()
        },
        style: Some("请用自然流畅的语调朗读".into()),
    });

    eprintln!("🔄 调用 MiMo TTS API... model={}", model);

    let resp = tts
        .synthesize(TtsRequest {
            text: "你好，欢迎使用小米 MiMo 语音合成服务。今天天气真不错。".into(),
            options: None,
        })
        .await
        .expect("真实 API 调用应该成功");

    eprintln!(
        "✅ 合成成功！音频: {} bytes, 格式: {}",
        resp.audio.len(),
        resp.format
    );

    assert!(!resp.audio.is_empty(), "音频数据不应为空");
    assert_eq!(resp.format, "mp3");

    let out_path = "/tmp/mimo_tts_output.mp3";
    std::fs::write(out_path, &resp.audio).expect("写入音频文件成功");
    eprintln!("📁 音频已保存到: {out_path}");
}
