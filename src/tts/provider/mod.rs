pub mod cosyvoice;
pub mod doubao;
pub mod gemini;
pub mod glm;
pub mod mimo;
pub mod minimax;
pub mod openai;
pub mod qwen3_tts;
pub mod xfyun;

pub use cosyvoice::{CosyvoiceTts, CosyvoiceTtsConnection, CosyvoiceTtsOption};
pub use doubao::{DoubaoTts, DoubaoTtsConnection, DoubaoTtsOption};
pub use gemini::{GeminiTts, GeminiTtsOption};
pub use glm::{GlmTts, GlmTtsOption};
pub use mimo::{MimoTts, MimoTtsOption};
pub use minimax::{MinimaxTts, MinimaxTtsOption};
pub use openai::{OpenaiTts, OpenaiTtsOption};
pub use qwen3_tts::{Qwen3Tts, Qwen3TtsConnection, Qwen3TtsOption};
pub use xfyun::{XfyunTts, XfyunTtsOption};
