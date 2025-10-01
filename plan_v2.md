**專案概觀**

- 產品定位：隱私優先、跨平台的桌面 AI 助手。「隱形 HUD」可用熱鍵即時叫出，支援截圖＋視覺問答、語音即時轉錄、與對話歷史導覽／重生。
- 既有技術棧：Electron + React + Vite（前端），主進程管理全域熱鍵、螢幕截圖、OpenAI 串流、會話與日誌，Renderer 經由 `preload` 使用 IPC 互動。
- 外部依賴：OpenAI API（Responses API 串流、Models 列表、Realtime 轉錄 WebSocket）。
- 資料儲存：`~/.ghost-ai`（設定、使用者偏好、提示詞、會話日誌）。OpenAI 設定採 `safeStorage` 加密；提示詞檔僅讀；截圖只在記憶體處理，不落地。

**現有功能盤點**

- HUD/Overlay（src/main/main.ts, src/components/\*）

  - 透明、無框、置頂、略過工作列；預設 click-through，滑過 UI 時取消 click-through。
  - 托盤選單：`Show Overlay`、`Toggle Hide`、`Quit`；應用選單（File/View）含顯示、隱藏、Reload(F5)、Toggle DevTools。
  - 視覺元素：上方 HUDBar（Listen/Ask/Hide/Settings），下方氣泡（Ask 面板/設定卡/轉錄泡泡）。可拖曳 HUDBar。

- 全域熱鍵（src/main/modules/hotkey-manager.ts, src/main/main.ts）

  - `Ctrl/Cmd+Enter`：切換 Ask 面板（與 Audio Toggle 有抑制機制，避免連動）。
  - `Ctrl/Cmd+\`：切換隱藏 HUD。
  - `Ctrl/Cmd+R`：清除會話（中止分析串流、重置記憶體歷史、清空 SessionStore、滾動 ID）。
  - `Ctrl/Cmd+Shift+Enter`：語音錄音開始/停止（與 Ask 切換互斥抑制 400ms）。
  - `Ctrl/Cmd+Up/Down`：Ask 區塊內捲動結果。
  - `Ctrl/Cmd+Shift+Up/Down`：歷史回答分頁導覽（上一頁／下一頁／回到 Live）。

- 截圖與分析（src/main/modules/screenshot-manager.ts, src/main/modules/hide-manager.ts, src/shared/openai-client.ts, src/main/main.ts）

  - 截圖：`screenshot-desktop` 取得 PNG Buffer，最多重試 3 次（200ms/400ms/800ms 退避）。
  - 螢幕隱藏：呼叫 `hideAllWindowsDuring` 在擷取時隱藏所有視窗，避免截到 HUD。
  - 是否附圖：由使用者偏好 `attachScreenshot` 決定（預設 true）。
  - 分析請求：走 Responses API 串流（`openAIClient.responseStream`），輸入包含：
    - system：使用「活動提示詞」（僅本會話第一輪載入）。
    - user：問題文字＋若有圖像則以 `data:image/png;base64` 形式附圖。
    - tools：`web_search_preview`。
  - 串流事件：`response.reasoning_*`（reasoning channel）、`response.web_search_call.*`（web_search channel）、`response.output_text.delta/done`（answer channel）。
  - 主進程以 IPC 廣播四事件：`capture:analyze-stream:start|delta|done|error`，Renderer據以更新 UI。

- 提示詞（src/main/modules/prompts-manager.ts, Settings.tsx）

  - 路徑：`~/.ghost-ai/prompts`。
  - 僅讀模式：不會自動建立預設檔，`ensureDefaultPrompt` 為 no-op。
  - 活動提示詞：名稱持久化於使用者設定；第一輪分析必須存在檔案，否則回傳錯誤訊息「No active prompt selected…」。

- 會話與歷史（src/main/main.ts, src/main/modules/session-store.ts, src/main/modules/log-manager.ts）

  - 以 `currentSessionId` 標識；`Ctrl/Cmd+R` 清除後重生新 ID。
  - 記憶體內累積純文字 QA 歷史（格式：`Q: ...\nA: ...`）。
  - 第一輪使用的初始提示詞會快取於 `initialPromptBySession`，在重生（regenerate）時維持一致上下文。
  - 寫檔：`~/.ghost-ai/logs/<sessionId>/<sessionId>.log`（純文字）與 `<sessionId>.json`（SessionStore 摘要）。

- 重生（Regenerate）（App.tsx, AskPanel.tsx, main.ts）

  - 取當頁回答對應的前一用戶消息作為提問，再用「先前配對 QA」組成 `priorPlain` 傳回主進程；主進程重建上下文並重新請求串流。
  - 成功後更新歷史並回到 Live；串流過程可取消／清除。

- 語音即時轉錄（src/main/modules/realtime-transcribe.ts, src/hooks/useTranscription.ts）

  - UI：WebAudio 混合麥克風＋系統音，重採樣為 24kHz mono，打包 PCM16 base64，批次送至主進程。
  - 主進程：以 `wss://api.openai.com/v1/realtime?intent=transcription` 連線，`server_vad`，語言 `en|zh`，模型預設 `gpt-4o-mini-transcribe`（程式內使用 `gpt-4o-realtime-preview-2025-06-03` 作為 WS 端的轉錄模型）。
  - 事件：`transcribe:start|delta|done|error|closed`；Renderer 將片段累積成轉錄內容並可直接作為 Ask 前綴。

