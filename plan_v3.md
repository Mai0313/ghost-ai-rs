# Ghost AI - Rust 遷移規格文件 v3

## 文件目的

本文件為將 Ghost AI 從 TypeScript/Electron 架構遷移至 Rust 的完整技術規格。此規格文件提供給其他 AI Agent 使用，用於在另一個 repository 中重新實現此專案。

---

## 專案概述

### 專案簡介

Ghost AI 是一個隱形的 AI 驅動桌面助手，提供以下核心功能：

- 文字輸入 + 螢幕截圖分析
- 即時語音轉錄與對話
- 完全隱形的操作界面（對螢幕截圖和螢幕分享不可見）
- 全域熱鍵控制
- 隱私優先設計（所有圖片在記憶體中處理，不落地）

### 技術架構（現有）

- **前端**：React + TypeScript
- **後端**：Electron Main Process
- **UI 框架**：Electron BrowserWindow (透明覆蓋層)
- **AI 服務**：直接整合 OpenAI API (Chat Completions, Whisper, Realtime API)
- **截圖**：screenshot-desktop
- **音訊處理**：Web Audio API
- **狀態管理**：electron-store
- **打包**：electron-builder

### 目標架構（Rust）

需要選擇適當的 Rust 生態系工具來替代上述功能。

---

## 核心功能詳細規格

### 1. 視窗管理系統

#### 1.1 主視窗屬性

- **完全透明覆蓋層視窗**
  - 無邊框 (frameless)
  - 透明背景
  - 全螢幕尺寸（覆蓋主要顯示器）
  - 始終置頂 (always on top)
  - 不顯示在工作列 (skip taskbar)
  - 無陰影
  - 不可調整大小
  - 不可全螢幕化
  - 初始狀態：隱藏

#### 1.2 內容保護

- 啟用視窗內容保護 (`setContentProtection(true)`)
- 防止截圖 API 捕獲此視窗
- 目的：實現「幽靈模式」

#### 1.3 滑鼠事件穿透

- 預設啟用滑鼠事件穿透 (`setIgnoreMouseEvents(true, { forward: true })`)
- 當滑鼠懸停在 UI 元件上時動態禁用穿透
- 實現方式：
  - 前端追蹤 `mousemove` 事件
  - 檢查 `elementFromPoint` 是否為 UI 元件
  - 通過 IPC 動態切換穿透狀態

#### 1.4 視窗顯示邏輯

- 應用啟動後預設隱藏
- 透過以下方式顯示：
  - 全域熱鍵觸發
  - 系統托盤選單
- 支援完全隱藏/顯示切換

### 2. 全域熱鍵系統

#### 2.1 固定熱鍵列表

所有熱鍵使用全域註冊（系統級），在任何應用程式中都能觸發：

