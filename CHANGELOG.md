# Changelog

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