- 設定與模型（src/components/Settings.tsx, src/main/modules/settings-manager.ts, src/shared/openai-client.ts）

  - OpenAI 設定：`apiKey`、`baseURL`、`model`、`timeout`、`maxTokens?`、`temperature?`。
  - 憑證保存：加密 JSON（`safeStorage` 可用時使用），另將 `baseURL`、`model` 明文存放以利顯示。
  - 模型列表：以 `client.models.list()` 過濾白名單 `allowedModels=[chatgpt-4o-latest,gpt-4o,gpt-4.1,o4-mini-2025-04-16,gpt-5,gpt-5-mini]`，失敗回傳白名單作 fallback。
  - 驗證設定：以 `models.list()` 成功與否作為有效性檢測。
  - 使用者偏好：`transcribeLanguage(en|zh)`、`attachScreenshot(boolean)` 等。

- IPC 與事件（src/main/preload.ts, src/main/main.ts）

  - Renderer → Main（invoke/send）：
    - `openai:update-config`、`openai:update-config-volatile`、`openai:get-config`、`openai:list-models`、`openai:validate-config`。
    - `settings:get|update`、`prompts:list|read|get-default|set-default|get-active|set-active`。
    - `hud:toggle-hide|set-mouse-ignore`、`app:quit`。
    - `session:get|new|dump`。
    - `capture:analyze-stream`（send）。
    - `transcribe:start`（invoke）、`transcribe:append|end|stop`（send）。
  - Main → Renderer（on）：
    - UI：`text-input:show|toggle`、`hud:show`、`ask:clear`、`ask:scroll`、`ask:paginate`、`audio:toggle`、`session:changed`、`openai:config-updated`。
    - 串流：`capture:analyze-stream:start|delta|done|error`；轉錄：`transcribe:start|delta|done|error|closed`。

**Rust 重寫技術架構**

- 平台選型

  - 建議使用 Tauri（Rust 主程式 + Web 前端）以替代 Electron：
    - 優點：跨平台、可重用現有 React 前端與瀏覽器媒體能力（getUserMedia/DisplayMedia）。
    - 另案：若完全移除 Web 前端，則以 `winit+wry` 自繪 UI 或以 `iced/dioxus`，成本高、功能等量移植較久。

