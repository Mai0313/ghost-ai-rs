# Ghost AI

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![eframe](https://img.shields.io/badge/eframe-2C3E50)](https://github.com/emilk/egui/tree/master/crates/eframe)
[![Tokio](https://img.shields.io/badge/Tokio-0A7E8C)](https://tokio.rs/)
[![OpenAI](https://img.shields.io/badge/OpenAI-412991?logo=openai&logoColor=white)](https://openai.com/)
[![tests](https://github.com/Mai0313/rust_template/actions/workflows/test.yml/badge.svg)](https://github.com/Mai0313/rust_template/actions/workflows/test.yml)
[![code-quality](https://github.com/Mai0313/rust_template/actions/workflows/code-quality-check.yml/badge.svg)](https://github.com/Mai0313/rust_template/actions/workflows/code-quality-check.yml)
[![license](https://img.shields.io/badge/License-MIT-green.svg?labelColor=gray)](https://github.com/Mai0313/rust_template/tree/master?tab=License-1-ov-file)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/Mai0313/ghost-ai/pulls)

Ghost AI 是一个完全由 Rust 编写的隐私优先桌面助手。它可以捕获屏幕与音频，直接调用 OpenAI API，并在极简的原生窗口中返回结果；当你进行屏幕共享时界面会自动隐藏。

## 亮点

- 隐身界面：隐藏窗口、混淆标题，并始终在内存中处理数据。
- 高效捕捉：通过全局快捷键完成截图、附加提示并实时获取回答。
- 语音对话：低延迟录音与转录，保留上下文的会话流程。
- 跨平台 Rust 栈：单一可执行文件，基于 `eframe`、`tokio` 与 OpenAI 异步集成。

## 架构

- 界面：使用 [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) 与 egui 实现跨平台原生 UI。
- 异步运行时：[`tokio`](https://tokio.rs/) 负责后台任务、快捷键以及 API 请求。
- 音频：[`cpal`](https://github.com/RustAudio/cpal) 与 [`hound`](https://github.com/ruuda/hound) 提供录音与编码能力。
- 截图：[`screenshots`](https://github.com/robmikh/screenshot-rs) 确保图像仅在内存中存在。
- 配置：人类可读的 `config.json` 保存在用户配置目录下。

## 快速开始

### 前置要求

- [Rust 工具链](https://www.rust-lang.org/tools/install)（推荐 1.75 及以上版本）。
- 拥有可访问所需模型的 OpenAI API Key。

### 构建与运行

```bash
cargo run --release
```

首次运行会引导你填写 OpenAI 凭据与默认设置。配置文件存放在操作系统的标准配置目录（例如 Windows 下的 `%APPDATA%/ghost-ai`）。

### 全局快捷键

- 打开命令 HUD：`Ctrl+Enter`（macOS 使用 `Cmd`）。
- 开始或停止录音：`Ctrl+Shift+Enter`。
- 切换可见性：`Ctrl+\`。
- 重置会话：`Ctrl+R`。

你可以在应用内的设置面板自定义这些按键。

## 开发流程

- 使用 `cargo check` 快速验证代码。
- 使用 `cargo fmt` 格式化源码。
- 使用 `cargo clippy -- -D warnings` 保持零警告。
- 使用 `cargo test` 运行单元测试（如果有）。

静态资源如 `ghost.ico` 会在构建阶段一起打包。如需发布正式版本，可执行 `cargo build --release` 并使用各平台工具封装安装包。

## 配置文件

运行时配置保存在标准操作系统目录的 `config.json` 中：

- Windows：`%APPDATA%/ghost-ai/config.json`
- macOS：`~/Library/Application Support/ghost-ai/config.json`
- Linux：`~/.config/ghost-ai/config.json`

提示模板与会话记录同样位于该目录。删除该文件夹即可重置应用。

## 参与贡献

欢迎社区贡献：

- 提交问题或建议改进。
- 完善文档或本地化。
- 通过 PR 优化性能、稳定性或可用性。

在提交 PR 前，请先运行上面的格式化与检查命令。

## 许可证

Ghost AI 以 MIT 许可证发布。详情见 [LICENSE](LICENSE)。
