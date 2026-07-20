/// MiMo (小米) TTS v2.5 - 非流式语音合成示例
///
/// 将文本合成为完整音频并保存到文件。
///
/// ## 使用方法
///
/// ```bash
/// # 使用 .env 中的 API Key
/// cargo run --example tts_mimo_synthesize -- \
///   --text "你好，欢迎使用小米 MiMo 语音合成" \
///   --output output_mimo.mp3
///
/// # 指定音色、风格指令
/// cargo run --example tts_mimo_synthesize -- \
///   --voice Mia --style "请用活泼可爱的语调朗读" \
///   --text "今天天气真不错" --output hello.mp3
///
/// # 显式指定 API Key 和 Base URL
/// cargo run --example tts_mimo_synthesize -- \
///   --api-key xxxxx --base-url https://api.xiaomimimo.com/v1 \
///   --text "你好" --output hello.mp3
/// ```
use std::path::PathBuf;

use clap::Parser;

use univoice::tts::provider::{MimoTts, MimoTtsOption};
use univoice::tts::{BaseTtsOption, TtsProvider, TtsRequest};

#[derive(Parser)]
#[command(
    name = "tts-mimo-synthesize",
    about = "MiMo (小米) TTS v2.5 非流式合成示例"
)]
struct Args {
    /// MiMo API Key（也支持 MIMO_API_KEY 环境变量）
    #[arg(long, env = "MIMO_API_KEY")]
    api_key: String,

    /// MiMo API Base URL（也支持 MIMO_BASE_URL 环境变量，默认 https://api.xiaomimimo.com/v1）
    #[arg(long, env = "MIMO_BASE_URL")]
    base_url: Option<String>,

    /// 待合成的文本
    #[arg(
        short,
        long,
        default_value = "你好，欢迎使用小米 MiMo 语音合成服务，今天天气真不错。"
    )]
    text: String,

    /// 输出音频文件路径
    #[arg(short, long, default_value = "output_mimo.mp3")]
    output: PathBuf,

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

    println!("\n=== MiMo (小米) TTS v2.5 - 非流式合成 ===");
    println!("文本: {}", args.text);
    println!("输出: {}", args.output.display());
    if let Some(ref s) = args.style {
        println!("风格: {}", s);
    }

    // 创建 MiMo TTS 实例
    // 可用音色常量：voice_id::mimo::MIA.into(), voice_id::mimo::CHLOE.into() 等
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

    // 执行合成
    let start = std::time::Instant::now();

    match tts
        .synthesize(TtsRequest {
            text: args.text,
            options: None,
        })
        .await
    {
        Ok(response) => {
            let elapsed = start.elapsed();

            println!("\n✅ 合成成功!");
            println!(
                "音频大小: {} bytes ({:.1} KB)",
                response.audio.len(),
                response.audio.len() as f64 / 1024.0
            );
            println!("音频格式: {}", response.format);
            println!("合成耗时: {} ms", elapsed.as_millis());

            // 保存到文件
            match std::fs::write(&args.output, &response.audio) {
                Ok(_) => println!("\n📁 文件已保存: {}", args.output.display()),
                Err(e) => {
                    eprintln!("错误: 写入文件失败: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ 合成失败: {}", e);
            std::process::exit(1);
        }
    }
}
