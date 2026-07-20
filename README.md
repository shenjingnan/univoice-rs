# univoice-rs

统一的 TTS（文字转语音）和 ASR（语音识别）Rust SDK。

## 特性

- **CLI 骨架** — 基于 clap 的命令行参数解析，支持子命令和 Shell 补全生成
- **异步运行时** — 集成 tokio，开箱即用的 async/await 支持
- **配置管理** — TOML 格式的配置文件读写，支持 `${env.VAR}` 环境变量引用
- **双层日志** — 基于 tracing 的日志系统，同时输出到文件和 stderr
- **日期时间工具** — 基于 chrono 的常用时间格式转换函数
- **测试支持** — 集成 tempfile 的测试隔离辅助工具
- **代码质量** — cargo fmt / clippy / typos / tarpaulin / codecov 一站式配置
- **CI/CD** — GitHub Actions 自动化测试、发布、覆盖率报告
- **Shell 补全** — 支持 bash / zsh / fish / powershell 自动补全

## 快速开始

```bash
# 运行
cargo run
cargo run -- config
cargo run -- greet --name World

# 测试
cargo test

# 代码质量检查
cargo fmt --check
cargo clippy -- -D warnings
```

## 项目结构

```
├── Cargo.toml           # 项目配置和依赖
├── rust-toolchain.toml  # Rust 工具链版本（1.85）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具
│   ├── cli.rs           # CLI 命令定义
│   ├── config/
│   │   ├── mod.rs       # 配置模块入口
│   │   └── settings.rs  # TOML 配置管理
│   ├── logging.rs       # tracing 双层日志
│   └── datetime.rs      # 日期时间工具
├── tests/               # 集成测试
├── .github/workflows/   # CI/CD
└── .githooks/           # Git hooks
```

## 开发

```bash
# 运行 CLI
cargo run -- config
cargo run -- greet --name World

# 运行测试
cargo test

# 代码检查
cargo fmt --check
cargo clippy -- -D warnings
```

## 依赖说明

| 分类 | Crate | 用途 |
|------|-------|------|
| 核心 | clap | CLI 参数解析 |
| 核心 | tokio | 异步运行时 |
| 核心 | serde / serde_json / toml | 序列化 |
| 核心 | chrono | 日期时间处理 |
| 核心 | tracing / tracing-subscriber | 日志 |
| 核心 | thiserror / anyhow | 错误处理 |
| 可选 | reqwest | HTTP 客户端（按需引入） |

## 许可

[MIT](LICENSE)

## 性能测试

<!-- PERFORMANCE_TABLE_START -->
# Univoice 性能基准测试报告

> 生成时间: 2026-07-20 17:17:06

> ⚠️ 注意：所有测试数据来自真实 API 调用，实际表现受网络环境、服务端负载等因素影响。

## TTS 性能指标

### 指标说明

| 指标 | 含义 |
|---|---|
| 首包延迟 | 从发起请求到收到第一个音频包的时间 (ms)，反映服务响应速度 |
| P50 | 中位数延迟 (ms)，反映典型体验 |
| P95 | 95 分位延迟 (ms)，反映最差情况 |
| 标准差 | 延迟的离散程度，越小越稳定 |
| 吞吐量 | 每秒合成的字符数 (chars/s) |

### 性能数据

| 服务商 | 场景 | 测试次数 | 成功率 | 首包延迟(ms) | P50(ms) | P95(ms) | 标准差(ms) | 吞吐量(chars/s) |
|---|---|---|---|---|---|---|---|---|
| doubao | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 89.4 | 45.5 |
| doubao | synthesize | 7 | 57% | 2219 | 2219 | 2310 | 82.6 | 128.5 |
| gemini | synthesize | 7 | 29% | 22637 | 22637 | 24686 | 2897.8 | 12.6 |
| glm | synthesize | 7 | 57% | 5365 | 5365 | 5782 | 372.4 | 53.1 |
| minimax | synthesize | 7 | 57% | 2105 | 2105 | 2441 | 256.0 | 135.4 |
| openai | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 89.4 | 45.5 |
| qwen | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 89.4 | 45.5 |
| qwen | synthesize | 9 | 100% | 4801 | 4801 | 6446 | 1680.1 | 59.4 |
| qwen-realtime | synthesize | 6 | 100% | 4272 | 4272 | 4618 | 253.5 | 66.7 |
| xfyun | synthesize | 3 | 0% | 0 | 0 | 0 | 0.0 | N/A |

## ASR 性能指标

### 指标说明

| 指标 | 含义 |
|---|---|
| 首包延迟 | 从开始识别到收到第一个文字块的时间 (ms) |
| RTF | 实时率 (Real-Time Factor)，< 1 表示快于实时 |
| CER | 字符错误率 (Character Error Rate)，越低越好 |
| 准确率 | 识别准确率 (1 - CER) |

### 性能数据

| 服务商 | 场景 | 测试次数 | 成功率 | 首包延迟(ms) | P50(ms) | P95(ms) | RTF | CER | 准确率 |
|---|---|---|---|---|---|---|---|---|---|
| doubao | listen_stream | 8 | 100% | 443 | 5045 | 5815 | 0.34 | N/A | N/A |
| doubao | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 1.10 | N/A | N/A |
| glm | listen_stream | 2 | 0% | 0 | 0 | 0 | N/A | N/A | N/A |
| mimo | listen_stream | 3 | 0% | 0 | 0 | 0 | N/A | N/A | N/A |
| openai | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 1.10 | N/A | N/A |
| qwen | listen_stream | 8 | 100% | 1604 | 1767 | 2187 | 0.12 | 1.0000 | 0.00 |
| qwen | mock-scenario | 6 | 100% | 2200 | 2200 | 2300 | 1.10 | N/A | N/A |

---
*报告由 Univoice Benchmark v0.1.2 自动生成*

<!-- PERFORMANCE_TABLE_END -->
