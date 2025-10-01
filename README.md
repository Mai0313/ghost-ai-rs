# Ghost AI

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![eframe](https://img.shields.io/badge/eframe-2C3E50)](https://github.com/emilk/egui/tree/master/crates/eframe)
[![Tokio](https://img.shields.io/badge/Tokio-0A7E8C)](https://tokio.rs/)
[![OpenAI](https://img.shields.io/badge/OpenAI-412991?logo=openai&logoColor=white)](https://openai.com/)
[![tests](https://github.com/Mai0313/rust_template/actions/workflows/test.yml/badge.svg)](https://github.com/Mai0313/rust_template/actions/workflows/test.yml)
[![code-quality](https://github.com/Mai0313/rust_template/actions/workflows/code-quality-check.yml/badge.svg)](https://github.com/Mai0313/rust_template/actions/workflows/code-quality-check.yml)
[![license](https://img.shields.io/badge/License-MIT-green.svg?labelColor=gray)](https://github.com/Mai0313/rust_template/tree/master?tab=License-1-ov-file)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/Mai0313/ghost-ai/pulls)

Ghost AI is a privacy-first desktop assistant implemented entirely in Rust. It captures screenshots and audio, sends them to OpenAI APIs, and renders responses in a minimalist native window that hides when you need to stay invisible during screen sharing.

## Highlights

- Stealth interface: hides windows, obfuscates titles, and keeps processing in memory only.
- Fast capture workflow: global hotkeys trigger screenshots, annotate prompts, and stream results instantly.
- Voice conversations: low-latency recording with transcription and conversational memory.
- Cross-platform Rust stack: single binary powered by `eframe`, `tokio`, and async OpenAI integrations.

## Architecture

- UI: [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) + egui for a native cross-platform interface.
- Async runtime: [`tokio`](https://tokio.rs/) drives background work, hotkeys, and API requests.
- Audio: [`cpal`](https://github.com/RustAudio/cpal) and [`hound`](https://github.com/ruuda/hound) manage capture and encoding.
- Screenshots: [`screenshots`](https://github.com/robmikh/screenshot-rs) ensures images stay in RAM.
- Configuration: human readable `config.json` persisted under the user configuration directory.

## Quick Start

### Prerequisites

- [Rust toolchain](https://www.rust-lang.org/tools/install) (1.75 or newer recommended).
- OpenAI API key with access to the models you intend to call.

### Build and Run

```bash
cargo run --release
```

The first run asks for your OpenAI credentials and preferred defaults. Settings live under the platform specific configuration directory (for example `%APPDATA%/ghost-ai` on Windows).

### Global Hotkeys

- Toggle command HUD: `Ctrl+Enter` (macOS uses `Cmd`).
- Start or stop recording: `Ctrl+Shift+Enter`.
- Toggle visibility: `Ctrl+\`.
- Reset session: `Ctrl+R`.

You can change bindings from the Settings panel inside the application.

## Development Workflow

- `cargo check` for quick validation during edits.
- `make fmt` to format and lint (runs `cargo fmt` and `cargo clippy`).
- `make test` or `cargo test --all` to exercise unit and integration suites.
- `make coverage` to generate coverage reports via `cargo-llvm-cov`.
- Use the `.devcontainer/` and `docker/` setups when you need a reproducible toolchain.

Static resources such as `ghost.ico` are bundled at build time. Release artifacts can be produced with `cargo build --release` and packaged with your platform-specific tooling if required.

## Configuration Files

All runtime configuration is stored in `config.json` under the standard OS configuration directory:

- Windows: `%APPDATA%/ghost-ai/config.json`
- macOS: `~/Library/Application Support/ghost-ai/config.json`
- Linux: `~/.config/ghost-ai/config.json`

Prompts and conversation history are persisted in the same directory. Delete the folder to reset the application.

## Contributing

Contributions are welcome:

- Report issues and share ideas.
- Improve documentation or localization.
- Submit pull requests that enhance performance, stability, or usability.

Please run the formatting and linting commands listed above before opening a PR.

## License

Ghost AI is distributed under the MIT License. See [LICENSE](LICENSE) for details.
