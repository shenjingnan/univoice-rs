/// MiMo (小米) TTS v2.5 - 流式语音合成示例
///
/// 通过 SSE 接收 base64 编码的音频流并保存。
///
/// ## 使用方法
///
/// ```bash
/// # 使用 .env 中的 API Key
/// cargo run --example tts_mimo_stream -- --output output_mimo_stream.mp3
///
/// # 自定义文本分段（流式）
/// cargo run --example tts_mimo_stream -- \
///   --text "第一段内容。" --text "第二段内容。" \
///   --output speech.mp3
/// ```
use std::path::PathBuf;

use clap::Parser;
use futures_util::StreamExt;

use univoice::tts::provider::{MimoTts, MimoTtsOption};
use univoice::tts::{BaseTtsOption, TextStream, TtsProvider};

#[derive(Parser)]
#[command(name = "tts-mimo-stream", about = "MiMo (小米) TTS v2.5 流式合成示例")]
struct Args {
    /// MiMo API Key（也支持 MIMO_API_KEY 环境变量）
    #[arg(long, env = "MIMO_API_KEY")]
    api_key: String,

    /// MiMo API Base URL（也支持 MIMO_BASE_URL 环境变量）
    #[arg(long, env = "MIMO_BASE_URL")]
    base_url: Option<String>,

    /// 输出音频文件路径
    #[arg(short, long, default_value = "output_mimo_stream.mp3")]
    output: PathBuf,

    /// 文本分段（可指定多个）
    #[arg(
        short,
        long,
        default_value = "你好，欢迎使用小米 MiMo 流式语音合成。",
        default_value = "今天天气真不错，适合出门散步。"
    )]
    text: Vec<String>,

    /// 模型名称（也支持 MIMO_TTS_MODEL 环境变量，默认 mimo-v2.5-tts）
    #[arg(long, env = "MIMO_TTS_MODEL")]
    model: Option<String>,

    /// 音色名称：mimo_default / default_zh / default_en / Mia / Chloe / Milo / Dean
    #[arg(long, default_value = "mimo_default")]
    voice: String,

    /// 音频格式: mp3 / opus / flac / wav / pcm（默认 mp3）
    #[arg(long, default_value = "mp3")]
    format: String,

    /// 风格/声音描述（Director Mode），用于指导合成风格（可选）
    #[arg(long)]
    style: Option<String>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    if args.api_key.is_empty() {
        eprintln!("错误: 请提供 --api-key 或设置 MIMO_API_KEY 环境变量");
        std::process::exit(1);
    }

    println!("\n=== MiMo (小米) TTS v2.5 - 流式合成（SSE）===");
    println!("文本分段数: {}", args.text.len());
    for (i, t) in args.text.iter().enumerate() {
        println!("  分段 {}: \"{}\" ({} 字)", i + 1, t, t.chars().count());
    }
    println!("输出: {}", args.output.display());
    if let Some(ref s) = args.style {
        println!("风格: {}", s);
    }

    // 创建 MiMo TTS 实例
    let tts = MimoTts::new(MimoTtsOption {
        base: BaseTtsOption {
            api_key: Some(args.api_key),
            base_url: args.base_url,
            model: args.model,
            voice: Some(args.voice.into()),
            format: Some(args.format),
            ..Default::default()
        },
        style: args.style,
    });

    // 构建文本流
    let text_stream: TextStream = Box::pin(futures_util::stream::iter(args.text.clone()));

    // 执行流式合成
    let start = std::time::Instant::now();
    let mut audio_chunks: Vec<Vec<u8>> = Vec::new();
    let mut chunk_count = 0;
    let mut first_frame_latency: Option<u128> = None;

    println!("\n开始流式合成...\n");

    match tts.speak_stream(text_stream).await {
        Ok(mut stream) => {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        if first_frame_latency.is_none() {
                            first_frame_latency = Some(start.elapsed().as_millis());
                            println!("[首帧] 延迟 {} ms", first_frame_latency.unwrap());
                        }
                        chunk_count += 1;
                        audio_chunks.push(chunk.audio_chunk);

                        if chunk_count % 10 == 0 || chunk_count == 1 {
                            println!("[接收中] 已收到 {} 个音频块", chunk_count);
                        }
                    }
                    Err(e) => {
                        eprintln!("接收音频错误: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("启动流式合成失败: {}", e);
            std::process::exit(1);
        }
    }

    if audio_chunks.is_empty() {
        eprintln!("错误: 未收到任何音频数据");
        std::process::exit(1);
    }

    // 合并所有音频块
    let total_len: usize = audio_chunks.iter().map(|c| c.len()).sum();
    let mut audio = Vec::with_capacity(total_len);
    for chunk in audio_chunks {
        audio.extend_from_slice(&chunk);
    }

    let elapsed = start.elapsed();

    println!("\n=== 统计信息 ===");
    println!("音频块数: {}", chunk_count);
    println!(
        "音频大小: {} bytes ({:.1} KB)",
        audio.len(),
        audio.len() as f64 / 1024.0
    );
    println!("合成耗时: {} ms", elapsed.as_millis());

    // 保存到文件
    match std::fs::write(&args.output, &audio) {
        Ok(_) => println!("\n📁 文件已保存: {}", args.output.display()),
        Err(e) => {
            eprintln!("错误: 写入文件失败: {}", e);
            std::process::exit(1);
        }
    }
}