- 專案結構（建議）

  - `apps/ghost-desktop`：Tauri 殼層（視窗、托盤、全域熱鍵、IPC 命令、事件）。
  - `crates/ghost-core`：核心邏輯（OpenAI 客戶端、會話、日誌、提示詞、設定、截圖、音訊 WS）。
  - `web/`：前端（可移植既有 React/Vite 專案）。

- 模組設計對應

  - 視窗/HUD：
    - Tauri `WindowBuilder`：透明、無框、置頂、跳過任務列；加入自定義屬性以支援 click-through。
    - Click-through：平台特化 API：
      - Windows：設置 `WS_EX_TRANSPARENT | WS_EX_LAYERED`（wry/tauri plugin），必要時透過 `winapi` 呼叫。
      - macOS：`NSWindow.ignoresMouseEvents = true`；
      - Linux(X11/Wayland)：以 wry 的 `set_ignore_cursor_events` 或輔以 shape/input region。
  - 全域熱鍵：`tauri-plugin-global-shortcut` 或 `rdev`；註冊固定對應快捷鍵與失敗回報。
  - 截圖：`screenshot-rs` 或 `scrap`/`display-info`，支援多螢幕（預設主顯示器）。保留 3 次指數退避策略。
  - 隱藏擷取：呼叫所有 Tauri 視窗 `hide()`，完成後還原 `show_inactive()`。
  - OpenAI（HTTP 串流）：
    - `reqwest` + `tokio`：Responses API 串流解析 SSE 或 chunk；映射 reasoning/web_search/answer 三通道事件。
    - 模型白名單與 `list models` 驗證。
    - `gpt-5` 需加上 `reasoning` 與 `service_tier` 參數（與現有一致）。
  - OpenAI（Realtime WS）：
    - `tokio-tungstenite`：連線 `wss://api.openai.com/v1/realtime?intent=transcription`，送 `transcription_session.update`，`input_audio_format=pcm16`，`turn_detection=server_vad`。
    - 收 `conversation.item.input_audio_transcription.delta|completed`，聚合後透過事件送往 UI。
  - 音訊擷取（若沿用 Web 前端，則無需原生擷取）：
    - 沿用瀏覽器 getUserMedia/DisplayMedia，Renderer 批次 base64 PCM 送 IPC。
    - 若改原生：`cpal` 擷取、重採樣（如 `rubato`/`speexdsp`）、PCM16 打包後推 WS。
  - 設定與儲存：
    - 目錄：`~/.ghost-ai`；使用 `directories` crate 取得家目錄。
    - 加密：優先 OS Keyring（`keyring`）或 libsodium；否則以隨機主金鑰（儲存在 OS keyring）加解 OpenAI JSON。
    - 使用者偏好：JSON（等價欄位：`transcribeLanguage`、`attachScreenshot` 等）。
  - 提示詞：
    - 僅讀 `~/.ghost-ai/prompts/<name>`，不自動建立檔案。活動名稱存於設定。
  - 會話與日誌：
    - 記憶體保存 QA 累加字串、初始提示詞快取；
    - `~/.ghost-ai/logs/<sessionId>/<sessionId>.log|.json` 同名檔寫入；
    - `Ctrl/Cmd+R` 重置 sessionId、落盤空檔、廣播 `session:changed`。
  - IPC 事件模型：
    - 命令（前端→主程式）名稱與資料結構對齊現有 IPC；
    - 事件（主程式→前端）對齊名稱與負載欄位，確保無痛移植 UI。

- 資料結構（Rust 等價）

  - `OpenAIConfig { api_key, base_url, model, timeout, max_tokens: Option<i32>, temperature: Option<f32> }`
  - `AnalysisResult { request_id, content, model, timestamp, session_id }`
  - `SessionEntry { index, request_id, text_input, ai_output }`
  - `UserSettings { transcribe_language: Enum(en|zh), attach_screenshot: bool, default_prompt: String, ... }`

**行為細節與對應（保真要求）**