| 熱鍵                   | 功能          | 說明                                       |
| ---------------------- | ------------- | ------------------------------------------ |
| `Ctrl/Cmd+Enter`       | 顯示 Ask 面板 | 顯示主視窗並打開問答面板，自動聚焦輸入框   |
| `Ctrl/Cmd+Shift+Enter` | 切換語音錄音  | 開始/停止語音錄音和即時轉錄                |
| `Ctrl/Cmd+\`           | 切換隱藏      | 隱藏/顯示整個 HUD 界面                     |
| `Ctrl/Cmd+R`           | 清除對話      | 清空對話歷史，生成新的 sessionId，停止錄音 |
| `Ctrl/Cmd+Up`          | 向上捲動      | 捲動 AI 回答內容向上                       |
| `Ctrl/Cmd+Down`        | 向下捲動      | 捲動 AI 回答內容向下                       |
| `Ctrl/Cmd+Shift+Up`    | 上一頁        | 切換到上一則助理回答                       |
| `Ctrl/Cmd+Shift+Down`  | 下一頁        | 切換到下一則助理回答或返回即時視圖         |

#### 2.2 熱鍵衝突處理

- 音訊切換熱鍵觸發時，抑制 400ms 內的 Ask 切換以避免重疊

#### 2.3 實現要求

- 使用低階鍵盤鉤子避免被監控軟體偵測
- 註冊失敗時通知使用者並提供替代方案
- 應用程式關閉前正確反註冊所有熱鍵

### 3. HUD (Heads-Up Display) 界面

#### 3.1 HUD 控制列

位置：頂部置中（距離頂部 20px）

包含按鈕：

1. **Listen** - 切換語音錄音
   - 錄音中顯示紅色並顯示計時器 (mm:ss)
   - 支援暫停/恢復
2. **Ask** - 切換問答面板
3. **Hide** - 隱藏 HUD
4. **Settings** - 切換設定面板

#### 3.2 樣式規格

- 深色玻璃質感
- 半透明深色背景 (rgba 格式)
- 圓角邊框
- 陰影效果
- 可拖曳（透過拖曳 HUD 控制列）

#### 3.3 面板系統

兩個主要面板位於 HUD 下方：

**Ask Panel (問答面板)**

- 單行輸入框
- Enter 送出（IME 組字時不送出）
- 支援附加截圖切換（📷 圖示）
- 顯示 AI 回答（Markdown 格式）
- 顯示推理過程（較小、半透明區域）
- 顯示網頁搜尋狀態指示器
- 分頁控制（顯示當前頁碼，如 "Live" 或 "2/5"）
- 重新生成按鈕（↻）
- 模型選擇下拉選單

**Settings Panel (設定面板)**

- OpenAI API 設定
  - API Key (加密儲存)
  - Base URL
  - 模型選擇（動態從 API 取得）
  - 測試連線按鈕
- 轉錄設定
  - 語言選擇 (en/zh)
- 截圖設定
  - 是否附加截圖切換
- Prompt 管理
  - 列出可用的 prompt 檔案
  - 選擇啟用的 prompt
  - 檔案來源：`~/.ghost-ai/prompts/`

### 4. 截圖系統

#### 4.1 截圖流程

1. 檢查使用者設定 (`attachScreenshot`)
2. 如果啟用：
   - 隱藏所有視窗 (`hideAllWindowsDuring`)
   - 執行全螢幕截圖
   - PNG 格式，完全在記憶體中處理
   - 轉換為 base64 用於 API 呼叫
3. 如果禁用：跳過截圖，僅傳送文字

#### 4.2 錯誤處理與重試

- 最多 3 次重試
- 每次重試間隔：200ms, 400ms, 800ms (指數退避)
- 失敗時拋出錯誤

#### 4.3 隱私要求

- 圖片**絕不**寫入磁碟
- 僅在記憶體中處理
- API 呼叫完成後立即清理緩衝區

### 5. AI 整合系統

#### 5.1 OpenAI Client 架構

**配置結構**

```typescript
interface OpenAIConfig {
  apiKey: string;
  baseURL: string;
  model: string;
  timeout: number;
  maxTokens?: number | null;
  temperature?: number;
}
```

**支援的模型列表**（白名單）

- chatgpt-4o-latest
- gpt-4o
- gpt-4.1
- o4-mini-2025-04-16
- gpt-5
- gpt-5-mini

#### 5.2 Streaming API (Responses API)

**請求流程**

1. 準備輸入：
   - System message (包含 custom prompt)
   - User message (文字 + 可選的圖片)
2. 配置參數：
   - 啟用串流
   - 啟用 web_search_preview 工具
   - 對 gpt-5 設定 reasoning effort: high
3. 處理串流事件：
   - `response.reasoning_summary_text.delta` - 推理增量
   - `response.reasoning_summary_text.done` - 推理完成
   - `response.output_text.delta` - 回答增量
   - `response.output_text.done` - 回答完成
   - `response.web_search_call.*` - 網頁搜尋狀態

**支援的頻道**

- `answer` - 主要回答內容
- `reasoning` - 推理過程（僅 gpt-5 等模型）
- `web_search` - 網頁搜尋狀態

#### 5.3 圖片處理

- 格式：PNG
- 編碼：base64
- 傳輸：`data:image/png;base64,{base64}`
- Detail 設定：auto

#### 5.4 配置管理

- 配置檔案位置：`~/.ghost-ai/config.json`
- API Key 使用 Electron safeStorage 加密儲存
- 支援動態更新配置（不需重啟）
- 支援驗證配置（測試按鈕）
- 支援列出可用模型

### 6. 即時轉錄系統

#### 6.1 WebSocket 連線

- Endpoint: `wss://api.openai.com/v1/realtime?intent=transcription`
- Headers:
  - `Authorization: Bearer {apiKey}`
  - `OpenAI-Beta: realtime=v1`

