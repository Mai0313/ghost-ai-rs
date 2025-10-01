# Ghost AI

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![eframe](https://img.shields.io/badge/eframe-2C3E50)](https://github.com/emilk/egui/tree/master/crates/eframe)
[![Tokio](https://img.shields.io/badge/Tokio-0A7E8C)](https://tokio.rs/)
[![OpenAI](https://img.shields.io/badge/OpenAI-412991?logo=openai&logoColor=white)](https://openai.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg?labelColor=gray)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/Mai0313/ghost-ai/pulls)

Ghost AI 是一款完全以 Rust 撰寫的隱私優先桌面助理。它會擷取螢幕與音訊，直接呼叫 OpenAI API，並在極簡的原生視窗中呈現結果；進行螢幕分享時介面會自動隱藏。

## 亮點
- 隱身介面：隱藏視窗、混淆標題，所有資料僅於記憶體處理。
- 高效率擷取：透過全域快速鍵完成截圖、附加提示並即時取得回覆。
- 語音對話：低延遲錄音與轉錄，維持對話脈絡。
- 跨平台 Rust 技術堆疊：以 `eframe`、`tokio` 與 OpenAI 非同步整合打造單一可執行檔。

## 架構
- 介面：使用 [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) 與 egui 建構跨平台原生 UI。
- 非同步執行緒：[`tokio`](https://tokio.rs/) 負責背景工作、快速鍵與 API 請求。
- 音訊：[`cpal`](https://github.com/RustAudio/cpal) 與 [`hound`](https://github.com/ruuda/hound) 處理錄音與編碼。
- 螢幕截圖：[`screenshots`](https://github.com/robmikh/screenshot-rs) 確保影像只存在於記憶體。
- 設定：可讀的 `config.json` 儲存在使用者設定目錄。

## 快速開始

### 先決條件
- [Rust 工具鏈](https://www.rust-lang.org/tools/install)（建議 1.75 以上版本）。
- 具備目標模型存取權限的 OpenAI API Key。

### 建置與執行
```bash
cargo run --release
```
首次啟動會引導設定 OpenAI 憑證與預設選項。設定檔位於作業系統標準路徑（例如 Windows 的 `%APPDATA%/ghost-ai`）。

### 全域快速鍵
- 叫出指令 HUD：`Ctrl+Enter`（macOS 改用 `Cmd`）。
- 開始或停止錄音：`Ctrl+Shift+Enter`。
- 切換可見度：`Ctrl+\`。
- 重設對話：`Ctrl+R`。

可於應用程式的設定面板調整上述按鍵。

## 開發流程
- 使用 `cargo check` 快速驗證修改。
- 使用 `cargo fmt` 格式化程式碼。
- 使用 `cargo clippy -- -D warnings` 維持零警告。
- 使用 `cargo test` 執行單元測試（若有）。

靜態資源如 `ghost.ico` 會於建置時一併打包。需要釋出時，可先執行 `cargo build --release`，再搭配各平台常用工具建立發行檔。

## 設定檔案
執行時設定儲存在作業系統預設目錄的 `config.json`：
- Windows：`%APPDATA%/ghost-ai/config.json`
- macOS：`~/Library/Application Support/ghost-ai/config.json`
- Linux：`~/.config/ghost-ai/config.json`

提示範本與對話紀錄也同步保存在此目錄。刪除整個資料夾即可重置應用程式。

## 參與貢獻
歡迎貢獻：
- 回報問題或提出功能建議。
- 改善文件與在地化內容。
- 透過 PR 強化效能、穩定度或使用體驗。

提交 PR 前請先執行上述格式化與檢查指令。

## 授權條款
Ghost AI 採用 MIT 授權，詳情請參閱 [LICENSE](LICENSE)。