- Ask 首輪：
  - 需成功讀到活動提示詞（檔案存在）；否則回傳錯誤訊息並不送出請求。
  - 問題文字會在主進程與 `priorPlain` 合併成 `combinedTextPrompt`；若有 `priorPlain` 則以「Previous conversation ... / New question ...」格式包裝。
  - 截圖僅在 `attachScreenshot==true` 時擷取，且 HUD 隱藏後再擷取。
- 重生：
  - 以 UI 提供之 `history` 覆蓋 prior；主進程自動補上第一輪提示詞前綴，避免遺失初始上下文。
  - 更新記憶體歷史時，重生不重覆累加上一輪答案（以 `history` 作為基底再附加）。
- 清除（`Ctrl/Cmd+R`）：
  - 中止進行中 analyze 串流；清空歷史與初始提示詞快取；停止轉錄；重設 `currentSessionId`；初始化新 session 日誌；廣播 `session:changed`。
- Click-through：
  - 預設忽略滑鼠；滑過 HUD/面板時取消忽略；移出後恢復忽略。
- 模型處理：
  - `openai:list-models` 過濾白名單；`openai:validate-config` 以 `models.list()` 成功為真。
  - `gpt-5` 額外傳遞 `reasoning/service_tier`；Responses API 使用 `tools=[web_search_preview]`。
- 轉錄：
  - WS open 後送 `transcription_session.update` 含 `input_audio_format=pcm16`、`turn_detection.server_vad`、語言（`en|zh`）。
  - `delta` 串流片段在 UI 累加；`completed` 時送出一次完整句子。

**非功能性需求**

- 跨平台：Windows 10+、macOS 12+、Ubuntu 22.04+。
- 安全與隱私：
  - 不將截圖落地；OpenAI 設定加密儲存；提示詞僅讀；
  - 會話日誌屬選擇性（現有行為為自動落地，Rust 版應提供開關）。
- 效能：
  - 截圖→首個 token 延遲 < 800ms（無附圖場景 < 400ms，取決於模型）。
  - 轉錄延遲：從語音輸入到 `delta` < 300ms（依網路）。
- 穩定性：所有 IPC/WS 需防呆與重試；Abort/Stop 時靜默處理。

**驗收與測試（Parity）**

- 熱鍵：
  - 每個熱鍵動作觸發對應事件（含 UI 聚焦、視窗顯示、抑制邏輯）。
- 截圖分析：
  - 附圖/不附圖兩模式；三次退避；HUD 不入鏡；串流三通道事件順序與語意一致。
  - 首輪未選取提示詞時，顯示錯誤提示且不發出 API 請求。
- 歷史與重生：
  - QA 拼接格式、重生不重覆答案、初始提示詞維持。
- 轉錄：
  - `start/append/end/stop` 全流程；WS 斷線自動清理；句子合併正確。
- 設定：
  - 加密保存、模型列表刷新（含 volatile 更新）、語言與附圖選項保存。
- 日誌：
  - `.log` 與 `.json` 內容正確，路徑格式正確。

**遷移步驟與里程碑**

- M0：腳手架
  - 初始化 Tauri 專案、設定 CI（lint/format/build）。
  - 建立 `ghost-core` crate：型別、設定、日誌、提示詞 IO。
- M1：視窗與熱鍵
  - 建 HUD 視窗、click-through、托盤、應用選單；全域熱鍵註冊與抑制邏輯。
- M2：設定與模型
  - 實作設定加密存取、`list-models`/`validate`、前端 Settings 對接。
- M3：截圖與 Responses 串流
  - 3 次退避擷取；Responses API 串流解析；三通道事件；會話歷史整合；Abort 控制。
- M4：會話與日誌
  - SessionStore、清除與重生；`.log/.json` 寫入；UI 分頁導覽。
- M5：Realtime 轉錄
  - WS 管理、語言與 VAD 設定、瀏覽器端音訊管線、串流事件全鏈路。
