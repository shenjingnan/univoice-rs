/// Doubao ASR - 实时音频流式识别 + VAD 判停示例
///
/// 模拟硬件实时发送音频场景：
/// 1. 从 PCM 文件读取音频数据
/// 2. 分块实时发送到 Doubao ASR（Streaming 模式）
/// 3. 通过 VAD（end_window_size + force_to_speech_time）检测用户是否说完
/// 4. 一旦收到 definite=true 立即返回识别结果，不等音频发完
///
/// ## 与 asr_doubao_vad.rs 的区别
///
/// asr_doubao_vad.rs 使用 Async 模式 + enable_nonstream，判停后二次识别有额外延迟。
/// 本示例使用 Streaming 模式，首遍识别到 definite=true 立即返回，适合"说完立刻出答案"的场景。
///
/// ## 使用方法
///
/// ```bash
/// cargo run --example asr_doubao_realtime -- \
///   --api-key YOUR_API_KEY \
///   --file speech.pcm
/// ```
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use futures_util::StreamExt;

use univoice::asr::{
    AsrProvider, AudioInput, BaseProviderOption, DEFAULT_CHUNK_SIZE, DoubaoAsr, DoubaoAsrMode,
    DoubaoAsrOption, adapt_audio_input,
};

#[derive(Parser)]
#[command(
    name = "asr-doubao-realtime",
    about = "Doubao ASR 实时音频流式识别 + VAD 判停"
)]
struct Args {
    /// 火山引擎新版控制台 API Key（也支持 DOUBAO_API_KEY 环境变量）
    #[arg(long, env = "DOUBAO_API_KEY")]
    api_key: String,

    /// PCM 音频文件路径
    #[arg(short, long)]
    file: PathBuf,

    /// 采样率（默认 16000）
    #[arg(long, default_value_t = 16000)]
    sample_rate: u32,

    /// 位深度（默认 16）
    #[arg(long, default_value_t = 16)]
    bits: u8,

    /// 声道数（默认 1）
    #[arg(long, default_value_t = 1)]
    channel: u8,

    /// VAD 尾静音判停阈值（毫秒）
    #[arg(long, default_value_t = 800)]
    end_window_size: u32,

    /// VAD 生效最小音频时长（毫秒）
    #[arg(long, default_value_t = 1000)]
    force_to_speech_time: u32,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    // 验证参数
    if args.api_key.is_empty() {
        eprintln!("错误: 请提供 --api-key 或设置 DOUBAO_API_KEY 环境变量");
        std::process::exit(1);
    }

    if !args.file.exists() {
        eprintln!("错误: 音频文件不存在: {}", args.file.display());
        std::process::exit(1);
    }