#### 6.2 Session 配置

```json
{
  "type": "transcription_session.update",
  "session": {
    "input_audio_format": "pcm16",
    "turn_detection": {
      "type": "server_vad",
      "threshold": 0.5,
      "silence_duration_ms": 350,
      "prefix_padding_ms": 150
    },
    "input_audio_transcription": {
      "model": "gpt-4o-realtime-preview-2025-06-03",
      "language": "en"  // or "zh"
    }
  }
}
```

#### 6.3 音訊處理管線

**捕獲階段**

1. 麥克風捕獲
   - `getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }})`
2. 系統音訊捕獲（可選）
   - `getDisplayMedia({ audio: true })`
   - 停止視訊軌道

**處理階段**

1. 建立 AudioContext
2. 混合音訊來源
3. 使用 ScriptProcessorNode 處理音訊 (bufferSize: 4096)
4. 降採樣至 24kHz mono
5. 轉換為 16-bit PCM
6. 批次處理：
   - 目標：每 220ms 或 32KB 刷新一次
   - 減少 WebSocket 開銷

**傳輸階段**

- 將 PCM16 轉為 base64
- 發送 `input_audio_buffer.append` 事件

**結束階段**

- 發送 `input_audio_buffer.end`
- 關閉 WebSocket
- 清理音訊資源

#### 6.4 轉錄事件處理

- `conversation.item.input_audio_transcription.delta` - 轉錄增量
- `conversation.item.input_audio_transcription.completed` - 轉錄完成

#### 6.5 暫停/恢復功能

- 暫停時：停止計時器和音訊處理，但不關閉連線
- 恢復時：繼續處理

### 7. Session 管理系統

#### 7.1 Session 生命週期

- 應用啟動時生成初始 sessionId (UUID)
- 觸發清除 (Ctrl/Cmd+R) 時生成新 sessionId
- 手動請求新 session 時生成新 sessionId

#### 7.2 Session Store 結構

```typescript
interface SessionEntry {
  index: number;
  requestId: string;
  text_input: string;
  ai_output: string;
}

interface SessionState {
  entries: SessionEntry[];
  nextIndex: number;
  logPath: string | null;
}
```

#### 7.3 對話歷史格式

純文字格式，每輪對話：

```
Q: <使用者問題>
A: <AI 回答>

```

#### 7.4 Initial Prompt 處理

- 只在 session 的第一輪注入
- 使用 system role
- 快取在 `initialPromptBySession` Map 中
- 重新生成時會包含在歷史中

#### 7.5 日誌系統

**對話日誌**

- 路徑：`~/.ghost-ai/logs/{sessionId}/{sessionId}.log`
- 格式：純文字，Q/A 格式
- 更新時機：每次 API 呼叫完成後

**Session JSON**

- 路徑：`~/.ghost-ai/logs/{sessionId}/{sessionId}.json`
- 格式：

```json
{
  "entries": [
    {
      "index": 0,
      "requestId": "uuid",
      "text_input": "user input",
      "ai_output": "ai response"
    }
  ],
  "nextIndex": 1,
  "log_path": "path/to/log"
}
```

### 8. 重新生成功能

#### 8.1 重新生成流程

1. 識別要重新生成的頁面（當前頁或最新頁）
2. 提取原始使用者訊息
3. 建構該頁之前的所有對話歷史
4. 以歷史作為 context override 呼叫 API
5. 更新該頁的 assistant 內容（不新增頁面）

#### 8.2 歷史重建