- M6：跨平台修整與打包
  - Windows/macOS/Linux 特性修正；打包與簽章；Release 腳本。
- M7：隱私與診斷
  - 會話日誌開關、除錯 `session:dump` 等輔助；最終驗收。

**相依建議（Rust）**

- OpenAI HTTP：`reqwest`（sse/chunk 解析）或直接讀取 `bytes_stream`。
- OpenAI WS：`tokio`, `tokio-tungstenite`。
- 截圖：`screenshot` crate（或 `scrap`）。
- 熱鍵：`tauri-plugin-global-shortcut` 或 `rdev`。
- 儲存：`serde`/`serde_json`、`directories`、`keyring`（或 `ring`+自管金鑰）。
- 事件/命令：Tauri `emit`/`invoke` 封裝 IPC。

**風險與決策點**

- Click-through 跨平台差異：需要平台特化 API；Windows/Wayland 可能需更低階的視窗旗標或區域裁切。
- Realtime 轉錄：OpenAI API 版本更新頻率高，需妥善處理相容與降級（模型不可用時的提示與 fallback）。
- 截圖權限：macOS 需螢幕錄製權限；Linux Wayland 需 portal 介面（可能影響無頭擷取）。
- 設定加密：不同 OS keyring 行為差異；需合理降級策略（例如加密失效時提醒並改為純文字本機儲存，或阻擋）。

**開發說明與對應 API（摘要）**

- 命令（擬定 Tauri `#[tauri::command]` 對應）：
  - `openai_update_config(OpenAIConfigPartial)`、`openai_update_config_volatile(OpenAIConfigPartial)`、`openai_get_config()`、`openai_list_models()`、`openai_validate_config(OpenAIConfig)`
  - `settings_get()`、`settings_update(UserSettingsPartial)`
  - `prompts_list()`、`prompts_read(name?)`、`prompts_get_active()`、`prompts_set_active(name)`
  - `hud_toggle_hide()`、`hud_set_mouse_ignore(bool)`、`app_quit()`
  - `session_get()`、`session_new()`、`session_dump()`
  - `capture_analyze_stream({ text_prompt, custom_prompt, history })`
  - `transcribe_start({ model? })`、`transcribe_append(base64_pcm16)`、`transcribe_end()`、`transcribe_stop()`
- 事件（擬定 Tauri `app.emit_all` 對應）：
  - `text-input:show|toggle`、`hud:show`、`ask:clear|scroll|paginate`、`audio:toggle`、`session:changed`、`openai:config-updated`
  - `capture:analyze-stream:start|delta|done|error`、`transcribe:start|delta|done|error|closed`

**附錄：檔案與路徑**

- 設定檔：`~/.ghost-ai/config.json`
- 提示詞：`~/.ghost-ai/prompts/*.txt|md|prompt`
- 日誌：`~/.ghost-ai/logs/<sessionId>/<sessionId>.log|.json`

**附錄：與現有程式的關鍵對應**

- 允許模型白名單：`src/shared/openai-client.ts:16`
- Responses 串流事件解析：`src/shared/openai-client.ts:148` 起（reasoning/web_search/answer）
- 轉錄 WS 事件：`src/main/modules/realtime-transcribe.ts:56` 起
- 會話清除與重生：`src/main/main.ts:234` 起（清除）、`src/main/main.ts:371` 起（手動新建）
- 串流 IPC：`src/main/main.ts:484` 起（capture:analyze-stream）
- Click-through 控制：`src/main/preload.ts:245` + `src/main/main.ts:367`

**補充建議**

- 保留現有 UI/文案一致性，優先達成功能等量後再考慮 UI 重構。
- 提供「停用落地日誌」選項，預設依舊啟用（以符合現狀），並於首次啟動提示使用者。
- 在 Settings 增加「測試 API 連線」按鈕（已有 Test），Rust 版可在失敗時提示診斷資訊（DNS、TLS、Proxy）。
