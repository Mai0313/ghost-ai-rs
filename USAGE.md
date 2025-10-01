# Ghost AI 使用指南

## 功能概覽

Ghost AI 是一個用 Rust 實現的桌面 AI 助理，具有以下核心功能：

### ✅ 已實現功能

1. **OpenAI API 整合**
   - 支援 Chat Completions API
   - 串流式回應（可選）
   - 支援多輪對話上下文
   - 自訂系統提示詞

2. **截圖分析**
   - 全螢幕截圖
   - 截圖前自動隱藏視窗
   - 截圖附加到對話
   - 支援 Vision API 分析圖片

3. **語音轉錄**
   - 麥克風音訊錄製
   - 使用 OpenAI Whisper API 轉錄
   - 支援中英文語言選擇
   - 轉錄結果自動填入輸入框

4. **浮動透明視窗**
   - 始終置頂
   - 半透明背景
   - 可拖曳調整大小

5. **全域快捷鍵**
   - `Ctrl+Enter`: 開啟/關閉 Ask 面板
   - `Ctrl+Shift+Enter`: 開始/停止錄音
   - `Ctrl+\`: 隱藏/顯示視窗
   - `Ctrl+R`: 清除對話
   - `Ctrl+Shift+S`: 截圖並附加

6. **對話管理**
   - 保存對話歷史到日誌文件
   - 清除對話功能
   - 純文字格式導出

7. **配置管理**
   - JSON 格式配置文件
   - 可視化設定介面
   - API Key 驗證功能

### ❌ 未實現功能（按 plan.md 忽略）

- 即時串流轉錄（OpenAI Realtime API）
- 系統音訊擷取
- 點擊穿透（egui 限制）
- 對話翻頁功能
- Web 搜尋指示器
- 推理過程顯示（GPT-5 特性）
- Markdown 高級渲染（目前僅純文字）

## 首次使用

### 1. 啟動應用

```bash
# 開發模式
cargo run

# 發布模式（推薦）
cargo run --release
```

### 2. 配置 OpenAI API

首次啟動時，點擊 **Settings** 按鈕：

1. 填入 **API Key**（必填）
2. 設定 **Base URL**（預設: https://api.openai.com/v1）
3. 選擇 **Model**（預設: gpt-4o-mini）
4. 調整 **Temperature**（預設: 0.7）
5. 點擊 **Validate API Key** 測試連線
6. 點擊 **Save Settings** 保存配置

### 3. 建立提示詞（可選）

在配置目錄下建立 `prompts` 資料夾：

- Windows: `%APPDATA%\ghost\ghost-ai\prompts\`
- macOS: `~/Library/Application Support/ghost/ghost-ai/prompts/`
- Linux: `~/.config/ghost/ghost-ai/prompts/`

放入 `.txt`、`.md` 或 `.prompt` 格式的提示詞文件，例如：

**coding-assistant.txt**
```
You are a helpful coding assistant. Always provide clear, concise code examples with explanations.
```

然後在 Settings 中選擇該提示詞。

## 基本使用流程

### 文字對話

1. 點擊 **Show Ask Panel** 或按 `Ctrl+Enter`
2. 在輸入框中輸入問題
3. 按 **Send** 或 `Ctrl+Enter`（輸入框內）提交
4. 等待 AI 回應顯示在對話區域

### 截圖+問題

1. 點擊 **Attach Screenshot** 按鈕
2. 應用會自動隱藏 200ms 後截圖
3. 預覽圖片會顯示在輸入框下方
4. 輸入問題（或留空僅分析圖片）
5. 點擊 **Send** 提交

### 語音輸入

1. 點擊 **Start Recording** 或按 `Ctrl+Shift+Enter`
2. 對著麥克風說話
3. 再次點擊 **Stop Recording** 停止錄音
4. 等待轉錄完成，文字會自動填入輸入框
5. 檢查並編輯文字，然後提交

### 清除對話

點擊 **Clear Session** 或按 `Ctrl+R` 清除所有對話歷史。

## 高級配置

### config.json 結構

```json
{
  "openai": {
    "api_key": "sk-...",
    "base_url": "https://api.openai.com/v1",
    "model": "gpt-4o-mini",
    "temperature": 0.7,
    "max_output_tokens": 2048
  },
  "capture": {
    "attach_screenshots": true,
    "hide_before_capture": true,
    "mode": "active_monitor"
  },
  "transcription": {
    "enabled": true,
    "realtime": false,
    "language": "en",
    "model": "whisper-1"
  },
  "hotkeys": {
    "toggle_ask_panel": "Ctrl+Enter",
    "toggle_record": "Ctrl+Shift+Enter",
    "toggle_hide": "Ctrl+\\",
    "clear_session": "Ctrl+R",
    "capture_screenshot": "Ctrl+Shift+S"
  },
  "prompts": {
    "active_prompt_name": "coding-assistant.txt"
  },
  "ui": {
    "opacity": 0.92,
    "compact_mode": false,
    "theme": "dark"
  }
}
```

### 日誌位置

對話日誌儲存在：

- Windows: `%APPDATA%\ghost\ghost-ai\logs\`
- macOS: `~/Library/Application Support/ghost/ghost-ai/logs/`
- Linux: `~/.local/share/ghost/ghost-ai/logs/`

每個會話一個資料夾，包含：
- `{session_id}-conversation.txt`: 純文字對話紀錄

## 疑難排解

### API 請求失敗

1. 檢查 API Key 是否正確
2. 驗證網路連線
3. 確認 Base URL 無誤
4. 查看 model 是否支援（Vision 需要 gpt-4o 或更新版本）

### 截圖失敗

1. 確認有螢幕存在
2. 檢查系統權限（macOS 需要授權螢幕錄製）
3. 嘗試切換 capture mode 到 "primary"

### 錄音失敗

1. 檢查麥克風權限
2. 確認預設輸入裝置正常
3. 嘗試在系統設定中重新選擇麥克風

### 快捷鍵無效

1. 檢查是否有其他應用佔用相同快捷鍵
2. 重新啟動應用
3. 在 config.json 中修改快捷鍵綁定

## 開發與貢獻

查看 [README.md](README.md) 了解開發環境設置和貢獻指南。

## 授權

MIT License - 詳見 [LICENSE](LICENSE)
