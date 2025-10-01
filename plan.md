# Ghost AI - Rust 遷移規格文件

> **目標:** 將 Ghost AI (Electron + TypeScript + React) 專案完整改寫為 Rust 原生桌面應用程式
>
> **文件版本:** 1.0.0
> **生成日期:** 2025-10-01
> **原專案:** ghost-ai (Electron-based)

---

## 📋 目錄

01. [專案概覽](#%E5%B0%88%E6%A1%88%E6%A6%82%E8%A6%BD)
02. [核心功能規格](#%E6%A0%B8%E5%BF%83%E5%8A%9F%E8%83%BD%E8%A6%8F%E6%A0%BC)
03. [技術架構設計](#%E6%8A%80%E8%A1%93%E6%9E%B6%E6%A7%8B%E8%A8%AD%E8%A8%88)
04. [模組詳細規格](#%E6%A8%A1%E7%B5%84%E8%A9%B3%E7%B4%B0%E8%A6%8F%E6%A0%BC)
05. [資料流與狀態管理](#%E8%B3%87%E6%96%99%E6%B5%81%E8%88%87%E7%8B%80%E6%85%8B%E7%AE%A1%E7%90%86)
06. [API 與介面定義](#api-%E8%88%87%E4%BB%8B%E9%9D%A2%E5%AE%9A%E7%BE%A9)
07. [UI/UX 規格](#uiux-%E8%A6%8F%E6%A0%BC)
08. [安全與隱私要求](#%E5%AE%89%E5%85%A8%E8%88%87%E9%9A%B1%E7%A7%81%E8%A6%81%E6%B1%82)
09. [效能與最佳化](#%E6%95%88%E8%83%BD%E8%88%87%E6%9C%80%E4%BD%B3%E5%8C%96)
10. [測試策略](#%E6%B8%AC%E8%A9%A6%E7%AD%96%E7%95%A5)
11. [部署與打包](#%E9%83%A8%E7%BD%B2%E8%88%87%E6%89%93%E5%8C%85)
12. [遷移路線圖](#%E9%81%B7%E7%A7%BB%E8%B7%AF%E7%B7%9A%E5%9C%96)

---

## 專案概覽

### 應用簡介

Ghost AI 是一個**隱形 AI 桌面助理**,提供以下核心能力:

- **螢幕截圖分析** - 自動擷取螢幕並透過 AI 進行視覺問答
- **即時語音轉錄** - 支援麥克風和系統音訊的即時轉錄
- **浮動疊加層 UI** - 透明、可拖曳、點擊穿透的控制介面
- **多輪對話管理** - 保持上下文的連續對話
- **隱身模式** - 對截圖和螢幕分享軟體完全隱形

### 技術棧對照

| 功能模組          | 原技術棧 (Electron)     | 目標技術棧 (Rust)                   |
| ----------------- | ----------------------- | ----------------------------------- |
| **桌面框架**      | Electron                | Tauri / iced / egui                 |
| **UI 框架**       | React + TypeScript      | 待選擇 (見下方建議)                 |
| **狀態管理**      | React Hooks             | Arc\<Mutex\<T>> / tokio::sync       |
| **IPC 通訊**      | Electron IPC            | Tauri Commands / 自訂 async channel |
| **HTTP 客戶端**   | fetch / openai SDK      | reqwest + async/await               |
| **WebSocket**     | ws (Node.js)            | tokio-tungstenite                   |
| **音訊處理**      | Web Audio API           | cpal + hound / dasp                 |
| **截圖**          | screenshot-desktop      | screenshots / xcap                  |
| **熱鍵**          | Electron globalShortcut | global-hotkey                       |
| **檔案存儲**      | electron-store          | serde_json + fs / rusqlite          |
| **加密**          | Electron safeStorage    | ring / rustls / keyring             |
| **Markdown 渲染** | BlockNote               | pulldown-cmark + 自訂渲染           |

### UI 框架建議

推薦以下三種方案之一:

#### 方案 A: **Tauri + Web 前端** (推薦用於快速遷移)

- **前端:** React/Vue/Svelte (保留原 UI 邏輯)
- **後端:** Rust (主進程邏輯)
- **優點:** 可重用大部分 React 組件,遷移成本低
- **缺點:** 仍依賴 Web 技術

#### 方案 B: **iced** (推薦用於純 Rust)

- **特性:** 類似 Elm 的響應式 UI,跨平台原生
- **優點:** 純 Rust,效能優異,易於部署
- **缺點:** Markdown 渲染需自行實現

#### 方案 C: **egui** (推薦用於即時模式 UI)

- **特性:** 即時模式 GUI,輕量快速
- **優點:** 非常適合疊加層 UI,低延遲
- **缺點:** 複雜佈局較困難

**建議選擇:**

- 若要快速遷移 → **Tauri + React**
- 若要純 Rust 體驗 → **iced**
- 若要極致效能 → **egui**

---

## 核心功能規格

### 1. 浮動 HUD 控制列

#### 功能需求

- [x] 透明視窗,可自訂不透明度 (預設 59.5%)
- [x] 可拖曳定位,記憶位置
- [x] 點擊穿透 (hover 時關閉穿透)
- [x] 始終置頂 (always-on-top)
- [x] 無邊框、無陰影
- [x] 不顯示在工作列

#### UI 元素

```
+-------------------------------------------------------+
| [Listen] [Ask] [Hide] [Settings]  [時間顯示] [拖曳區] |
+-------------------------------------------------------+
```

**按鈕功能:**

- `Listen` - 開始/停止語音錄製
- `Ask` - 開啟文字輸入面板
- `Hide` - 隱藏整個應用
- `Settings` - 開啟設定面板

**錄音狀態:**

- 未錄製: 顯示 "Listen"
- 錄製中: 顯示 "Pause" (紅色) + 時間計時器
- 暫停: 顯示 "Resume"

#### 技術實現要點

**Rust 實現建議:**

```rust
// 使用 tao (Tauri 的視窗庫) 或 winit
use tao::window::{WindowBuilder, Window};
use tao::dpi::LogicalPosition;

let window = WindowBuilder::new()
    .with_transparent(true)
    .with_decorations(false)
    .with_always_on_top(true)
    .with_skip_taskbar(true)
    .with_resizable(false)
    .build(&event_loop)?;

// 點擊穿透 (平台特定)
#[cfg(target_os = "windows")]
{
    use windows::Win32::UI::WindowsAndMessaging::*;
    let hwnd = window.hwnd() as isize;
    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TRANSPARENT | WS_EX_LAYERED);
    }
}
```

**拖曳實現:**

```rust
struct DragState {
    dragging: bool,
    offset: (f64, f64),
}

// 在滑鼠事件中
match event {
    WindowEvent::CursorMoved { position, .. } => {
        if drag_state.dragging {
            let new_pos = LogicalPosition::new(
                position.x - drag_state.offset.0,
                position.y - drag_state.offset.1,
            );
            window.set_outer_position(new_pos);
        }
    }
    WindowEvent::MouseInput { state, button, .. } => {
        if button == MouseButton::Left {
            drag_state.dragging = state == ElementState::Pressed;
        }
    }
}
```

---

### 2. 螢幕截圖與 AI 分析

#### 功能需求

- [x] 全螢幕截圖
- [x] 截圖前自動隱藏應用視窗
- [x] 截圖失敗重試機制 (最多 3 次)
- [x] 截圖僅存於記憶體,不寫入磁碟
- [x] 支援開關截圖附加功能

#### 工作流程

```
使用者按下 Cmd+Enter → 開啟 Ask 面板 → 輸入問題 → 按 Send
    ↓
檢查 "Attach Screenshot" 是否勾選
    ↓ (是)
自動隱藏所有視窗 (200ms 延遲)
    ↓
擷取螢幕截圖 (PNG 格式,存於 Vec<u8>)
    ↓
顯示視窗
    ↓
發送請求到 OpenAI Vision API (base64 編碼圖片 + 文字提示)
    ↓
串流接收 AI 回應
    ↓
渲染 Markdown 回應
```

#### API 呼叫規格

**OpenAI Chat Completions API**

```rust
#[derive(Serialize)]
struct ChatRequest {
    model: String,  // "gpt-4o"
    messages: Vec<Message>,
    stream: bool,   // true
    max_tokens: Option<u32>,
    temperature: f32,
}

#[derive(Serialize)]
struct Message {
    role: String,  // "user" | "assistant" | "system"
    content: MessageContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    MultiModal(Vec<ContentPart>),
}

#[derive(Serialize)]
struct ContentPart {
    #[serde(rename = "type")]
    part_type: String,  // "text" | "image_url"
    text: Option<String>,
    image_url: Option<ImageUrl>,
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,  // "data:image/png;base64,..."
}
```

**截圖實現建議:**

```rust
use screenshots::Screen;

async fn capture_screen_with_retry() -> Result<Vec<u8>, CaptureError> {
    let mut attempts = 0;
    let max_attempts = 3;

    while attempts < max_attempts {
        match capture_screen_internal().await {
            Ok(buffer) => return Ok(buffer),
            Err(e) => {
                attempts += 1;
                if attempts < max_attempts {
                    tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempts))).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}

async fn capture_screen_internal() -> Result<Vec<u8>, CaptureError> {
    let screen = Screen::from_point(0, 0)?;
    let image = screen.capture()?;

    // 轉換為 PNG bytes
    let mut buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    Ok(buffer)
}
```

---

### 3. 即時語音轉錄

#### 功能需求

- [x] 麥克風音訊擷取
- [x] 系統音訊擷取 (桌面音訊)
- [x] 即時串流轉錄 (OpenAI Realtime API)
- [x] 支援暫停/恢復
- [x] 多語言支援 (英文/中文)
- [x] 顯示錄音時長

#### 音訊處理規格

**輸入要求:**

- 格式: PCM16 (16-bit signed integer)
- 採樣率: 24000 Hz (24 kHz)
- 聲道: 單聲道 (mono)
- 區塊大小: 3072 samples
- 批次大小: 最大 32 KB

**音訊處理管線:**

```
麥克風 → cpal Stream (原始採樣率,如 48kHz,雙聲道)
    ↓
雙聲道混合為單聲道 (平均左右聲道)
    ↓
重採樣到 24kHz (線性插值)
    ↓
轉換為 16-bit PCM (i16)
    ↓
累積到 3072 samples
    ↓
Base64 編碼
    ↓
透過 WebSocket 發送到 OpenAI
```

**Rust 實現建議:**

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};

struct AudioProcessor {
    stream: Option<Stream>,
    sample_buffer: Vec<i16>,
    target_sample_rate: u32,  // 24000
    source_sample_rate: u32,  // 從裝置獲取
    ws_sender: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl AudioProcessor {
    fn start_capture(&mut self) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(AudioError::NoDevice)?;
        let config = device.default_input_config()?;

        self.source_sample_rate = config.sample_rate().0;

        let ws_sender = self.ws_sender.clone();
        let mut buffer = Vec::new();

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 混合雙聲道 → 單聲道
                let mono: Vec<f32> = data.chunks(2)
                    .map(|chunk| (chunk[0] + chunk.get(1).unwrap_or(&0.0)) / 2.0)
                    .collect();

                // 重採樣
                let resampled = resample(&mono, source_sample_rate, 24000);

                // 轉換為 i16
                let pcm16: Vec<i16> = resampled.iter()
                    .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                    .collect();

                buffer.extend_from_slice(&pcm16);

                // 當累積到 3072 samples 時發送
                if buffer.len() >= 3072 {
                    let chunk = buffer.drain(..3072).collect::<Vec<_>>();
                    let bytes = chunk.iter()
                        .flat_map(|&s| s.to_le_bytes())
                        .collect::<Vec<_>>();
                    let base64 = base64::encode(&bytes);
                    let _ = ws_sender.try_send(base64.into_bytes());
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
        )?;

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }
}

fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let output_len = (input.len() as f64 * ratio) as usize;

    (0..output_len)
        .map(|i| {
            let src_idx = i as f64 / ratio;
            let idx0 = src_idx.floor() as usize;
            let idx1 = (idx0 + 1).min(input.len() - 1);
            let frac = src_idx - idx0 as f64;

            // 線性插值
            input[idx0] * (1.0 - frac as f32) + input[idx1] * frac as f32
        })
        .collect()
}
```

#### OpenAI Realtime API 整合

**WebSocket 連線:**

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn start_realtime_transcription(
    api_key: String,
    language: Language,
    tx: mpsc::Sender<TranscriptEvent>,
) -> Result<(), RealtimeError> {
    let url = "wss://api.openai.com/v1/realtime?model=gpt-4o-realtime-preview-2025-06-03";

    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    // 發送初始化配置
    let session_config = json!({
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "instructions": match language {
                Language::English => "Transcribe audio to English text.",
                Language::Chinese => "将音频转录为中文文本。",
            },
            "input_audio_format": "pcm16",
            "input_audio_transcription": {
                "model": "whisper-1"
            },
            "turn_detection": {
                "type": "server_vad",
                "threshold": 0.5,
                "prefix_padding_ms": 150,
                "silence_duration_ms": 350
            }
        }
    });

    write.send(Message::Text(session_config.to_string())).await?;

    // 監聽回應
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let event: serde_json::Value = serde_json::from_str(&text)?;

                match event["type"].as_str() {
                    Some("response.input_audio_transcription.delta") => {
                        let delta = event["delta"].as_str().unwrap_or("");
                        tx.send(TranscriptEvent::Delta(delta.to_string())).await?;
                    }
                    Some("response.input_audio_transcription.completed") => {
                        let transcript = event["transcript"].as_str().unwrap_or("");
                        tx.send(TranscriptEvent::Done(transcript.to_string())).await?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

**音訊數據傳送:**

```rust
// 接收來自音訊處理器的 base64 數據
while let Some(audio_data) = audio_rx.recv().await {
    let msg = json!({
        "type": "input_audio_buffer.append",
        "audio": String::from_utf8(audio_data)?
    });

    ws_write.send(Message::Text(msg.to_string())).await?;
}
```

---

### 4. 對話管理

#### 功能需求

- [x] 維護多輪對話上下文
- [x] 對話歷史分頁瀏覽
- [x] 支援重新生成答案
- [x] 清除對話並重置會話
- [x] 會話持久化 (可選)

#### 資料結構

```rust
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    id: Uuid,
    created_at: DateTime<Utc>,
    entries: Vec<ConversationEntry>,
    initial_prompt: Option<String>,  // 首次請求使用的系統提示詞
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationEntry {
    index: usize,
    request_id: Uuid,
    question: String,
    answer: String,
    reasoning: Option<String>,  // GPT-5 推理過程
    web_search_status: WebSearchStatus,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum WebSearchStatus {
    NotUsed,
    InProgress,
    Searching,
    Completed,
}

struct ConversationManager {
    current_session: Arc<Mutex<Session>>,
    history_index: Option<usize>,  // None = 顯示最新答案 (Live)
}

impl ConversationManager {
    fn new() -> Self {
        Self {
            current_session: Arc::new(Mutex::new(Session {
                id: Uuid::new_v4(),
                created_at: Utc::now(),
                entries: Vec::new(),
                initial_prompt: None,
            })),
            history_index: None,
        }
    }

    fn append_entry(&self, question: String) -> Uuid {
        let mut session = self.current_session.lock().unwrap();
        let request_id = Uuid::new_v4();

        session.entries.push(ConversationEntry {
            index: session.entries.len(),
            request_id,
            question,
            answer: String::new(),
            reasoning: None,
            web_search_status: WebSearchStatus::NotUsed,
            timestamp: Utc::now(),
        });

        request_id
    }

    fn update_answer(&self, request_id: Uuid, delta: &str) {
        let mut session = self.current_session.lock().unwrap();
        if let Some(entry) = session.entries.iter_mut()
            .find(|e| e.request_id == request_id) {
            entry.answer.push_str(delta);
        }
    }

    fn clear(&mut self) {
        self.current_session = Arc::new(Mutex::new(Session {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            entries: Vec::new(),
            initial_prompt: None,
        }));
        self.history_index = None;
    }

    fn paginate(&mut self, direction: Direction) {
        let session = self.current_session.lock().unwrap();
        let total = session.entries.len();

        if total == 0 { return; }

        match direction {
            Direction::Prev => {
                // 向前翻頁
                self.history_index = Some(match self.history_index {
                    None => total - 2,  // 從 Live 到倒數第二頁
                    Some(idx) if idx > 0 => idx - 1,
                    Some(idx) => idx,  // 已在第一頁
                });
            }
            Direction::Next => {
                // 向後翻頁
                self.history_index = match self.history_index {
                    Some(idx) if idx < total - 1 => Some(idx + 1),
                    Some(idx) if idx == total - 1 => None,  // 回到 Live
                    None => None,  // 已在 Live
                };
            }
        }
    }

    fn get_current_display(&self) -> Option<ConversationEntry> {
        let session = self.current_session.lock().unwrap();

        match self.history_index {
            None => session.entries.last().cloned(),
            Some(idx) => session.entries.get(idx).cloned(),
        }
    }

    fn get_page_label(&self) -> String {
        let session = self.current_session.lock().unwrap();
        let total = session.entries.len();

        match self.history_index {
            None => "Live".to_string(),
            Some(idx) => format!("{}/{}", idx + 1, total),
        }
    }
}
```

#### 重新生成實現

```rust
impl ConversationManager {
    async fn regenerate(&mut self, openai: &OpenAIClient) -> Result<Uuid, RegenerateError> {
        let session = self.current_session.lock().unwrap();

        // 獲取當前顯示的條目
        let current_idx = self.history_index.unwrap_or(session.entries.len() - 1);
        let entry = session.entries.get(current_idx)
            .ok_or(RegenerateError::NoEntry)?;

        let question = entry.question.clone();

        // 構建歷史上下文 (排除當前條目)
        let history = session.entries.iter()
            .take(current_idx)
            .map(|e| format!("Q: {}\nA: {}\n\n", e.question, e.answer))
            .collect::<String>();

        drop(session);  // 釋放鎖

        // 刪除當前條目及之後的所有條目
        {
            let mut session = self.current_session.lock().unwrap();
            session.entries.truncate(current_idx);
        }

        // 重新提交相同問題
        let request_id = self.append_entry(question.clone());

        // 發送請求,使用歷史上下文
        openai.send_request_with_history(question, history, request_id).await?;

        // 重置為 Live 模式
        self.history_index = None;

        Ok(request_id)
    }
}
```

---

### 5. 提示詞管理

#### 功能需求

- [x] 支援多個提示詞檔案
- [x] 使用者選擇活動提示詞
- [x] 僅在會話首次請求時注入提示詞
- [x] 提示詞檔案格式: `.txt`, `.md`, `.prompt`
- [x] 安全的檔案路徑處理

#### 存儲位置

```
~/.ghost-ai/
    ├── prompts/
    │   ├── default.txt
    │   ├── coding-assistant.md
    │   └── translator.prompt
    ├── logs/
    └── config.json
```

#### 資料結構

```rust
use std::path::{Path, PathBuf};
use std::fs;

struct PromptsManager {
    prompts_dir: PathBuf,
}

impl PromptsManager {
    fn new() -> Result<Self, PromptsError> {
        let home = dirs::home_dir().ok_or(PromptsError::NoHomeDir)?;
        let prompts_dir = home.join(".ghost-ai").join("prompts");

        // 創建目錄(如果不存在)
        fs::create_dir_all(&prompts_dir)?;

        Ok(Self { prompts_dir })
    }

    fn list_prompts(&self) -> Result<Vec<String>, PromptsError> {
        let mut prompts = Vec::new();

        for entry in fs::read_dir(&self.prompts_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "txt" || ext == "md" || ext == "prompt" {
                        if let Some(name) = path.file_stem() {
                            prompts.push(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        prompts.sort();
        Ok(prompts)
    }

    fn read_prompt(&self, name: &str) -> Result<String, PromptsError> {
        // 清理檔案名稱防止路徑遍歷
        let safe_name = name.replace(['/', '\\'], "_");

        // 嘗試不同副檔名
        for ext in &["txt", "md", "prompt"] {
            let path = self.prompts_dir.join(format!("{}.{}", safe_name, ext));

            if path.exists() {
                return Ok(fs::read_to_string(&path)?);
            }
        }

        Err(PromptsError::NotFound(name.to_string()))
    }
}
```

#### 設定整合

```rust
#[derive(Serialize, Deserialize)]
struct UserSettings {
    active_prompt: Option<String>,
    transcribe_language: Language,
    attach_screenshot: bool,
}

impl UserSettings {
    fn get_active_prompt(&self, prompts_mgr: &PromptsManager) -> Option<String> {
        self.active_prompt.as_ref()
            .and_then(|name| prompts_mgr.read_prompt(name).ok())
    }
}
```

---

### 6. 設定管理

#### 功能需求

- [x] OpenAI API 設定 (API Key, Base URL, Model)
- [x] 使用者偏好設定 (語言、截圖開關)
- [x] API Key 加密存儲
- [x] 設定持久化到磁碟
- [x] 設定驗證 (測試 API 連線)

#### 資料結構

```rust
use serde::{Serialize, Deserialize};
use keyring::Entry;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIConfig {
    #[serde(skip)]  // 不序列化 API Key
    api_key: String,
    base_url: String,
    model: String,
    timeout: u64,  // 秒
    max_tokens: Option<u32>,
    temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Language {
    English,
    Chinese,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSettings {
    transcribe_language: Language,
    attach_screenshot: bool,
    active_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    #[serde(flatten)]
    openai: OpenAIConfig,
    #[serde(flatten)]
    user: UserSettings,
}
```

#### 加密存儲實現

```rust
use keyring::Entry;
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

struct SettingsManager {
    config_path: PathBuf,
    keyring_entry: Entry,
}

impl SettingsManager {
    fn new() -> Result<Self, SettingsError> {
        let home = dirs::home_dir().ok_or(SettingsError::NoHomeDir)?;
        let config_dir = home.join(".ghost-ai");
        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.json");
        let keyring_entry = Entry::new("ghost-ai", "openai-api-key")?;

        Ok(Self { config_path, keyring_entry })
    }

    fn save_openai_config(&self, config: &OpenAIConfig) -> Result<(), SettingsError> {
        // 加密並儲存 API Key 到系統鑰匙圈
        self.keyring_entry.set_password(&config.api_key)?;

        // 儲存其他設定到 JSON (不含 API Key)
        let mut config_clone = config.clone();
        config_clone.api_key = String::new();  // 清空敏感資料

        let json = serde_json::to_string_pretty(&config_clone)?;
        fs::write(&self.config_path, json)?;

        Ok(())
    }

    fn load_openai_config(&self) -> Result<OpenAIConfig, SettingsError> {
        // 從 JSON 讀取非敏感設定
        let json = fs::read_to_string(&self.config_path)?;
        let mut config: OpenAIConfig = serde_json::from_str(&json)?;

        // 從鑰匙圈讀取 API Key
        config.api_key = self.keyring_entry.get_password()
            .unwrap_or_default();

        Ok(config)
    }

    fn save_user_settings(&self, settings: &UserSettings) -> Result<(), SettingsError> {
        // 合併 OpenAI 和使用者設定
        let mut config = self.load_config()?;
        config.user = settings.clone();

        let json = serde_json::to_string_pretty(&config)?;
        fs::write(&self.config_path, json)?;

        Ok(())
    }
}
```

#### 設定驗證

```rust
impl OpenAIClient {
    async fn validate_config(&self, config: &OpenAIConfig) -> Result<bool, ValidationError> {
        // 發送測試請求
        let client = reqwest::Client::new();

        let response = client
            .get(&format!("{}/models", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .timeout(Duration::from_secs(config.timeout))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn list_models(&self) -> Result<Vec<String>, OpenAIError> {
        let response = self.client
            .get(&format!("{}/models", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await?
            .json::<ModelsResponse>()
            .await?;

        // 過濾允許的模型
        let allowed = [
            "chatgpt-4o-latest",
            "gpt-4o",
            "gpt-4.1",
            "o4-mini-2025-04-16",
            "gpt-5",
            "gpt-5-mini",
        ];

        Ok(response.data.into_iter()
            .filter(|m| allowed.contains(&m.id.as_str()))
            .map(|m| m.id)
            .collect())
    }
}
```

---

### 7. 熱鍵管理

#### 功能需求

- [x] 全域熱鍵註冊
- [x] 跨平台支援 (Windows/macOS/Linux)
- [x] 固定快捷鍵組合 (不可自訂)

#### 快捷鍵列表

| 功能          | Windows/Linux    | macOS           |
| ------------- | ---------------- | --------------- |
| 開啟 Ask 面板 | Ctrl+Enter       | Cmd+Enter       |
| 開始/停止錄音 | Ctrl+Shift+Enter | Cmd+Shift+Enter |
| 隱藏/顯示應用 | Ctrl+\\          | Cmd+\\          |
| 清除對話      | Ctrl+R           | Cmd+R           |
| 向上捲動      | Ctrl+Up          | Cmd+Up          |
| 向下捲動      | Ctrl+Down        | Cmd+Down        |
| 上一頁        | Ctrl+Shift+Up    | Cmd+Shift+Up    |
| 下一頁        | Ctrl+Shift+Down  | Cmd+Shift+Down  |

#### Rust 實現

```rust
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

struct HotkeyManager {
    manager: GlobalHotKeyManager,
    hotkeys: Vec<HotKey>,
}

impl HotkeyManager {
    fn new() -> Result<Self, HotkeyError> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkeys = Vec::new();

        Ok(Self { manager, hotkeys })
    }

    fn register_fixed_hotkeys(&mut self) -> Result<(), HotkeyError> {
        // 根據平台選擇修飾鍵
        let cmd_or_ctrl = if cfg!(target_os = "macos") {
            Modifiers::META  // Cmd
        } else {
            Modifiers::CONTROL  // Ctrl
        };

        // Ask 面板
        let ask_hotkey = HotKey::new(Some(cmd_or_ctrl), Code::Enter);
        self.manager.register(ask_hotkey)?;
        self.hotkeys.push(ask_hotkey);

        // 錄音切換
        let audio_hotkey = HotKey::new(
            Some(cmd_or_ctrl | Modifiers::SHIFT),
            Code::Enter
        );
        self.manager.register(audio_hotkey)?;
        self.hotkeys.push(audio_hotkey);

        // 隱藏切換
        let hide_hotkey = HotKey::new(Some(cmd_or_ctrl), Code::Backslash);
        self.manager.register(hide_hotkey)?;
        self.hotkeys.push(hide_hotkey);

        // 清除對話
        let clear_hotkey = HotKey::new(Some(cmd_or_ctrl), Code::KeyR);
        self.manager.register(clear_hotkey)?;
        self.hotkeys.push(clear_hotkey);

        // 捲動和翻頁
        let scroll_up = HotKey::new(Some(cmd_or_ctrl), Code::ArrowUp);
        let scroll_down = HotKey::new(Some(cmd_or_ctrl), Code::ArrowDown);
        let page_prev = HotKey::new(Some(cmd_or_ctrl | Modifiers::SHIFT), Code::ArrowUp);
        let page_next = HotKey::new(Some(cmd_or_ctrl | Modifiers::SHIFT), Code::ArrowDown);

        self.manager.register(scroll_up)?;
        self.manager.register(scroll_down)?;
        self.manager.register(page_prev)?;
        self.manager.register(page_next)?;

        self.hotkeys.extend([scroll_up, scroll_down, page_prev, page_next]);

        Ok(())
    }

    fn unregister_all(&mut self) -> Result<(), HotkeyError> {
        for hotkey in &self.hotkeys {
            self.manager.unregister(*hotkey)?;
        }
        self.hotkeys.clear();
        Ok(())
    }
}

// 熱鍵事件處理
use global_hotkey::GlobalHotKeyEvent;

fn handle_hotkey_event(event: GlobalHotKeyEvent, app: &mut App) {
    match event.id {
        id if id == app.hotkeys.ask_hotkey.id() => {
            app.toggle_ask_panel();
        }
        id if id == app.hotkeys.audio_hotkey.id() => {
            app.toggle_recording();
        }
        id if id == app.hotkeys.hide_hotkey.id() => {
            app.toggle_hidden();
        }
        id if id == app.hotkeys.clear_hotkey.id() => {
            app.clear_conversation();
        }
        id if id == app.hotkeys.scroll_up.id() => {
            app.scroll_result(Direction::Up);
        }
        id if id == app.hotkeys.scroll_down.id() => {
            app.scroll_result(Direction::Down);
        }
        id if id == app.hotkeys.page_prev.id() => {
            app.paginate(Direction::Prev);
        }
        id if id == app.hotkeys.page_next.id() => {
            app.paginate(Direction::Next);
        }
        _ => {}
    }
}
```

---

### 8. 日誌管理

#### 功能需求

- [x] 記錄對話歷史到磁碟
- [x] 為每個會話建立獨立目錄
- [x] 儲存純文字日誌和結構化 JSON
- [x] 安全的檔案路徑處理

#### 存儲結構

```
~/.ghost-ai/logs/
    ├── {session_id_1}/
    │   ├── {session_id_1}.log       # 純文字對話
    │   └── {session_id_1}.json      # 結構化資料
    └── {session_id_2}/
        ├── {session_id_2}.log
        └── {session_id_2}.json
```

#### 日誌格式

**純文字日誌 (.log):**

```
System Prompt: You are a helpful AI assistant...

Q: What is Rust?
A: Rust is a systems programming language...

Q: How do I install it?
A: You can install Rust using rustup...
```

**結構化日誌 (.json):**

```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "created_at": "2025-10-01T12:34:56Z",
  "log_path": "/home/user/.ghost-ai/logs/550e8400.../550e8400....log",
  "entries": [
    {
      "index": 0,
      "request_id": "660e9500-e29b-41d4-a716-446655440001",
      "text_input": "What is Rust?",
      "ai_output": "Rust is a systems programming language...",
      "timestamp": "2025-10-01T12:35:10Z"
    }
  ]
}
```

#### Rust 實現

```rust
use std::path::PathBuf;
use std::fs;
use uuid::Uuid;

struct LogManager {
    logs_dir: PathBuf,
}

impl LogManager {
    fn new() -> Result<Self, LogError> {
        let home = dirs::home_dir().ok_or(LogError::NoHomeDir)?;
        let logs_dir = home.join(".ghost-ai").join("logs");
        fs::create_dir_all(&logs_dir)?;

        Ok(Self { logs_dir })
    }

    fn write_conversation_log(
        &self,
        session_id: Uuid,
        content: &str
    ) -> Result<PathBuf, LogError> {
        // 清理 session_id 防止路徑遍歷
        let safe_id = session_id.to_string()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>();

        let session_dir = self.logs_dir.join(&safe_id);
        fs::create_dir_all(&session_dir)?;

        let log_path = session_dir.join(format!("{}.log", safe_id));
        fs::write(&log_path, content)?;

        Ok(log_path)
    }

    fn write_session_json(
        &self,
        session: &Session
    ) -> Result<PathBuf, LogError> {
        let safe_id = session.id.to_string()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>();

        let session_dir = self.logs_dir.join(&safe_id);
        let json_path = session_dir.join(format!("{}.json", safe_id));

        let json = serde_json::to_string_pretty(session)?;
        fs::write(&json_path, json)?;

        Ok(json_path)
    }
}
```

---

## 技術架構設計

### 整體架構圖

```
┌─────────────────────────────────────────────────────────┐
│                      使用者介面層                         │
│  ┌─────────┬─────────┬─────────┬──────────┬──────────┐  │
│  │ HUD Bar │Ask Panel│Settings │Transcript│Recording │  │
│  │         │         │Panel    │Bubble    │Indicator │  │
│  └─────────┴─────────┴─────────┴──────────┴──────────┘  │
└─────────────────────┬───────────────────────────────────┘
                      │ UI Events / State Updates
┌─────────────────────▼───────────────────────────────────┐
│                   應用核心層 (App State)                  │
│  ┌─────────────────────────────────────────────────┐    │
│  │  ConversationManager                            │    │
│  │  SettingsManager                                │    │
│  │  PromptsManager                                 │    │
│  │  HotkeyManager                                  │    │
│  │  WindowManager                                  │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────────┘
                      │ Business Logic
┌─────────────────────▼───────────────────────────────────┐
│                    服務層 (Services)                      │
│  ┌──────────┬──────────┬──────────┬──────────────────┐  │
│  │  OpenAI  │  Audio   │Screenshot│  LogManager      │  │
│  │  Client  │Processor │Manager   │  SessionStore    │  │
│  └──────────┴──────────┴──────────┴──────────────────┘  │
└─────────────────────┬───────────────────────────────────┘
                      │ System Calls / External APIs
┌─────────────────────▼───────────────────────────────────┐
│                   系統層 (Platform)                       │
│  ┌──────────┬──────────┬──────────┬──────────────────┐  │
│  │  Window  │  Audio   │Screenshot│  File System     │  │
│  │  System  │  Devices │  APIs    │  Keyring         │  │
│  └──────────┴──────────┴──────────┴──────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 模組依賴關係

```
main
 ├─ window_manager (視窗創建、透明度、點擊穿透)
 ├─ hotkey_manager (全域熱鍵)
 ├─ app_state (全域狀態)
 │   ├─ conversation_manager
 │   ├─ settings_manager
 │   ├─ prompts_manager
 │   └─ log_manager
 └─ services
     ├─ openai_client
     │   └─ reqwest (HTTP)
     ├─ audio_processor
     │   ├─ cpal (音訊擷取)
     │   └─ tokio_tungstenite (WebSocket)
     └─ screenshot_manager
         └─ screenshots (截圖)
```

### 並行與非同步架構

**執行緒模型:**

```
主執行緒 (UI Event Loop)
  ├─ UI 渲染
  ├─ 使用者輸入處理
  └─ 視窗事件處理

Tokio Runtime (非同步任務)
  ├─ OpenAI API 請求 (串流)
  ├─ WebSocket 連線 (轉錄)
  ├─ 檔案 I/O (日誌寫入)
  └─ 定時器 (錄音計時)

音訊執行緒 (cpal callback)
  └─ 音訊數據處理
```

**通訊機制:**

```rust
use tokio::sync::{mpsc, broadcast, Mutex};
use std::sync::Arc;

struct AppChannels {
    // UI → 服務層
    ui_commands: mpsc::Sender<UICommand>,

    // 服務層 → UI
    ui_events: broadcast::Sender<UIEvent>,

    // 音訊處理器 → WebSocket
    audio_data: mpsc::Sender<Vec<u8>>,

    // OpenAI 串流 → UI
    stream_updates: broadcast::Sender<StreamUpdate>,
}

#[derive(Debug, Clone)]
enum UICommand {
    SubmitQuestion(String),
    ToggleRecording,
    ClearConversation,
    Paginate(Direction),
    UpdateSettings(UserSettings),
}

#[derive(Debug, Clone)]
enum UIEvent {
    StreamDelta { request_id: Uuid, delta: String },
    StreamDone { request_id: Uuid, content: String },
    TranscriptDelta(String),
    TranscriptDone(String),
    ErrorOccurred(String),
    SettingsUpdated,
}
```

### 錯誤處理策略

```rust
use thiserror::Error;

#[derive(Error, Debug)]
enum AppError {
    #[error("Screenshot capture failed: {0}")]
    ScreenshotError(#[from] screenshots::Error),

    #[error("Audio processing error: {0}")]
    AudioError(String),

    #[error("OpenAI API error: {0}")]
    OpenAIError(#[from] reqwest::Error),

    #[error("Settings error: {0}")]
    SettingsError(#[from] SettingsError),

    #[error("Hotkey registration failed: {0}")]
    HotkeyError(#[from] global_hotkey::Error),

    #[error("Window creation failed: {0}")]
    WindowError(String),
}

// 統一錯誤處理
impl App {
    fn handle_error(&mut self, error: AppError) {
        eprintln!("Error: {}", error);

        // 顯示使用者友好的錯誤訊息
        let user_message = match error {
            AppError::ScreenshotError(_) => "截圖失敗,請重試",
            AppError::AudioError(_) => "音訊錄製錯誤,請檢查麥克風權限",
            AppError::OpenAIError(_) => "API 請求失敗,請檢查網路連線和設定",
            AppError::SettingsError(_) => "設定載入失敗",
            AppError::HotkeyError(_) => "快捷鍵註冊失敗",
            AppError::WindowError(_) => "視窗創建失敗",
        };

        self.show_error_message(user_message);
    }
}
```

---

## 模組詳細規格

### OpenAI 客戶端模組

#### 職責

- 封裝 OpenAI API 呼叫
- 管理串流式回應
- 處理 Web 搜尋和推理功能

#### API 定義

```rust
use reqwest::Client;
use futures_util::StreamExt;

struct OpenAIClient {
    client: Client,
    config: Arc<Mutex<OpenAIConfig>>,
}

impl OpenAIClient {
    fn new(config: OpenAIConfig) -> Self {
        Self {
            client: Client::new(),
            config: Arc::new(Mutex::new(config)),
        }
    }

    async fn send_request_stream(
        &self,
        image: Option<Vec<u8>>,
        text_prompt: String,
        system_prompt: Option<String>,
        request_id: Uuid,
        tx: broadcast::Sender<StreamUpdate>,
    ) -> Result<String, OpenAIError> {
        let config = self.config.lock().await;

        // 構建訊息
        let mut messages = Vec::new();

        if let Some(prompt) = system_prompt {
            messages.push(Message {
                role: "system".to_string(),
                content: MessageContent::Text(prompt),
            });
        }

        let user_content = if let Some(img) = image {
            let base64 = base64::encode(&img);
            MessageContent::MultiModal(vec![
                ContentPart {
                    part_type: "text".to_string(),
                    text: Some(text_prompt),
                    image_url: None,
                },
                ContentPart {
                    part_type: "image_url".to_string(),
                    text: None,
                    image_url: Some(ImageUrl {
                        url: format!("data:image/png;base64,{}", base64),
                    }),
                },
            ])
        } else {
            MessageContent::Text(text_prompt)
        };

        messages.push(Message {
            role: "user".to_string(),
            content: user_content,
        });

        // 判斷使用的 API
        let is_responses_api = config.model.starts_with("gpt-5")
            || config.model.starts_with("o4-");

        let full_text = if is_responses_api {
            self.call_responses_api(&messages, &config, request_id, tx).await?
        } else {
            self.call_chat_completions_api(&messages, &config, request_id, tx).await?
        };

        Ok(full_text)
    }

    async fn call_chat_completions_api(
        &self,
        messages: &[Message],
        config: &OpenAIConfig,
        request_id: Uuid,
        tx: broadcast::Sender<StreamUpdate>,
    ) -> Result<String, OpenAIError> {
        let request = ChatRequest {
            model: config.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        };

        let mut response = self.client
            .post(&format!("{}/chat/completions", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&request)
            .send()
            .await?;

        let mut full_text = String::new();

        while let Some(chunk) = response.chunk().await? {
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];

                    if data == "[DONE]" {
                        break;
                    }

                    if let Ok(event) = serde_json::from_str::<ChatCompletionChunk>(data) {
                        if let Some(delta) = event.choices.first()
                            .and_then(|c| c.delta.content.as_ref()) {
                            full_text.push_str(delta);

                            let _ = tx.send(StreamUpdate {
                                request_id,
                                channel: Channel::Answer,
                                event_type: EventType::Delta,
                                delta: delta.clone(),
                                text: full_text.clone(),
                            });
                        }
                    }
                }
            }
        }

        let _ = tx.send(StreamUpdate {
            request_id,
            channel: Channel::Answer,
            event_type: EventType::Done,
            delta: String::new(),
            text: full_text.clone(),
        });

        Ok(full_text)
    }

    async fn call_responses_api(
        &self,
        messages: &[Message],
        config: &OpenAIConfig,
        request_id: Uuid,
        tx: broadcast::Sender<StreamUpdate>,
    ) -> Result<String, OpenAIError> {
        // Responses API 支援 web_search 和 reasoning
        let mut modalities = vec!["text".to_string()];

        if config.model.starts_with("gpt-5") {
            modalities.push("web_search".to_string());
        }

        let request = ResponsesRequest {
            model: config.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            modalities,
            reasoning_effort: if config.model.starts_with("gpt-5") {
                Some("high".to_string())
            } else {
                None
            },
            service_tier: if config.model.starts_with("gpt-5") {
                Some("priority".to_string())
            } else {
                None
            },
        };

        let mut response = self.client
            .post(&format!("{}/responses", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&request)
            .send()
            .await?;

        let mut answer_text = String::new();
        let mut reasoning_text = String::new();

        while let Some(chunk) = response.chunk().await? {
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("event: ") {
                    let event_type = &line[7..];

                    // 根據事件類型處理
                    match event_type {
                        "response.output_text.delta" => {
                            // 解析下一行 data
                            // (簡化示意,實際需要狀態機)
                        }
                        "response.reasoning_summary_text.delta" => {
                            // 推理過程增量
                        }
                        "response.web_search_call.in_progress" => {
                            let _ = tx.send(StreamUpdate {
                                request_id,
                                channel: Channel::WebSearch,
                                event_type: EventType::InProgress,
                                delta: String::new(),
                                text: String::new(),
                            });
                        }
                        "response.web_search_call.completed" => {
                            let _ = tx.send(StreamUpdate {
                                request_id,
                                channel: Channel::WebSearch,
                                event_type: EventType::Completed,
                                delta: String::new(),
                                text: String::new(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(answer_text)
    }
}

#[derive(Debug, Clone)]
struct StreamUpdate {
    request_id: Uuid,
    channel: Channel,
    event_type: EventType,
    delta: String,
    text: String,
}

#[derive(Debug, Clone)]
enum Channel {
    Answer,
    Reasoning,
    WebSearch,
}

#[derive(Debug, Clone)]
enum EventType {
    Delta,
    Done,
    InProgress,
    Searching,
    Completed,
}
```

---

### 音訊處理模組

#### 職責

- 擷取麥克風和系統音訊
- 音訊重採樣和格式轉換
- 批次處理並發送到 WebSocket

#### 完整實現

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

struct AudioProcessor {
    mic_stream: Option<cpal::Stream>,
    desktop_stream: Option<cpal::Stream>,
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    ws_sender: mpsc::Sender<Vec<u8>>,
    paused: Arc<Mutex<bool>>,
    elapsed_ms: Arc<Mutex<u64>>,
}

impl AudioProcessor {
    fn new(ws_sender: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            mic_stream: None,
            desktop_stream: None,
            sample_buffer: Arc::new(Mutex::new(Vec::new())),
            ws_sender,
            paused: Arc::new(Mutex::new(false)),
            elapsed_ms: Arc::new(Mutex::new(0)),
        }
    }

    async fn start(&mut self) -> Result<(), AudioError> {
        // 啟動麥克風
        self.start_microphone().await?;

        // 啟動系統音訊(最佳努力)
        let _ = self.start_desktop_audio().await;

        // 啟動計時器
        self.start_timer();

        Ok(())
    }

    async fn start_microphone(&mut self) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or(AudioError::NoMicrophoneDevice)?;

        let config = device.default_input_config()?;
        let source_rate = config.sample_rate().0;

        let buffer = Arc::clone(&self.sample_buffer);
        let sender = self.ws_sender.clone();
        let paused = Arc::clone(&self.paused);

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *paused.lock().unwrap() {
                    return;
                }

                // 混合雙聲道
                let mono: Vec<f32> = data.chunks(2)
                    .map(|ch| (ch[0] + ch.get(1).unwrap_or(&0.0)) / 2.0)
                    .collect();

                // 重採樣
                let resampled = resample_linear(&mono, source_rate, 24000);

                // 轉換為 i16
                let pcm16: Vec<i16> = resampled.iter()
                    .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .collect();

                // 追加到 buffer
                let mut buf = buffer.lock().unwrap();
                buf.extend_from_slice(&pcm16);

                // 當累積到 3072 samples 時發送
                while buf.len() >= 3072 {
                    let chunk: Vec<i16> = buf.drain(..3072).collect();
                    let bytes: Vec<u8> = chunk.iter()
                        .flat_map(|&s| s.to_le_bytes())
                        .collect();

                    let base64 = base64::encode(&bytes);
                    let _ = sender.try_send(base64.into_bytes());
                }
            },
            |err| eprintln!("Microphone stream error: {}", err),
        )?;

        stream.play()?;
        self.mic_stream = Some(stream);

        Ok(())
    }

    async fn start_desktop_audio(&mut self) -> Result<(), AudioError> {
        // 平台特定實現
        #[cfg(target_os = "windows")]
        {
            self.start_desktop_audio_windows().await
        }

        #[cfg(target_os = "macos")]
        {
            self.start_desktop_audio_macos().await
        }

        #[cfg(target_os = "linux")]
        {
            self.start_desktop_audio_linux().await
        }
    }

    fn start_timer(&self) {
        let elapsed = Arc::clone(&self.elapsed_ms);
        let paused = Arc::clone(&self.paused);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                if !*paused.lock().unwrap() {
                    *elapsed.lock().unwrap() += 100;
                }
            }
        });
    }

    fn pause(&mut self) {
        *self.paused.lock().unwrap() = true;
    }

    fn resume(&mut self) {
        *self.paused.lock().unwrap() = false;
    }

    fn stop(&mut self) {
        self.mic_stream = None;
        self.desktop_stream = None;
        self.sample_buffer.lock().unwrap().clear();
        *self.elapsed_ms.lock().unwrap() = 0;
    }

    fn get_elapsed_formatted(&self) -> String {
        let ms = *self.elapsed_ms.lock().unwrap();
        let secs = ms / 1000;
        let mins = secs / 60;
        let secs = secs % 60;

        format!("{:02}:{:02}", mins, secs)
    }
}

fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let output_len = (input.len() as f64 * ratio).ceil() as usize;

    (0..output_len)
        .map(|i| {
            let src_idx = i as f64 / ratio;
            let idx0 = src_idx.floor() as usize;
            let idx1 = (idx0 + 1).min(input.len() - 1);
            let frac = (src_idx - idx0 as f64) as f32;

            input[idx0] * (1.0 - frac) + input[idx1] * frac
        })
        .collect()
}
```

---

### 視窗管理模組

#### 職責

- 建立透明浮動視窗
- 管理視窗狀態(顯示/隱藏)
- 處理點擊穿透

#### 實現

```rust
use tao::window::{Window, WindowBuilder};
use tao::event_loop::EventLoop;

struct WindowManager {
    window: Window,
    hidden: bool,
    mouse_ignore: bool,
}

impl WindowManager {
    fn new(event_loop: &EventLoop<()>) -> Result<Self, WindowError> {
        let window = WindowBuilder::new()
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top(true)
            .with_skip_taskbar(true)
            .with_resizable(false)
            .with_title("Ghost AI")
            .with_inner_size(tao::dpi::LogicalSize::new(800, 600))
            .build(event_loop)?;

        let mut manager = Self {
            window,
            hidden: false,
            mouse_ignore: true,
        };

        manager.apply_platform_specific_settings()?;
        manager.set_mouse_ignore(true)?;

        Ok(manager)
    }

    fn apply_platform_specific_settings(&self) -> Result<(), WindowError> {
        #[cfg(target_os = "windows")]
        {
            self.apply_windows_settings()?;
        }

        #[cfg(target_os = "macos")]
        {
            self.apply_macos_settings()?;
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn apply_windows_settings(&self) -> Result<(), WindowError> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        let hwnd = HWND(self.window.hwnd() as isize);

        unsafe {
            // 設定點擊穿透
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            SetWindowLongW(
                hwnd,
                GWL_EXSTYLE,
                ex_style | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32
            );

            // 設定不透明度
            SetLayeredWindowAttributes(
                hwnd,
                COLORREF(0),
                (255.0 * 0.595) as u8,  // 59.5% 不透明度
                LWA_ALPHA
            );
        }

        Ok(())
    }

    fn set_mouse_ignore(&mut self, ignore: bool) -> Result<(), WindowError> {
        self.mouse_ignore = ignore;

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::*;

            let hwnd = HWND(self.window.hwnd() as isize);

            unsafe {
                let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);

                let new_style = if ignore {
                    ex_style | WS_EX_TRANSPARENT.0 as i32
                } else {
                    ex_style & !(WS_EX_TRANSPARENT.0 as i32)
                };

                SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
            }
        }

        Ok(())
    }

    fn toggle_hidden(&mut self) {
        self.hidden = !self.hidden;
        self.window.set_visible(!self.hidden);
    }
}
```

---

## 資料流與狀態管理

### 全域狀態定義

```rust
use tokio::sync::RwLock;

struct AppState {
    // UI 狀態
    hud_visible: bool,
    ask_panel_visible: bool,
    settings_panel_visible: bool,
    transcript_bubble_visible: bool,

    // 輸入狀態
    text_input: String,

    // 錄音狀態
    recording: bool,
    paused: bool,
    elapsed_ms: u64,

    // 對話狀態
    conversation: Arc<RwLock<ConversationManager>>,
    current_result: String,
    current_reasoning: String,
    web_search_status: WebSearchStatus,
    streaming: bool,

    // 設定
    settings: Arc<RwLock<UserSettings>>,
    openai_config: Arc<RwLock<OpenAIConfig>>,

    // 其他
    history_index: Option<usize>,
    attach_screenshot: bool,
}
```

### 狀態更新流程

**提交問題流程:**

```
使用者按 Send
  ↓
App::handle_submit()
  ↓
檢查 active_prompt
  ↓ (無提示詞)
顯示錯誤: "請在設定中選擇提示詞"
  ↓ (有提示詞)
conversation.append_entry(question)
  ↓
截圖 (如果 attach_screenshot = true)
  ↓
openai_client.send_request_stream()
  ↓
監聽 stream_updates channel
  ↓
接收 StreamUpdate::Delta
  ↓
更新 current_result
  ↓
觸發 UI 重繪
  ↓
接收 StreamUpdate::Done
  ↓
conversation.update_answer(request_id, full_text)
  ↓
log_manager.write_conversation_log()
  ↓
streaming = false
```

---

## API 與介面定義

### 公開 API (供 UI 調用)

```rust
impl App {
    // 對話相關
    pub async fn submit_question(&mut self, question: String) -> Result<(), AppError>;
    pub async fn regenerate_answer(&mut self) -> Result<(), AppError>;
    pub fn clear_conversation(&mut self);
    pub fn paginate(&mut self, direction: Direction);
    pub fn scroll_result(&mut self, direction: Direction);

    // 錄音相關
    pub async fn toggle_recording(&mut self) -> Result<(), AppError>;
    pub fn pause_recording(&mut self);
    pub fn resume_recording(&mut self);
    pub fn stop_recording(&mut self);

    // UI 相關
    pub fn toggle_ask_panel(&mut self);
    pub fn toggle_settings_panel(&mut self);
    pub fn toggle_hidden(&mut self);
    pub fn set_mouse_ignore(&mut self, ignore: bool);

    // 設定相關
    pub async fn update_openai_config(&mut self, config: OpenAIConfig) -> Result<(), AppError>;
    pub async fn validate_config(&self) -> Result<bool, AppError>;
    pub async fn list_models(&self) -> Result<Vec<String>, AppError>;
    pub async fn update_user_settings(&mut self, settings: UserSettings) -> Result<(), AppError>;

    // 提示詞相關
    pub fn list_prompts(&self) -> Result<Vec<String>, AppError>;
    pub fn get_active_prompt(&self) -> Option<String>;
    pub fn set_active_prompt(&mut self, name: String) -> Result<(), AppError>;
}
```

---

## UI/UX 規格

### 顏色主題

```rust
struct Theme {
    opacity: f32,  // 0.595

    // 色板 (RGB)
    text: (u8, u8, u8),           // (255, 255, 255)
    muted_text: (u8, u8, u8),     // (189, 189, 189)
    bar_bg: (u8, u8, u8),         // (30, 30, 30)
    settings_bg: (u8, u8, u8),    // (20, 20, 20)
    panel_bg: (u8, u8, u8),       // (22, 22, 22)
    primary: (u8, u8, u8),        // (43, 102, 246)
    danger: (u8, u8, u8),         // (255, 40, 40)
    border: (u8, u8, u8),         // (60, 60, 60)
}

impl Theme {
    fn text_color(&self) -> Color {
        self.rgba(self.text, 1.0)
    }

    fn primary_color(&self, multiplier: f32) -> Color {
        self.rgba(self.primary, multiplier)
    }

    fn rgba(&self, rgb: (u8, u8, u8), alpha_multiplier: f32) -> Color {
        Color::rgba(
            rgb.0,
            rgb.1,
            rgb.2,
            (self.opacity * alpha_multiplier * 255.0) as u8
        )
    }
}
```

### 佈局規格

**HUD 控制列:**

- 寬度: 自適應內容
- 高度: 48px
- 圓角: 24px
- 背景: bar_bg with opacity
- 陰影: 0 4px 16px rgba(0,0,0,0.3)
- 間距: 12px padding, 8px gap

**Ask 面板:**

- 寬度: 600px
- 高度: 最大 500px
- 圓角: 12px
- 背景: panel_bg with opacity
- 邊框: 1px solid border
- 陰影: 0 8px 32px rgba(0,0,0,0.4)

**Settings 面板:**

- 寬度: 500px
- 高度: 自適應內容
- 圓角: 12px
- 其他同 Ask 面板

### 字型

```rust
const FONT_FAMILY: &str = if cfg!(target_os = "windows") {
    "Segoe UI, sans-serif"
} else if cfg!(target_os = "macos") {
    "-apple-system, BlinkMacSystemFont, sans-serif"
} else {
    "system-ui, sans-serif"
};

const FONT_SIZE_NORMAL: f32 = 14.0;
const FONT_SIZE_SMALL: f32 = 12.0;
const FONT_SIZE_LARGE: f32 = 16.0;
```

---

## 安全與隱私要求

### 1. API Key 保護

- ✅ 使用系統鑰匙圈存儲 (keyring crate)
- ✅ 記憶體中加密 (不以明文儲存)
- ✅ 不寫入日誌檔案
- ✅ UI 中使用 password 輸入框

### 2. 截圖安全

- ✅ 截圖僅存於記憶體 (Vec\<u8>)
- ✅ 使用後立即清除
- ✅ 不寫入臨時檔案
- ✅ 不寫入日誌

### 3. 檔案路徑安全

```rust
fn sanitize_path(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}
```

### 4. 隱身模式

- ✅ 視窗設定 `content_protection` 防截圖
- ✅ 點擊穿透避免干擾
- ✅ 不顯示在工作列
- ✅ 無邊框、無標題列

---

## 效能與最佳化

### 1. 記憶體管理

- 使用 Arc 共享不可變資料
- 使用 Mutex/RwLock 保護可變狀態
- 及時清理對話歷史 (提供 clear 功能)
- 音訊 buffer 大小限制

### 2. 並行處理

- 使用 Tokio 非同步執行時
- WebSocket 和 HTTP 請求非阻塞
- UI 執行緒不執行重計算

### 3. 快取策略

- 設定檔案快取在記憶體
- 提示詞檔案按需載入
- 模型列表快取 (TTL: 5分鐘)

---

## 測試策略

### 單元測試

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_linear() {
        let input = vec![0.0, 1.0, 0.0, -1.0];
        let output = resample_linear(&input, 4, 8);
        assert_eq!(output.len(), 8);
    }

    #[tokio::test]
    async fn test_conversation_append() {
        let mut mgr = ConversationManager::new();
        let id = mgr.append_entry("test question".to_string());

        let session = mgr.current_session.lock().unwrap();
        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].request_id, id);
    }
}
```

### 整合測試

- OpenAI API 呼叫 (使用 mock server)
- 音訊處理管線
- 截圖擷取
- 設定持久化

---

## 部署與打包

### Cargo.toml 配置

```toml
[package]
name = "ghost-ai"
version = "1.0.0"
edition = "2021"

[dependencies]
# UI 框架 (三選一)
# iced = "0.12"
# egui = "0.27"
tauri = { version = "2.0", features = ["shell-open"] }

# 非同步執行時
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"

# HTTP 客戶端
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# WebSocket
tokio-tungstenite = "0.23"

# 音訊
cpal = "0.15"

# 截圖
screenshots = "0.8"

# 熱鍵
global-hotkey = "0.5"

# 存儲
keyring = "2"
dirs = "5"

# 加密
base64 = "0.22"

# 錯誤處理
thiserror = "1"
anyhow = "1"

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# 日期時間
chrono = { version = "0.4", features = ["serde"] }

# Markdown (如果使用 iced/egui)
pulldown-cmark = "0.11"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_Graphics_Gdi",
] }
```

### 跨平台打包

**Windows:**

```bash
cargo build --release
cargo install cargo-wix
cargo wix init
cargo wix
```

**macOS:**

```bash
cargo build --release
cargo install cargo-bundle
cargo bundle --release
```

**Linux:**

```bash
cargo build --release
cargo install cargo-deb
cargo deb
```

---

## 遷移路線圖

### 階段 1: 核心框架 (1-2 週)

- [ ] 選擇 UI 框架 (Tauri / iced / egui)
- [ ] 實現基本視窗管理
  - [ ] 透明視窗
  - [ ] 點擊穿透
  - [ ] 拖曳定位
- [ ] 實現 HUD 控制列 UI
- [ ] 設定管理模組
- [ ] 熱鍵管理模組

### 階段 2: OpenAI 整合 (1 週)

- [ ] 實現 OpenAI 客戶端
  - [ ] Chat Completions API
  - [ ] Responses API
  - [ ] 串流處理
- [ ] 實現 Ask 面板 UI
- [ ] Markdown 渲染器
- [ ] 對話管理模組

### 階段 3: 截圖功能 (3-5 天)

- [ ] 截圖管理模組
- [ ] 視窗隱藏邏輯
- [ ] 圖片編碼和傳輸
- [ ] 截圖開關功能

### 階段 4: 語音轉錄 (1-2 週)

- [ ] 音訊擷取 (麥克風)
- [ ] 音訊處理管線
  - [ ] 重採樣
  - [ ] 格式轉換
  - [ ] 批次處理
- [ ] WebSocket 整合 (Realtime API)
- [ ] 轉錄 UI (TranscriptBubble)
- [ ] 系統音訊擷取 (平台特定)

### 階段 5: 進階功能 (1 週)

- [ ] 提示詞管理
- [ ] 對話翻頁
- [ ] 重新生成答案
- [ ] Web 搜尋指示器
- [ ] 推理過程顯示

### 階段 6: 日誌與持久化 (3-5 天)

- [ ] 日誌管理模組
- [ ] 會話存儲
- [ ] 設定持久化

### 階段 7: 測試與優化 (1 週)

- [ ] 單元測試
- [ ] 整合測試
- [ ] 效能優化
- [ ] 記憶體洩漏檢查

### 階段 8: 打包與部署 (3-5 天)

- [ ] Windows 打包
- [ ] macOS 打包
- [ ] Linux 打包
- [ ] 安裝程式
- [ ] 文件撰寫

**預計總時間: 6-8 週**

---

## 附錄

### A. 依賴庫完整列表

| 庫名稱            | 用途          | 替代方案    |
| ----------------- | ------------- | ----------- |
| tauri             | 桌面應用框架  | iced, egui  |
| tokio             | 非同步執行時  | async-std   |
| reqwest           | HTTP 客戶端   | hyper, ureq |
| tokio-tungstenite | WebSocket     | tungstenite |
| cpal              | 音訊擷取      | rodio       |
| screenshots       | 截圖          | xcap        |
| global-hotkey     | 全域熱鍵      | -           |
| keyring           | 鑰匙圈存儲    | -           |
| serde             | 序列化        | -           |
| pulldown-cmark    | Markdown 解析 | comrak      |

### B. 原專案與 Rust 專案對照表

| 原檔案 (TypeScript)                 | Rust 模組              |
| ----------------------------------- | ---------------------- |
| main/main.ts                        | src/main.rs            |
| main/modules/hotkey-manager.ts      | src/hotkey.rs          |
| main/modules/screenshot-manager.ts  | src/screenshot.rs      |
| main/modules/settings-manager.ts    | src/settings.rs        |
| main/modules/prompts-manager.ts     | src/prompts.rs         |
| main/modules/realtime-transcribe.ts | src/audio/realtime.rs  |
| main/modules/log-manager.ts         | src/log.rs             |
| main/modules/session-store.ts       | src/conversation.rs    |
| shared/openai-client.ts             | src/openai.rs          |
| hooks/useTranscription.ts           | src/audio/processor.rs |
| components/App.tsx                  | src/ui/app.rs          |
| components/AskPanel.tsx             | src/ui/ask_panel.rs    |
| components/HUDBar.tsx               | src/ui/hud.rs          |
| components/Settings.tsx             | src/ui/settings.rs     |

### C. 關鍵決策記錄

**決策 1: UI 框架選擇**

- **建議:** Tauri (如需快速遷移) 或 iced (純 Rust)
- **理由:**
  - Tauri 允許重用 React 組件
  - iced 提供純 Rust 體驗和更好的效能
- **風險:** iced 的 Markdown 渲染需自行實現

**決策 2: 音訊處理位置**

- **選擇:** 在主應用中處理 (而非獨立進程)
- **理由:** 簡化架構,減少 IPC 開銷
- **風險:** 音訊處理可能影響 UI 流暢度 (需測試)

**決策 3: 狀態管理**

- **選擇:** Arc\<Mutex\<T>> / Arc\<RwLock\<T>>
- **理由:** 簡單直接,適合中小型應用
- **替代:** 使用狀態管理庫 (如 redux-rs)

**決策 4: 錯誤處理**

- **選擇:** thiserror + Result\<T, E>
- **理由:** Rust 慣用做法,類型安全
- **替代:** anyhow (更靈活但失去類型資訊)

---

## 結語

本規格文件提供了將 Ghost AI 從 Electron/TypeScript 遷移到 Rust 的完整藍圖。主要優勢包括:

✅ **效能提升** - 原生代碼,更低的記憶體佔用
✅ **安全性** - Rust 的類型系統和所有權模型
✅ **可維護性** - 強型別和編譯時檢查
✅ **跨平台** - 統一的程式碼庫,原生編譯

**後續步驟:**

1. 選擇 UI 框架 (建議先用 Tauri 快速驗證概念)
2. 實現 MVP (HUD + Ask 面板 + OpenAI 整合)
3. 逐步添加音訊轉錄和進階功能
4. 測試和優化
5. 打包和發布

祝開發順利! 🦀