    // 读取 PCM 文件
    let audio_data = match std::fs::read(&args.file) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("错误: 读取文件失败: {}", e);
            std::process::exit(1);
        }
    };

    // 估算音频时长（PCM: 采样率 × 位深/8 × 声道）
    let bytes_per_sec = args.sample_rate as u64 * (args.bits as u64 / 8) * args.channel as u64;
    let duration_ms = (audio_data.len() as u64)
        .checked_mul(1000)
        .and_then(|n| n.checked_div(bytes_per_sec.max(1)))
        .unwrap_or(0);

    println!();
    println!("=== Doubao ASR - 实时音频流式识别 + VAD 判停 ===");
    println!("场景: PCM 文件模拟硬件实时发送音频");
    println!();
    println!("配置:");
    println!("  音频文件: {}", args.file.display());
    println!(
        "  采样率: {} Hz, 位深: {} bit, 声道: {}",
        args.sample_rate, args.bits, args.channel
    );
    println!(
        "  音频大小: {} 字节 (~{} ms)",
        audio_data.len(),
        duration_ms
    );
    println!("  VAD end_window_size: {} ms", args.end_window_size);
    println!(
        "  VAD force_to_speech_time: {} ms",
        args.force_to_speech_time
    );
    println!("  模式: Streaming（首遍识别 + definite 判停，不二次识别）");

    // 创建 Doubao ASR 实例（Streaming 模式 + VAD 参数）
    let asr = DoubaoAsr::new(DoubaoAsrOption {
        base: BaseProviderOption {
            language: Some("zh-CN".into()),
            ..Default::default()
        },
        api_key: Some(args.api_key),
        mode: DoubaoAsrMode::Streaming, // 首遍识别，判停即返回
        sample_rate: args.sample_rate,
        bits: args.bits,
        channel: args.channel,
        end_window_size: Some(args.end_window_size),
        force_to_speech_time: Some(args.force_to_speech_time),
        // 注意: enable_nonstream 不开启，避免二次识别延迟
        ..Default::default()
    });

    // 将音频数据切分为流（模拟硬件实时发送）
    let audio_stream = adapt_audio_input(AudioInput::Data(audio_data), DEFAULT_CHUNK_SIZE);

    // 执行流式识别
    let start = Instant::now();
    let mut first_result_time: Option<u64> = None;
    let mut chunk_count = 0;
    let mut intermediate_count = 0;
    let mut final_text: Option<String> = None;
    let mut definite_time: Option<u64> = None;

    println!();
    println!("开始流式识别...");
    println!("{}", "-".repeat(50));
    println!();

    match asr.listen_stream(audio_stream).await {
        Ok(mut stream) => {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => {
                        chunk_count += 1;
                        let now = start.elapsed().as_millis() as u64;

                        if first_result_time.is_none() {
                            first_result_time = Some(now);
                            println!("[首字延迟] {} ms", now);
                            println!();
                        }

                        // 判断是否 VAD 判停（definite=true）
                        let is_definite = chunk
                            .segment
                            .as_ref()
                            .and_then(|s| s.confidence)
                            .map(|c| c >= 0.99)
                            .unwrap_or(false);

                        if is_definite {
                            // ===== VAD 判停：用户说完了 =====
                            definite_time = Some(now);
                            final_text = Some(chunk.text.clone());

                            let status = if chunk.is_final {
                                "最终"
                            } else {
                                "VAD 判停"
                            };
                            println!("[{}] ★★★ definite=true 用户已说完 ★★★", status);
                            println!("[{}] 文本: \"{}\"", status, chunk.text);
                            if let Some(ref seg) = chunk.segment {
                                println!(
                                    "[{}] 语音段: [{}ms - {}ms], 置信度: {:.0}%",
                                    status,
                                    seg.start,
                                    seg.end,
                                    seg.confidence.unwrap_or(0.0) * 100.0
                                );
                            }
                            println!();

                            // 判停后立即 return（这就是"说完立刻返回答案"）
                            break;
                        } else if chunk.is_final && !chunk.text.is_empty() {
                            // ===== 音频全部发完后的最终结果 =====
                            final_text = Some(chunk.text.clone());
                            definite_time = Some(now);

                            println!("[最终] 音频发送完毕，收到最终结果:");
                            println!("[最终] 文本: \"{}\"", chunk.text);
                            println!();

                            break;
                        } else if !chunk.text.is_empty() {
                            // ===== 中间结果 =====
                            intermediate_count += 1;
                            println!("[中间] #{:<3} \"{}\"", intermediate_count, chunk.text);
                        }
                    }
                    Err(e) => {
                        eprintln!("[错误] 识别出错: {}", e);
                        break;
                    }
                }
            }

            // 流结束但没触发判停的情况
            if final_text.is_none() {
                println!("[提示] 音频流结束，未触发 VAD 判停（音频可能过短）");
            }
        }
        Err(e) => {
            eprintln!("错误: 启动识别失败: {}", e);
            std::process::exit(1);
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;

    // 输出统计
    println!("{}", "=".repeat(50));
    println!("统计信息");
    println!("{}", "-".repeat(50));
    println!("  总耗时:        {} ms", elapsed);

    #[allow(clippy::option_if_let_else)]
    let ttlm = match definite_time {
        Some(t) => t,
        None => elapsed,
    };
    println!("  首字延迟:      {} ms", first_result_time.unwrap_or(0));
    println!("  判停时刻:      {} ms（自识别开始）", ttlm);
    println!("  总接收块数:    {}", chunk_count);
    println!("  中间结果数:    {}", intermediate_count);

    if let Some(ref text) = final_text {
        println!();
        println!("{}", "=".repeat(50));
        println!("最終识别结果: \"{}\"", text);
        println!("{}", "=".repeat(50));
    } else {
        println!();
        println!("最终识别结果: (无)");
    }
    println!();
}