```typescript
function makePlainHistoryText(history: Array<{role, content}>): string {
  let out = "";
  for (let i = 0; i < history.length - 1; i += 2) {
    const user = history[i];
    const assistant = history[i + 1];
    if (user?.role === "user" && assistant?.role === "assistant") {
      out += `Q: ${user.content}\nA: ${assistant.content}\n\n`;
    }
  }
  return out;
}
```

### 9. 分頁導航系統

#### 9.1 分頁邏輯

- 只計算 assistant 角色的訊息
- `assistantAnswerIndices` 陣列儲存所有 assistant 訊息的索引
- `historyIndex` 狀態：
  - `null` - 顯示即時內容 ("Live")
  - `number` - 顯示歷史頁面 (如 "2/5")

#### 9.2 導航行為

- **上一頁**：
  - 在 Live 時 → 跳到最後一頁
  - 在其他頁 → 跳到前一頁
- **下一頁**：
  - 在最後一頁 → 返回 Live
  - 在其他頁 → 跳到下一頁
  - 在 Live 時 → 無動作

### 10. Prompt 管理系統

#### 10.1 檔案系統

- 目錄：`~/.ghost-ai/prompts/`
- 支援格式：`.txt`, `.md`, `.prompt`
- **唯讀**：應用程式不建立或修改 prompt 檔案
- 使用者需手動建立檔案

#### 10.2 選擇機制

- 活動 prompt 名稱儲存在使用者設定中 (`defaultPrompt`)
- 首次使用必須選擇 prompt，否則分析被阻止
- 無預設 fallback

#### 10.3 安全性

- 路徑正規化防止路徑穿越
- 檔案名稱清理：移除 `../` 等

### 11. 設定持久化

#### 11.1 electron-store 配置

- 路徑：`~/.ghost-ai/config.json`
- 結構：

```typescript
{
  encryptedOpenAI?: string;  // base64 加密的 OpenAI 配置
  baseURL?: string;          // 純文字（便於偵錯）
  model?: string;            // 純文字
  userSettings?: {
    transcribeLanguage?: "en" | "zh";
    attachScreenshot?: boolean;
    defaultPrompt?: string;
  }
}
```

#### 11.2 加密儲存

- 使用 Electron safeStorage
- 如果不可用則使用 base64（向下相容）

### 12. 串流中斷處理

#### 12.1 AbortController 機制

- 每個渲染器維護一個活動的 AbortController
- 新請求時中止前一個
- Ctrl/Cmd+R 清除時中止所有活動請求

#### 12.2 競態條件防護

- 每個請求快照 `requestSessionId`
- 只在 `requestSessionId === currentSessionId` 且未中止時寫入日誌
- 防止中斷的對話寫入錯誤的 session

### 13. Markdown 渲染

#### 13.1 渲染引擎

- 使用 BlockNote editor (read-only mode)
- 動態轉換 Markdown：`editor.tryParseMarkdownToBlocks(markdown)`
- 替換內容：`editor.replaceBlocks(editor.document, blocks)`

#### 13.2 樣式自訂

- 暗色主題捲軸
- WebKit 和 Firefox 樣式
- 定義在 `src/styles/blocknote-custom.css`

#### 13.3 程式碼區塊

- 無語法高亮
- 移除 Shiki 依賴以簡化

### 14. 系統托盤整合

#### 14.1 托盤選單

- 圖示：`ghost.ico`
- 選單項目：
  1. Show Overlay - 顯示主視窗
  2. Toggle Hide - 切換隱藏狀態
  3. Quit - 退出應用

#### 14.2 圖示路徑解析

生產環境：`process.resourcesPath/ghost.ico`
開發環境：`{projectRoot}/ghost.ico`

### 15. IPC 通訊架構

#### 15.1 主要 IPC 頻道

**OpenAI 相關**

- `openai:update-config` - 更新並持久化配置
- `openai:update-config-volatile` - 僅更新記憶體中的配置
- `openai:get-config` - 取得配置
- `openai:validate-config` - 驗證配置
- `openai:list-models` - 列出可用模型
- `openai:config-updated` (event) - 配置更新通知

