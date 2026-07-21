# Changelog

## [0.1.4](https://github.com/shenjingnan/univoice-rs/compare/v0.1.3...v0.1.4) - 2026-07-21

### Added

- *(tts)* 补充 MiniMax TTS 缺失的 API 字段支持 ([#28](https://github.com/shenjingnan/univoice-rs/pull/28))
- *(tts)* 添加 MiMo TTS v2.5 提供商支持 ([#26](https://github.com/shenjingnan/univoice-rs/pull/26))

### Other

- *(tts)* 重命名 TTS Provider：qwen → cosyvoice, qwen-realtime → qwen3-tts ([#30](https://github.com/shenjingnan/univoice-rs/pull/30))

## [0.1.3](https://github.com/shenjingnan/univoice-rs/compare/v0.1.2...v0.1.3) - 2026-07-20

### Added

- *(benchmark)* 添加 --analyze 报告生成命令并修复 Doubao 环境变量 ([#25](https://github.com/shenjingnan/univoice-rs/pull/25))

### Other

- *(deps)* bump serde_json from 1.0.150 to 1.0.151 ([#22](https://github.com/shenjingnan/univoice-rs/pull/22))
- *(deps)* bump toml from 0.8.23 to 1.1.3+spec-1.1.0 ([#23](https://github.com/shenjingnan/univoice-rs/pull/23))

## [0.1.2](https://github.com/shenjingnan/univoice-rs/compare/v0.1.1...v0.1.2) - 2026-07-19

### Other

- *(build)* 从 crates.io 发布包中排除 benchmark 源码 ([#19](https://github.com/shenjingnan/univoice-rs/pull/19))

## [0.1.1](https://github.com/shenjingnan/univoice-rs/compare/v0.1.0...v0.1.1) - 2026-07-19

### Fixed

- 修复 Doubao ASR VAD 无法提前判停的问题 ([#16](https://github.com/shenjingnan/univoice-rs/pull/16))

### Other

- *(deps)* bump sha2 from 0.10.9 to 0.11.0 ([#5](https://github.com/shenjingnan/univoice-rs/pull/5))
- *(deps)* bump tokio-tungstenite from 0.24.0 to 0.30.0 ([#13](https://github.com/shenjingnan/univoice-rs/pull/13))
- *(deps)* bump anyhow from 1.0.102 to 1.0.103 ([#6](https://github.com/shenjingnan/univoice-rs/pull/6))
- *(deps)* bump reqwest from 0.12.28 to 0.13.4 ([#7](https://github.com/shenjingnan/univoice-rs/pull/7))
- *(deps)* bump clap from 4.6.1 to 4.6.2 ([#8](https://github.com/shenjingnan/univoice-rs/pull/8))
- *(deps)* bump bytes from 1.11.1 to 1.12.1 ([#9](https://github.com/shenjingnan/univoice-rs/pull/9))
- *(deps)* bump uuid from 1.23.3 to 1.24.0 ([#11](https://github.com/shenjingnan/univoice-rs/pull/11))
- *(deps)* bump tungstenite from 0.24.0 to 0.30.0 ([#12](https://github.com/shenjingnan/univoice-rs/pull/12))
- *(deps)* bump tokio from 1.52.3 to 1.53.0 ([#14](https://github.com/shenjingnan/univoice-rs/pull/14))
- 移除项目中的所有 TypeScript 残留代码 ([#15](https://github.com/shenjingnan/univoice-rs/pull/15))
- 初始化

## [0.1.0] - 2026-06-05

### Added

- 项目初始化
- CLI 骨架（clap + tokio）
- 配置管理（TOML 配置读写）
- 双层日志系统（tracing）
- 日期时间工具模块
- CI/CD 配置（GitHub Actions）
- 代码质量工具（fmt, clippy, typos, tarpaulin, codecov）
- Shell 补全生成