**截圖分析**

- `capture:analyze-stream` (send) - 開始串流分析
- `capture:analyze-stream:start` (event) - 串流開始
- `capture:analyze-stream:delta` (event) - 增量更新
- `capture:analyze-stream:done` (event) - 串流完成
- `capture:analyze-stream:error` (event) - 錯誤

**轉錄相關**

- `transcribe:start` - 開始轉錄
- `transcribe:append` - 附加音訊資料
- `transcribe:end` - 結束輸入
- `transcribe:stop` - 停止轉錄
- `transcribe:start` (event) - 轉錄啟動
- `transcribe:delta` (event) - 轉錄增量
- `transcribe:done` (event) - 轉錄完成
- `transcribe:error` (event) - 錯誤
- `transcribe:closed` (event) - 連線關閉

**UI 控制**

- `text-input:show` - 顯示輸入框
- `text-input:toggle` - 切換輸入框
- `hud:show` - 顯示 HUD
- `hud:toggle-hide` - 切換隱藏
- `hud:set-mouse-ignore` - 設定滑鼠穿透
- `ask:clear` - 清除對話
- `ask:scroll` - 捲動內容
- `ask:paginate` - 分頁導航
- `audio:toggle` - 切換錄音

**Session 管理**

- `session:get` - 取得當前 sessionId
- `session:new` - 建立新 session
- `session:changed` (event) - session 變更通知
- `session:dump` - 匯出所有 session 資料

**設定管理**

- `settings:get` - 取得使用者設定
- `settings:update` - 更新使用者設定

**Prompt 管理**

- `prompts:list` - 列出 prompt 檔案
- `prompts:read` - 讀取 prompt 內容
- `prompts:set-default` - 設定預設 prompt
- `prompts:get-default` - 取得預設 prompt
- `prompts:get-active` - 取得活動 prompt
- `prompts:set-active` - 設定活動 prompt

**應用程式控制**

- `app:quit` - 退出應用

#### 15.2 事件負載格式

**Delta 事件**

```typescript
{
  requestId: string;
  sessionId: string;
  channel?: "answer" | "reasoning" | "web_search";
  eventType?: string;
  delta?: string;
  text?: string;
}
```

**分析結果**

```typescript
{
  requestId: string;
  content: string;
  model: string;
  timestamp: string;
  sessionId: string;
}
```

### 16. 錯誤處理策略

#### 16.1 API 錯誤

- 顯示友善錯誤訊息
- 在 UI 中內嵌顯示錯誤
- 保持輸入可用以便重試
- 不中斷應用程式運行

#### 16.2 截圖錯誤

- 指數退避重試（最多 3 次）
- 失敗時拋出錯誤並顯示給使用者
- 允許使用者選擇禁用截圖

#### 16.3 音訊錯誤

- 麥克風權限拒絕：顯示指引
- 系統音訊失敗：繼續使用麥克風
- WebSocket 錯誤：顯示並允許重試

#### 16.4 熱鍵註冊失敗

- 記錄失敗的熱鍵
- 通知使用者
- 不中斷應用啟動

### 17. 效能最佳化

#### 17.1 啟動最佳化

- 延遲載入非關鍵模組
- 背景初始化 OpenAI client
- 分階段啟動

#### 17.2 記憶體管理

- 及時釋放截圖 Buffer
- 清理音訊處理緩衝區
- 定期清理舊的 session 資料

#### 17.3 UI 效能

- 保持元件掛載（切換顯示而非重新載入）
- 使用 debounce 處理設定變更
- 批次處理串流增量

### 18. 隱私與安全

#### 18.1 資料不落地原則

- 截圖僅在記憶體中處理
- 音訊即時串流，不儲存
- API 回應不快取敏感內容

#### 18.2 加密儲存

- API Key 使用系統加密儲存
- 配置檔案儲存在使用者目錄

#### 18.3 網路安全

- 強制 HTTPS
- 驗證 API 憑證

#### 18.4 程序隱蔽

- 視窗內容保護
- 滑鼠穿透
- 無工作列顯示

### 19. 跨平台考量

#### 19.1 快速鍵差異

- macOS: `Cmd` 鍵
- Windows/Linux: `Ctrl` 鍵
- 使用 `CommandOrControl` 抽象

#### 19.2 路徑處理

- 使用者目錄：`os.homedir()`
- 配置目錄：`~/.ghost-ai/`
- 跨平台路徑分隔符處理

#### 19.3 系統整合

- macOS: DMG + ZIP
- Windows: NSIS + Portable
- Linux: AppImage + deb

---

## Rust 實作建議

### 推薦的 Rust Crates

#### GUI 框架

- **tauri** - 類似 Electron 的框架，輕量且高效
- **egui** - 即時模式 GUI，適合 overlay
- **iced** - 宣告式 UI 框架

#### 視窗管理

- **winit** - 跨平台視窗建立
- **tao** - Tauri 的視窗庫（winit fork）

#### 系統整合

- **global-hotkey** - 全域熱鍵
- **tray-icon** - 系統托盤
- **screenshots** - 跨平台截圖

#### 音訊處理

- **cpal** - 跨平台音訊 I/O
- **dasp** - 數位音訊訊號處理
- **hound** - WAV 編解碼

#### HTTP/WebSocket

- **reqwest** - HTTP 客戶端
- **tokio-tungstenite** - WebSocket 客戶端
- **async-openai** - OpenAI API 客戶端

#### 資料儲存

- **serde** + **serde_json** - 序列化
- **directories** - 跨平台目錄路徑
- **keyring** - 安全儲存憑證

#### 其他工具

- **tokio** - 非同步運行時
- **base64** - Base64 編解碼
- **uuid** - UUID 生成
- **tracing** - 結構化日誌

### 架構建議

#### 1. 使用 Tauri 作為主框架

- 類似 Electron 但更輕量
- Rust 後端 + Web 前端
- 內建 IPC 機制
- 跨平台打包支援

#### 2. 分離關注點

```
src/
├── main.rs              # 應用程式入口
├── app.rs               # 應用程式狀態
├── window/              # 視窗管理
│   ├── manager.rs
│   └── overlay.rs
├── hotkey/              # 熱鍵系統
│   └── manager.rs
├── capture/             # 截圖
│   └── screenshot.rs
├── audio/               # 音訊處理
│   ├── recorder.rs
│   └── processor.rs
├── ai/                  # AI 整合
│   ├── client.rs
│   ├── streaming.rs
│   └── transcription.rs
├── session/             # Session 管理
│   ├── store.rs
│   └── history.rs
├── config/              # 配置管理
│   ├── settings.rs
│   └── prompts.rs
└── ipc/                 # IPC 處理器
    ├── handlers.rs
    └── events.rs
```

#### 3. 狀態管理

使用 `Arc<Mutex<AppState>>` 或 `tokio::sync::RwLock` 管理共享狀態

#### 4. 非同步處理

所有 I/O 操作使用 async/await（tokio runtime）

#### 5. 錯誤處理

使用 `anyhow` 或 `thiserror` 建立清晰的錯誤類型

### 關鍵實作細節

#### 視窗透明與穿透

```rust
use tauri::window::WindowBuilder;

let window = WindowBuilder::new(app, "main", tauri::WindowUrl::App("index.html".into()))
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .fullscreen(false)
    .resizable(false)
    .build()?;

window.set_ignore_cursor_events(true)?;
```

#### 全域熱鍵

```rust
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};

let manager = GlobalHotKeyManager::new()?;
let hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::KeyEnter);
manager.register(hotkey)?;
```

#### 截圖捕獲

```rust
use screenshots::Screen;

let screens = Screen::all()?;
let screen = screens.first().unwrap();
let image = screen.capture()?;
let buffer = image.to_png()?;
```

#### OpenAI 串流

```rust
use async_openai::{Client, types::*};
use futures::StreamExt;

let client = Client::new();
let request = CreateChatCompletionRequestArgs::default()
    .model("gpt-4o")
    .messages(messages)
    .build()?;

let mut stream = client.chat().create_stream(request).await?;
while let Some(result) = stream.next().await {
    let response = result?;
    // 處理增量
}
```

#### WebSocket 轉錄

```rust
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures::{SinkExt, StreamExt};

let (ws_stream, _) = connect_async("wss://api.openai.com/v1/realtime").await?;
let (mut write, mut read) = ws_stream.split();

// 發送配置
write.send(Message::Text(config_json)).await?;

// 接收訊息
while let Some(msg) = read.next().await {
    let msg = msg?;
    // 處理轉錄事件
}
```

---

## 實作優先順序

### Phase 1: 核心基礎設施

1. 建立 Tauri 專案
2. 實作透明 overlay 視窗
3. 全域熱鍵系統
4. 基本 HUD UI
5. IPC 架構

### Phase 2: 截圖與文字分析

1. 截圖捕獲模組
2. OpenAI client 整合
3. 串流 API 支援
4. Ask Panel UI
5. Markdown 渲染

### Phase 3: 語音轉錄

1. 音訊捕獲（麥克風 + 系統）
2. 音訊處理管線
3. WebSocket 連線
4. 即時轉錄 UI
5. 暫停/恢復功能

### Phase 4: Session 與歷史

1. Session 管理
2. 對話歷史儲存
3. 日誌系統
4. 分頁導航
5. 重新生成功能

### Phase 5: 配置與設定

1. 設定持久化
2. Settings Panel UI
3. Prompt 管理
4. API 配置與驗證
5. 加密儲存

### Phase 6: 優化與打包

1. 效能優化
2. 記憶體管理
3. 錯誤處理增強
4. 跨平台測試
5. 打包設定

---

## 測試策略

### 單元測試

- 每個模組的核心邏輯
- 資料轉換函式
- 錯誤處理路徑

### 整合測試

- IPC 通訊
- API 整合
- 檔案系統操作

### 端到端測試

- 完整使用者流程
- 熱鍵觸發
- 多平台相容性

### 效能測試

- 啟動時間
- 記憶體使用
- CPU 使用率
- 截圖延遲

---

## 已知限制與注意事項

### 技術限制

1. OpenAI Realtime API 僅支援特定模型
2. 系統音訊捕獲在某些平台可能受限
3. 視窗內容保護可能無法完全阻止所有截圖工具

### 平台差異

1. macOS 需要額外的權限設定（麥克風、螢幕錄製）
2. Linux 的系統音訊捕獲支援因發行版而異
3. Windows 的全域熱鍵可能與某些應用衝突

### 效能考量

1. 即時音訊處理會佔用 CPU
2. 大圖片會增加 API 延遲
3. 長對話歷史會佔用記憶體

---

## 參考資源

### 現有專案檔案

- `src/main/main.ts` - 主程序入口
- `src/main/preload.ts` - IPC 橋接
- `src/shared/openai-client.ts` - OpenAI 客戶端
- `src/App.tsx` - 主 UI 元件
- `src/components/` - UI 元件
- `.github/copilot-instructions.md` - 完整技術文件

### API 文件

- OpenAI Chat Completions API
- OpenAI Realtime API
- OpenAI Whisper API

### Rust 生態

- Tauri 文件: https://tauri.app
- async-openai: https://github.com/64bit/async-openai
- global-hotkey: https://github.com/tauri-apps/global-hotkey

---

## 結語

本規格文件提供了將 Ghost AI 從 Electron/TypeScript 遷移至 Rust 的完整藍圖。所有功能都已詳細記錄，包括資料流、狀態管理、錯誤處理和跨平台考量。

實作時建議：

1. 使用 Tauri 作為主框架以降低複雜度
2. 採用模組化設計便於測試和維護
3. 充分利用 Rust 的類型系統和所有權確保安全性
4. 保持與原始功能的對等性，同時利用 Rust 的效能優勢
5. 詳細記錄任何架構決策的理由

如有任何不清楚之處，請參考原始專案的實作或本文件引用的具體檔案和行號。
