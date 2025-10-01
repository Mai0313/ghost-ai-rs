Ghost AI – Developer Instructions

This document captures technical notes relevant to contributors.

Model list loading and config updates

- Renderer-side model selectors live in both `src/components/AskPanel.tsx` (next to the Ask input) and `src/components/Settings.tsx` (in the OpenAI Settings section).
- Both selectors are synchronized and use the same API calls to maintain consistency.
- Models are fetched through `window.ghostAI.listOpenAIModels()` which bridges to `ipcMain.handle('openai:list-models')` and ultimately `src/shared/openai-client.ts#listModels`.
- To avoid the selector getting stuck on "Loading models…" when the API key is missing or invalid, `listModels()` now returns a sensible default list even on errors.

IPC updates

- New broadcast event: `openai:config-updated`.
  - Emitted by main after both `openai:update-config` (persisted) and `openai:update-config-volatile` (in-memory) updates.
  - Main implementation lives in `src/main/main.ts`.
  - Preload exposes a convenience listener: `onOpenAIConfigUpdated(handler)` in `src/main/preload.ts`.
  - Renderer subscribes and refreshes the model list when config changes (see `src/components/AskPanel.tsx`).

// Removed: settings:updated broadcast and preload listener to keep surface minimal.

OpenAI client behavior

- `src/shared/openai-client.ts#listModels()`:
  - Tries to call `client.models.list()`.
  - Filters to an allowed ordering when available.
  - On any exception, returns the same default allowed order so the UI remains usable.

Renderer notes

- Both the Ask footer model selector and Settings model selector now refresh automatically when settings change, via `onOpenAIConfigUpdated`.
- Settings screen (`src/components/Settings.tsx`)
  - Loads once on mount; remains mounted while you toggle visibility, avoiding reload on each open.
  - Subscribes to `onOpenAIConfigUpdated` and (local save) updates to refresh state only on real updates.
  - Contains a model selector in the OpenAI Settings section that is synchronized with the Ask panel selector.
  - **Optimized config loading (2024-12-19)**:
    - Added debounced model refresh (800ms) when API key or base URL changes to prevent excessive API calls
    - Improved loading states with `loadingConfig` and `loadingModels` indicators
    - Enhanced error handling with proper try-catch blocks and console warnings
    - Optimized initial loading with `useCallback` hooks to prevent unnecessary re-renders
    - Added change detection to avoid redundant model updates when values haven't actually changed
    - Better cleanup of event listeners and debounce timeouts

Ask panel notes

- `src/components/AskPanel.tsx` is kept mounted; App toggles visibility via CSS to avoid reloading the model list on each open.
- Model list fetch happens on mount and when `onOpenAIConfigUpdated` fires (API key/baseURL/model changes), not on every toggle.
- **Screenshot attachment toggle (2024-12-19)**:
  - Added screenshot toggle button (📷) next to the input field in AskPanel footer
  - Toggle state is synchronized between AskPanel and Settings page via App component state management
  - Changes are immediately persisted to user settings via `updateUserSettings({ attachScreenshot })`
  - Both UI locations reflect the same state and update each other in real-time

Troubleshooting

- If requests fail after selecting a model, verify that your OpenAI API key has access to that model and that `baseURL` is correct.
- Use the Settings screen "Test" button to validate API connectivity.

### Packaging/runtime specifics

- Renderer asset base path
  - Vite must use a relative base when serving via `file://` in Electron.
  - Set `base: './'` in `vite.config.ts` so `dist/renderer/index.html` references assets relatively.
- Dev/prod detection in main
  - Use `app.isPackaged` instead of `process.env.NODE_ENV` to distinguish packaged runtime.
  - In production, load `path.join(__dirname, 'renderer', 'index.html')`; in dev, load `http://localhost:5173`.
- Symptom: packaged app installs but shows no UI
  - Likely due to incorrect `base` or dev detection. Apply the two fixes above, rebuild, then repackage.

## Ghost AI – Developer Notes

This document explains technical behaviors relevant to contributors.

### Prompt Injection Behavior

- The active default prompt is stored in `~/.ghost-ai/prompts/default.txt`.
- It is injected into the first turn of each session only.
- Subsequent turns for the same `sessionId` omit the default prompt to avoid duplicative instructions.

Implementation details:

- `src/main/modules/session-store.ts` exposes `hasEntries(sessionId: string): boolean`.
- `src/main/main.ts` checks `!sessionStore.hasEntries(requestSessionId)` to decide whether to read and pass the default prompt.
- The call site:

```ts
// Load active prompt content only for the first turn of the current session
const defaultPrompt = (() => {
  try {
    const isFirstTurn = !sessionStore.hasEntries(requestSessionId);
    if (!isFirstTurn) return "";
    return readPrompt() || "";
  } catch {
    return "";
  }
})();
if (defaultPrompt) initialPromptBySession.set(requestSessionId, defaultPrompt);
```

### Conversation Memory

- Plain‑text `Q:`/`A:` history is maintained in the main process per session using a Map: `conversationHistoryBySession: Map<string, string>`.
- The initial (first‑turn) default prompt used for a session is cached in `initialPromptBySession: Map<string, string>` and reused when rebuilding history for regeneration.
- On each new request, `combinedTextPrompt` is composed as:

```ts
// Use override if provided (regeneration), otherwise use accumulated history
const priorPlain =
  (typeof payload.history === "string" ? payload.history : null) ??
  conversationHistoryBySession.get(requestSessionId) ??
  "";

const initialPromptPrefix = initialPromptBySession.get(requestSessionId) ?? "";

const priorWithInitial =
  typeof payload.history === "string"
    ? `${initialPromptPrefix}${priorPlain || ""}`
    : priorPlain;

const combinedTextPrompt = priorWithInitial
  ? `Previous conversation (plain text):\n${priorWithInitial}\n\nNew question:\n${(payload.textPrompt ?? "").trim()}`
  : (payload.textPrompt ?? "").trim();
```

### Sessions

- A top-level `currentSessionId` is created on app start and when Ask is cleared or a new session is requested.
- Session entries are appended via `sessionStore.appendEntry` and persisted under `~/.ghost-ai/logs/<sessionId>/<sessionId>.json` for debugging/inspection.

### Streaming Only

- The app uses streaming analysis only (`capture:analyze-stream` IPC). Legacy non-streaming paths were removed.

# Ghost AI - Developer Instructions (Copilot)

This document describes important technical details for contributors. Update this whenever you change IPC channels, main/renderer contracts, or shared types.

## Architecture Overview

- Electron main process (`src/main/*`) handles:
  - Global hotkeys, tray, window lifecycle
  - OpenAI requests via `src/shared/openai-client.ts`
  - Screenshot capture via `src/main/modules/screenshot-manager.ts`
  - Settings persistence via `electron-store`
- Renderer (UI files under `src/*`) is a lightweight HUD and settings UI built with React
- Shared types and the OpenAI client are in `src/shared/*`

- Maintenance: Removed unused stubs `src/services/audio-processor.ts` and `src/services/image-processor.ts`.

## OpenAI Integration

- Wrapper class: `src/shared/openai-client.ts`
  - `initialize`, `updateConfig`, `validateConfig`, `listModels`
  - Streaming only: `completionStream(imageBuffer, textPrompt, customPrompt, requestId, onDelta)`
    - `onDelta` signature: `{ channel: 'answer'; delta?: string; text?: string; eventType: string }`
    - This path does not emit reasoning or tool events; only answer deltas/done.
  - Removed legacy helpers and Conversations helpers: `chatCompletion(...)`, `analyzeWithHistoryStream(...)`, `createConversation(...)`, `retrieveConversationItems(...)`.

### Reasoning stream (Responses API)

- `responseStream` forwards reasoning events in addition to answer deltas.
- Handled event types: `response.reasoning_summary_part.added`, `response.reasoning_summary_part.done`, `response.reasoning_summary_text.delta`, `response.reasoning_summary_text.done`.
- The delta callback now receives `{ channel: 'answer' | 'reasoning', eventType, delta?, text? }`.
  - Additionally supports `{ channel: 'web_search' }` with `eventType` in
    `response.web_search_call.in_progress|searching|completed`.
  - Main relays these via `capture:analyze-stream:delta` with the same fields.
  - Preload preserves the fields for renderer `onDelta`.
  - Renderer renders reasoning in a smaller, translucent area above the main answer and streams it live.
  - UI labels: a small "Reasoning" label appears above the reasoning block, and a small "Answer" label appears above the final answer markdown for clarity.

Notes:

- The project pins neither OpenAI SDK version nor strict types (uses `@ts-ignore` at call-sites as the SDK frequently changes). Keep the mapping minimal and guarded.
- Vision is implemented through `chat.completions.create` with a `content` array of `[text, image_url]`.
- To maximize model compatibility, we DO NOT set `temperature` or `max_tokens`/`max_completion_tokens` in API calls. Let the model defaults apply. If you re-introduce these, guard per-model support.
- For `gpt-5` only, we set `reasoning_effort: 'low'` on chat completion requests. Do not send `reasoning_effort` to non‑`gpt-5` models as many do not support it.

Type notes (OpenAIConfig):

- `src/shared/types.ts` defines `OpenAIConfig`.
- Field `maxTokens` is kept for configurability but not sent to the API by default.
- Type: `maxTokens?: number | null`.
- Default: `null` (see `src/main/main.ts#initializeOpenAI`). Treat `null` as "use model default / maximum".
- If you re-introduce explicit token limits at call sites, only include the parameter when `typeof maxTokens === 'number'`; when `null` or `undefined`, omit the param entirely to preserve model defaults.

## IPC Contracts

Preload exposes `window.ghostAI` via `src/main/preload.ts`:

```ts
interface GhostAPI {
  updateOpenAIConfig(cfg: Partial<OpenAIConfig>): Promise<boolean>;
  getOpenAIConfig(): Promise<OpenAIConfig | null>;
  validateOpenAIConfig(cfg: OpenAIConfig): Promise<boolean>;
  listOpenAIModels(): Promise<string[]>;
  analyzeCurrentScreenStream(
    textPrompt: string,
    customPrompt: string,
    handlers: {
      onStart?: (p: { requestId: string; sessionId: string }) => void;
      onDelta?: (p: {
        requestId: string;
        sessionId: string;
        channel?: "answer" | "reasoning";
        eventType?: string;
        delta?: string;
        text?: string;
      }) => void;
      onDone?: (p: AnalysisResult & { sessionId: string }) => void;
      onError?: (p: {
        requestId?: string;
        error: string;
        sessionId: string;
      }) => void;
    },
    history?: string | null, // optional plain-text Q/A prior-context override (used for regeneration)
  ): () => void; // unsubscribe
  // Realtime transcription (WS)
  startTranscription(options: { model?: string }): Promise<{ ok: boolean }>;
  appendTranscriptionAudio(base64Pcm16: string): void;
  endTranscription(): void;
  stopTranscription(): void;
  onTranscribeStart(
    handler: (p: { ok: boolean; sessionId: string }) => void,
  ): () => void;
  onTranscribeDelta(
    handler: (p: { delta: string; sessionId: string }) => void,
  ): () => void;
  onTranscribeDone(
    handler: (p: { content: string; sessionId: string }) => void,
  ): () => void;
  onTranscribeError(
    handler: (p: { error: string; sessionId: string }) => void,
  ): () => void;
  onTranscribeClosed(handler: () => void): () => void;
  getUserSettings(): Promise<any>;
  updateUserSettings(partial: Partial<any>): Promise<any>;
  onTextInputShow(handler: () => void): void;
  onTextInputToggle(handler: () => void): void; // toggles Ask panel open/closed
  onHUDShow(handler: () => void): void; // Emitted when HUD should become visible again (e.g., after toggling hide)
  toggleHide(): Promise<true>; // IPC to toggle main-process hidden state
  onAudioToggle(handler: () => void): void;
  // Toggle native click-through for the transparent overlay window
  setMouseIgnore(ignore: boolean): Promise<true>;
}
```

## Prompts Management (Read-only)

- Prompts are stored under `~/.ghost-ai/prompts`.
- The app NEVER writes or overwrites any prompt files. Selection is persisted by name in user settings (`defaultPrompt`).
- There is NO fallback to `default.txt`. An active prompt must be selected in Settings → Prompts; otherwise analyze is blocked with an explicit error.
- Main module: `src/main/modules/prompts-manager.ts`
  - `listPrompts()`, `readPrompt(name?)` (reads selected or explicit name)
  - `setDefaultPromptFrom(name)` and `setActivePromptName(name)` now only persist the selected name (no file writes)
  - `getActivePromptName()` returns the persisted name; `getDefaultPromptName()` is legacy
- IPC handlers in main: `prompts:list`, `prompts:read`, `prompts:set-default`, `prompts:get-default`, `prompts:get-active`, `prompts:set-active`
- Preload surface:

```ts
listPrompts(): Promise<{ prompts: string[]; defaultPrompt: string | null }>;
readPrompt(name?: string): Promise<string>;
setDefaultPrompt(name: string): Promise<string>; // persist name only
getDefaultPrompt(): Promise<string | null>;
getActivePromptName(): Promise<string | null>;
setActivePromptName(name: string): Promise<string>;
```

Renderer Settings UI lists available files and sets the active prompt; creating/editing/deleting prompt files is done outside the app.

Analyze flow: on first turn only, main reads the selected prompt by name and injects it. If none is selected, main emits an error and aborts the analyze request.

Main-side handlers in `src/main/main.ts` (streaming only):

- `ipcMain.on('capture:analyze-stream', ...)`
- `ipcMain.handle('hud:set-mouse-ignore', (evt, ignore) => mainWindow.setIgnoreMouseEvents(ignore, { forward: true }))`
- Hotkey handler emits `text-input:toggle` to renderer on `Cmd/Ctrl+Enter` to toggle Ask panel
  - To avoid overlap with `Cmd/Ctrl+Shift+Enter` (voice), the main process suppresses Ask toggles within ~400ms after a voice toggle.
- Emits to renderer:
  - `capture:analyze-stream:start` with `{ requestId, sessionId }` (sessionId required)
  - `capture:analyze-stream:delta` with `{ requestId, sessionId, channel?, eventType?, delta?, text? }`
  - `capture:analyze-stream:done` with final `AnalysisResult & { sessionId }` (sessionId required)
  - `capture:analyze-stream:error` with `{ requestId?, error, sessionId }` (sessionId required)

Streaming cancellation (interrupt):

- The main process now tracks per-renderer AbortControllers for analyze streams. On `Cmd/Ctrl+R` (clear), main aborts the active stream for that renderer, emits `ask:clear`, resets conversation history, generates a new `sessionId`, and broadcasts `session:changed`.
- Renderer must unsubscribe any active stream listeners upon `ask:clear` or `session:changed` and reset its UI state (`streaming=false`, clear `requestId`, `result`, input `text`, stop recording, etc.).
- The OpenAI client (`openai-client.ts`) accepts an optional `AbortSignal` in `completionStream(..., signal?)` which is passed through to the SDK call to cancel mid-stream.
- **Race condition fix**: To prevent interrupted conversations from being written to the wrong session log, each analysis records the `sessionId` at start (`requestSessionId`) and only writes to log if not aborted AND the session hasn't changed during analysis (`requestSessionId === currentSessionId`). This ensures interrupted conversations are not logged at all, rather than being written to the new session.

Conversation history (main-managed):

- The main process keeps a simple in-memory per-session string in `conversationHistoryBySession` formatted as:
  - `Q: <question>\nA: <answer>\n\n` appended per turn
- On each new request, main composes the prompt by prepending the session’s initial prompt (if any) when regeneration provides an override history.
- On `Cmd/Ctrl+R` (clear) or `session:new`, main clears `conversationHistoryBySession` and `initialPromptBySession` and generates a new `sessionId`.

Logging (new):

- Module: `src/main/modules/log-manager.ts`
  - `writeConversationLog(sessionId: string, content: string): Promise<string>`
  - Writes plain-text conversation to `~/.ghost-ai/logs/<sessionId>/<sessionId>.log`.
- Integration point: in `capture:analyze-stream` handler, after appending `Q:`/`A:` to the session’s history, call `await logManager.writeConversationLog(requestSessionId, conversationHistoryBySession.get(requestSessionId) ?? '')`.

HUD / Hide integration:

- `ipcMain.handle('hud:toggle-hide')` toggles visibility via `toggleHidden(mainWindow)`.
- When re-showing, main sends `hud:show` so the renderer can set `visible=true` and re-enable input.
- Renderer’s Hide button calls `window.ghostAI.toggleHide()` instead of only local `visible=false`.

Ensure to unsubscribe listeners on `done` or `error` from the preload wrapper.

### Realtime Transcription

- Main module: `src/main/modules/realtime-transcribe.ts`
  - Opens `wss://api.openai.com/v1/realtime?intent=transcription`
  - Sends `transcription_session.update` with `input_audio_format: "pcm16"`, `turn_detection: { type: "server_vad", threshold: 0.5, silence_duration_ms: 350, prefix_padding_ms: 150 }`, and `input_audio_transcription.model: "gpt-4o-mini-transcribe"`
  - Language hint: `input_audio_transcription.language` is set from user settings (`en` or `zh`), default `en`, to avoid garbled Chinese output
  - Accepts `input_audio_buffer.append` events with base64 PCM16 mono @ 24kHz
  - Emits to renderer:
    - `transcribe:start`, `transcribe:delta` (streaming deltas), `transcribe:done` (sentence completed), `transcribe:error`, `transcribe:closed`
  - IPC channels: `transcribe:start` (handle), `transcribe:append`, `transcribe:end`, `transcribe:stop`
  - Logs: WS connect/open/close/error, message type hints, appended bytes per chunk, input buffer end

- Renderer (`src/renderer/main.tsx`):
  - Captures microphone via `getUserMedia({ audio: { echoCancellation:false, noiseSuppression:false, autoGainControl:false } })`
  - Attempts to capture system audio via `getDisplayMedia({ audio:true })` and discards video tracks
  - Mixes sources in `AudioContext`, downsamples to 24 kHz mono, converts to 16‑bit PCM, and performs client-side batching: flush every ~220 ms or at ~32 KB of PCM16 bytes. This reduces WS overhead and improves transcription stability with minimal latency.
  - Sends batches via `appendTranscriptionAudio` and renders transcript deltas via the same unified render sink used by analyze streaming: `appendLive(delta)` and `finalizeLive({ content })`
  - On each transcription completion, the renderer appends a pair into its local `history`: `{ role: 'user', content: <transcript> }` and `{ role: 'assistant', content: <transcript> }`, making each transcript segment a paginated page. You can Regenerate on such a page to analyze that transcript; the assistant content for that page is replaced in place while the user transcript is preserved.
  - Pause/Resume support: renderer gates both the elapsed timer and audio processing/delta application when paused (no IPC changes required)
  - On stop: sends `endTranscription` and `stopTranscription`, closes audio graph, stops tracks, and clears timers
  - Logs: microphone/system audio permission results and audio processing errors

## Renderer Flow (Ask UI)

- Component: `src/renderer/main.tsx` maintains `text`, `result`, `busy`, `streaming` states.
  - On Enter key: calls `ghostAI.analyzeCurrentScreenStream(...)`.
  - Appends deltas to `result` in real time.
  - Streamed content is rendered as Markdown using a read-only BlockNote editor. The renderer derives a `displayMarkdown` value showing either the live streamed content or the selected historical page. Historical pages include both assistant answers and transcript pages (transcripts are inserted as `{user, assistant}` where assistant initially mirrors the transcript). On each `displayMarkdown` change, the renderer converts Markdown with `editor.tryParseMarkdownToBlocks(displayMarkdown)` and replaces content via `editor.replaceBlocks(editor.document, blocks)`.
  - Code blocks are rendered without syntax highlighting. We removed the Shiki-based highlighter to simplify dependencies.
  - Shows the streamed response bubble ABOVE the input field.
  - Disables the input while streaming.
  - No non-streaming fallback; errors are surfaced inline and user can retry immediately.
  - Error handling: when an error occurs (from streaming or fallback), the UI writes an inline message to the same bubble in the form `Error: <message>` and re-enables input so the user can retry immediately.
  - Clear conversation: `Cmd/Ctrl+R` clears renderer `history` + `result` and also clears main-process conversation context.
  - Renderer `history` is for UI navigation only; model memory/context is managed in main.
  - Regeneration flow: after an answer finishes, the Ask footer shows a `↻ Regenerate` button. Clicking it:
    - Identifies the page to regenerate (current page if paged; otherwise the latest completed page)
    - Extracts that page's original user message from `history`
    - Builds a plain-text `Q:`/`A:` history string from all pairs before that page
    - Calls `analyzeCurrentScreenStream(userMessage, customPrompt, handlers, priorPlain)` where `priorPlain` is the string above
    - On completion, replaces the assistant content for that page in-place (does not append a new page)
  - Ask input placeholder: shows `Thinking…` while busy/streaming; otherwise `Type your question…`.

### Overlay click-through policy

- The main window is a full-screen, transparent overlay (no frame, no shadow) positioned over the primary display.
- By default it is click-through: `setIgnoreMouseEvents(true, { forward: true })` is enabled so underlying apps remain interactive.
- The renderer toggles click-through dynamically via `window.ghostAI.setMouseIgnore(false/true)` when the pointer is over the HUD bar or bubbles, and during drag.
- The root container uses `pointer-events: none` while interactive elements use `pointer-events: auto`, ensuring only visible UI captures input. This also allows dragging the HUD to the very top/bottom edges without invisible blockers.

### Validation messages

- When testing API settings via the "Test" button in Settings, the validation result is shown as a styled notification next to the button.
- Validation uses IconCheckCircle and IconXCircle from Icons.tsx to display success/failure states with appropriate colors.
- The notification appears in a styled container with a background color, border, and icon to improve visibility and user experience.
- The validation message style is defined inline in Settings.tsx in the JSX return section.

## UI Theming and Styles

- Centralized theme and styles live under `src/styles/`.
  - `theme.ts`: exports `theme` and `makeTheme(opacity?: number)`; the single `opacity` value synchronizes text/background transparency across the UI.
    - Change default opacity at the export site: `export const theme = makeTheme(0.85)`.
    - Edit base colors in `palette` (RGB tuples) to adjust text/background/primary/danger etc.
  - `styles.ts`: component styles (bar, settings card, ask card, buttons) consume `theme.color.*()` so most customization is done via `theme.ts`.
  - Prefer per-component tweaks via multipliers (e.g., `theme.color.panelBg(0.9)`) rather than hard-coded rgba.
  - Markdown viewer encapsulated in `src/components/MarkdownViewer.tsx`.
    - Assets are imported in `src/main.tsx` (global CSS imports).
    - Use `<MarkdownViewer markdown={displayMarkdown} />` to render.
  - Scrollbar styling for the AI answer panel lives in `src/styles/blocknote-custom.css` under `.bn-markdown-viewer` with both WebKit (::-webkit-scrollbar) and Firefox (scrollbar-width/color) rules to match the dark panel aesthetics.

## Screenshot Capture

- Captured via `screenshot-desktop` with PNG buffers.
- Uses `hideAllWindowsDuring(...)` to avoid self-capture.
- Main checks `loadUserSettings().attachScreenshot` before capturing. If false, capture is skipped and `openAIClient.responseStream(undefined, ...)` is called.

## Settings

- Persisted with `electron-store` and `safeStorage` (encrypts the OpenAI config blob).
- `Settings.tsx` reads and updates config via `window.ghostAI`.
- New: `settings:get`/`settings:update` now persist user preferences, including `transcribeLanguage: 'en' | 'zh'` (default `'en'`) and `attachScreenshot?: boolean` (default `true`). The latter controls whether each Ask includes a screenshot. When disabled, the main process skips capture and only sends the text question; `responseStream` accepts an optional `imageBuffer`.

## Coding Guidelines

- Keep all UI logic in renderer, system/IO in main.
- Prefer strong typing but be pragmatic with SDK surface changes.
- On streaming, always clean up listeners to avoid leaks.
- When modifying IPC channels, update:
  - `src/main/main.ts`
  - `src/main/preload.ts`
  - `src/global.d.ts`
  - This document.

# Ghost AI — Copilot Instructions (需求與計畫整合)

本文件整合 `.kiro/specs/ghost-ai` 中的需求與實作計畫，作為 Copilot 與協作開發的工作指南。專案為純 Electron + TypeScript 跨平台桌面應用，直接整合 OpenAI API，核心能力包含：

- 文字輸入 + 螢幕截圖分析
- 語音錄音（並預留 WebRTC 實時對話）
- 隱藏式操作界面與全域熱鍵

**重要：本專案採用純前端 TypeScript + Electron 架構，完全無後端服務。所有功能都在 Electron 應用程式內實現，包括 OpenAI API 設定都可在前端界面中配置。**

更新時請與 `.kiro/specs/ghost-ai/{requirements.md,tasks.md,design.md}` 保持一致。

---

## 一、功能需求與驗收標準（Functional Requirements）

以下匯總自 `requirements.md`，保留 User Story 與 Acceptance Criteria，作為 DoD（Definition of Done）的依據。

### R1：文字輸入與螢幕截圖分析

- User Story：作為用戶，我希望透過熱鍵快速呼叫輸入框，輸入問題並自動截圖，讓 AI 助手分析當前螢幕內容並回答。
- Acceptance Criteria：
  1. 當用戶按下設定熱鍵，系統會立即顯示文字輸入框。
  2. 當用戶送出文字，系統會自動進行螢幕截圖。
  3. 截圖完成後，將用戶問題、螢幕截圖與自定義提示詞一併送至 Chat Completion API。
  4. 截圖失敗時，顯示錯誤並允許重試。
  5. API 回應後，在適當界面顯示 AI 回答。

### R2：語音錄音與實時對話（預留）

- User Story：作為用戶，我希望透過按鈕或熱鍵開始錄音，並預留未來 WebRTC 實時對話能力。
- Acceptance Criteria：
  1. 按下錄音按鈕或熱鍵，立即開始錄音。
  2. 錄音中有明確狀態指示。
  3. 再次按下時停止錄音並保存音訊。
  4. 麥克風權限被拒時，顯示權限請求指引。
  5. 錄音完成後，預留 WebRTC 實時對話介面。
  6. 首次使用時，提供設定界面讓用戶輸入OpenAI API金鑰和基礎URL。

### R3：隱藏式操作界面

- User Story：作為用戶，我希望可隱藏操作界面，避免在螢幕分享或截圖時被看到。
- Acceptance Criteria：
  1. 按下隱藏熱鍵，立即隱藏所有可見界面。
  2. 隱藏狀態下，不出現在螢幕截圖或分享中。
  3. 再次按下熱鍵可恢復顯示。
  4. 系統重啟後記住上次隱藏狀態。
  5. 隱藏狀態下，全域熱鍵仍可運作。

### R4：全域熱鍵系統

- User Story：作為用戶，我希望能自定義所有功能熱鍵，並在任何應用中可用。
- Acceptance Criteria：
  1. 設定熱鍵時檢查衝突並警告。
  2. 任何應用中按下熱鍵，能正確觸發功能。
  3. 系統啟動時自動註冊所有熱鍵。
  4. 註冊失敗時通知用戶並提供替代方案。
  5. 修改設定後，立即更新註冊。

### R5：自定義提示詞與API設定管理

- User Story：作為用戶，我希望能設定與管理自定義 AI 提示詞，以及配置OpenAI API連接設定。
- Acceptance Criteria：
  1. 提供簡潔的提示詞編輯介面。
  2. 保存時驗證格式並儲存至本地。
  3. 發送請求時自動包含自定義提示詞。
  4. 過長時給出警告與優化建議。
  5. 重置時恢復預設模板。
  6. 開啟設定時提供API金鑰、基礎URL、模型選擇等配置選項。
  7. 保存API設定時加密儲存敏感資訊並驗證連接有效性。

### R6：AI 分析與回應處理

- User Story：作為用戶，我希望 AI 能準確分析輸入與螢幕內容，提供有用回應。
- Acceptance Criteria：
  1. 收到截圖與提示詞後，使用前端設定的API金鑰直接透過 OpenAI API 處理圖片分析。
  2. 回傳分析結果後，在界面顯示。
  3. API 失敗時顯示錯誤並可重試。
  4. 分析完成後，支援複製結果或進行後續操作。
  5. 處理音訊時，支援語音轉文字並整合分析流程。

### R7：系統整合與穩定性

- User Story：作為用戶，我希望系統穩定運行並與作業系統良好整合。
- Acceptance Criteria：
  1. 啟動後在系統托盤顯示圖示並提供基本控制。
  2. 發生錯誤時記錄日誌並嘗試自動恢復。
  3. 登出或關機時正確清理資源與保存設定。
  4. 崩潰後重啟可恢復到上次工作狀態。
  5. 系統更新後保留用戶自定義設定。
  6. 應用程式可打包到 Windows/macOS/Linux。

### R8：隱私保護與安全性

- User Story：作為用戶，我希望我的資料被妥善保護，系統運行不被他人輕易偵測。
- Acceptance Criteria：
  1. 運行時採用隱蔽的程序名稱與視窗標題。
  2. 截圖與音訊僅在記憶體中處理，不落地。
  3. 與 API 通訊採用加密連線並清理網路日誌。
  4. 偵測到監控軟體時警告用戶潛在風險。
  5. 關閉應用時清理所有暫存與記憶體痕跡。
  6. 儲存API金鑰時使用本地加密儲存，不依賴外部服務。
  7. 設定敏感資訊時提供安全性警告和最佳實踐建議。

---

## 二、非功能性需求（NFR，節選自設計）

出自 `design.md` 的約束（請據此實作與檢查）：

- 錯誤處理策略：
  - 熱鍵註冊失敗：替代組合、用戶提示、不中斷運行
  - 截圖失敗：最多 3 次重試、降級至視窗截圖、友善錯誤
  - 網路連線：自動重試、離線提示、請求快取
  - OpenAI API：指數退避、配額/用量監控、必要時降級
  - 跨平台錯誤：平台特定降級、可用性檢測、對應訊息
  - 資源不足：記憶體監控/清理、優雅降級、警告

- 隱私與安全：
  - 僅在記憶體中處理圖片與音訊，安全清理緩衝區
  - HTTPS、可考慮憑證固定與請求標頭混淆
  - 程序隱蔽（隨機名稱、隱藏標題、最小足跡）
  - 暫存資料自動清理與加密；API 金鑰以環境變數配置並限制存取

- 效能最佳化：
  - 啟動最佳化（延遲載入、預載資源、背景初始化、分階段啟動）
  - 記憶體最佳化（及時釋放、生命週期管理、GC 最佳化、監控）
  - 跨平台最佳化與原生 API 利用
  - API 通訊最佳化（連線池、快取、壓縮、批次處理）

---

## 三、實作計畫（Implementation Plan）

以下整合自 `tasks.md`，為開發優先序與工作拆解，含路徑約定與需求對應：

- [ ] 1. 專案骨架與核心介面
  - 建立目錄：`src/main/`, `src/renderer/`, `src/shared/`, `src/services/`
  - 定義三大功能的 TS 介面；設定 `package.json`/`tsconfig.json`
  - 安裝 OpenAI SDK 與必要套件（electron, typescript, react, electron-builder…）
  - 以 UI 設定方式管理 `OPENAI_BASE_URL`, `OPENAI_API_KEY`（不強制使用環境變數）
  - 打包設定（Win/macOS/Linux）
  - 對應需求：R1, R2, R3, R4, R7.6

### UI 指南（Renderer 端）

- 新的 UI 採用「頂部置中控制列（HUD）為主，面板按需顯示」設計：
  - 應用啟動後預設為隱藏（`visible=false`），不顯示任何面板。
  - 透過熱鍵或托盤選單觸發 `text-input:show` 時，僅顯示頂部 HUD；此時不自動打開任何泡泡或設定面板（避免畫面雜訊）。
  - HUD 包含 `Listen`、`Ask`、`Hide`、`Settings` 四個按鈕；`Ask`/`Settings` 皆為「切換顯示/隱藏」的行為（再次點擊可收合）。
  - 錄音中按鈕會切換成紅色並顯示 `mm:ss` 計時。
  - 需要輸入問題或查看設定時，才在 HUD 下方顯示對話泡泡/設定卡片。
  - 樣式使用內聯樣式，深色玻璃質感（半透明深色背景 + 邊框 + 陰影）。
  - 主要檔案：`src/renderer/main.tsx`、`src/renderer/components/Settings.tsx`、`src/renderer/components/Icons.tsx`。

- 重要互動流程：
  - `window.ghostAI.onTextInputShow` 顯示 HUD 並直接打開 `Ask`，焦點移到單行輸入。
  - `Ask` 只有單行輸入框；Enter 送出（IME 組字時不送出）。
  - `Settings` 面板沿用 IPC：`openai:update-config`、`openai:get-config`、`openai:validate-config`，並新增 `settings:get`、`settings:update` 管理使用者偏好（例如預設 custom prompt）。

- 型別補充：在 `tsconfig.json` 加入 `"types": ["vite/client"]` 以支援 `import.meta.env`。

### 視窗與隱形屬性（Main 端）

- `BrowserWindow` 採用絕對隱形覆蓋層設定，避免出現類似 Chrome 的標準視窗：
  - `show: false`（啟動不顯示）
  - `frame: false`
  - `transparent: true`
  - `backgroundColor: '#00000000'`
  - `resizable: false`, `fullscreenable: false`, `hasShadow: false`, `skipTaskbar: true`, `alwaysOnTop: true`
  - 關閉選單列：`mainWindow.setMenuBarVisibility(false)`
- 只有在使用者透過熱鍵或托盤操作時才 `mainWindow.show()`，並由 Renderer 端控制 HUD/面板的顯示。
- 截圖流程透過 `hideAllWindowsDuring` 暫時隱藏所有視窗，避免干擾截圖與留下雜訊。

### 熱鍵（固定，全部全局）

-- 所有熱鍵均採用 Main 端 `globalShortcut` 註冊，Renderer 不再自行處理鍵盤事件：

- `Cmd/Ctrl+Enter`：顯示 HUD 並打開 Ask（且自動聚焦）
- `Cmd/Ctrl+Shift+Enter`：切換語音錄音（開始/停止）。目前僅切換 UI 與錄音計時，不進行轉錄。
- `Cmd/Ctrl+\\`：隱藏/顯示 HUD（切換）
- `Cmd/Ctrl+R`：清除 Ask 對話與結果，並重置語音狀態（停止錄音並丟棄暫存音訊）

### 會話（Top-level Session）

- 應用程式在啟動時會生成一個頂層 `sessionId`（UUID）。
- 觸發 `Cmd/Ctrl+R` 清除時，主程序會：
  - 清空 `conversationHistoryText`
  - 生成新的 `sessionId`
  - 廣播 `session:changed` 給 Renderer，並嘗試停止目前的轉錄連線
- IPC：
  - `session:get` → `{ sessionId }`
  - `session:new` → 建立新 session 並回傳 `{ sessionId }`
  - 事件：`session:changed` → `{ sessionId }`
- 所有截圖分析串流事件都會攜帶 `sessionId`（必填）：
  - `capture:analyze-stream:start|delta|error`
  - `capture:analyze-stream:done` 的 `AnalysisResult` 亦攜帶 `sessionId`
- 即時轉錄事件也會攜帶 `sessionId`（必填）：
  - `transcribe:start|delta|done|error`
- 紀錄檔：`writeConversationLog(id, content)` 目前以 `sessionId` 為檔名並存放於資料夾 `~/.ghost-ai/logs/<sessionId>/<sessionId>.log`。
  同時會在每次更新時輸出 `~/.ghost-ai/logs/<sessionId>/<sessionId>.json`，包含：
  - `entries[]`: `{ index, requestId, log_path, text_input, ai_output }`
  - `nextIndex`: 下一個索引

### Session Store (global list-dict)

- 模組：`src/main/modules/session-store.ts`
- 功能：以 `sessionId` 為 key，維護每次送出（Ask + 圖片 + 可能的 Listen 轉錄快照）的結構化清單。
  - `appendEntry(sessionId, { requestId, text_input, ai_output })`: 新增一筆 entry，並自動賦予流水號 `index`
  - `updateSessionLogPath(sessionId, logPath)`: 更新該 session 的 `log_path`
  - 不緩存或落地任何 Listen 逐字稿；轉錄內容僅用於即時 UI 顯示
  - `getSessionsData()`: 輸出 `[{ sessionId: [entries...] }, ...]` 形態供除錯
  - `toJSON()`: 輸出 `{ [sessionId]: { entries, nextIndex, log_path } }` 形態，提供持久化用
- 整合點：
  - 影像分析完成（`capture:analyze-stream:done`）後：
    1. 寫入 `~/.ghost-ai/logs/<sessionId>/<sessionId>.log`
    2. 擷取並清空轉錄快照，`appendEntry(...)`
    3. `updateSessionLogPath(...)`
    4. 將 `toJSON()[sessionId]` 寫入 `~/.ghost-ai/logs/<sessionId>/<sessionId>.json`
  - 清除（Ctrl/Cmd+R）或 `session:new`：清掉 store 並重置 `sessionId`
- IPC/Preload：
  - `ipcMain.handle('session:dump')`、`window.ghostAI.dumpSession()` 可即時讀取當前 list-dict

範例（`session:dump` 的輸出形態）：

```
[
  {
    "d1a4c8c6-8f3c-4f8e-9fd0-2e7f5b6c5a12": [
      {
        "index": 0,
        "requestId": "f0a3e9f8-1c32-4e1b-9e7f-91a2b4c3d5e6",
        "log_path": "C:\\Users\\Wei\\.ghost-ai\\logs\\d1a4c8c6-8f3c-4f8e-9fd0-2e7f5b6c5a12\\d1a4c8c6-8f3c-4f8e-9fd0-2e7f5b6c5a12.log",
        "text_input": "使用者輸入的內容（可為空）",
        "ai_output": "模型的回應（完整內容）"
      }
    ]
  }
]
```

### 影像分析串流（空白輸入處理與提示詞角色）

- 若 Renderer 傳入的 `textPrompt` 為空字串，主程序仍會送出請求；在 SDK 呼叫前，會以預設文字 `'Please analyze this screenshot.'` 進行補齊，確保串流能正常返回。
- 啟用中的自定義提示詞（從 `prompts-manager` 讀取）會以 `system` 角色附加，提供全域指示；避免使用 `assistant` 角色以免影響模型行為。

– `Cmd/Ctrl+Up`：向上捲動內容
– `Cmd/Ctrl+Down`：向下捲動內容
– `Cmd/Ctrl+Shift+Up`：切換到上一頁（上一則助理回答）
– `Cmd/Ctrl+Shift+Down`：切換到下一頁（下一則助理回答；在最後一頁時返回到「最新/直播」視圖）

- 更新：Renderer 改以 `ask:paginate` 事件（由 Preload 暴露 `window.ghostAI.onAskPaginate(handler)`）處理分頁切換；`ask:scroll` 恢復為內容滾動。

### 選單快捷鍵（避免與 Renderer 清除快捷鍵衝突）

- `View` 選單的 Reload 改用 `F5` 作為加速鍵，避免覆蓋全局 `Cmd/Ctrl+R`（清除 Ask 對話）。

- [ ] 2. 全域熱鍵系統
  - [ ] 2.1 `./src/main/hotkey-manager.ts`：註冊文字/錄音/隱藏熱鍵，跨平台與衝突檢測；型別放 `./src/shared/types.ts`
    - 對應：R4 全部條目
  - [ ] 2.2 熱鍵設定管理：本地儲存、自訂 UI、驗證、動態更新、預設與重置
    - 對應：R4.1, R4.2, R4.4, R4.5

- [ ] 3. 文字輸入與螢幕截圖分析
  - [ ] 3.1 `./src/renderer/components/TextInputComponent`：熱鍵觸發顯示、輸入驗證與送出、統一樣式
    - 對應：R1.1–R1.3
  - [ ] 3.2 `./src/main/screenshot-manager.ts`：跨平台截圖、記憶體處理、重試與錯誤處理
    - 對應：R1.2, R1.4
  - [ ] 3.3 整合：組合用戶問題/截圖/自定義提示詞，格式化 Chat Completion 請求，顯示結果與重試
    - 對應：R1.3, R1.5

- [ ] 4. 語音錄音
  - [ ] 4.1 `./src/main/audio-manager.ts`：權限請求、設備檢測、開始/停止、記憶體處理與格式轉換
    - 對應：R2.1, R2.2, R2.4
  - [ ] 4.2 錄音狀態指示 UI：時長與音量指示、通知與確認
    - 對應：R2.2, R2.3
  - [ ] 4.3 WebRTC 預留：架構與介面、數據模型、音訊流處理基礎、設定選項
    - 對應：R2.5

- [ ] 5. 隱藏式操作界面
  - [ ] 5.1 `./src/main/hide-manager.ts`：完全隱藏/恢復、截圖/分享時隱藏、狀態持久化
    - 對應：R3.1, R3.2, R3.4
  - [ ] 5.2 隱藏狀態管理：記憶與恢復、重啟恢復、隱藏模式下熱鍵保持、用戶通知
    - 對應：R3.3, R3.4, R3.5

- [ ] 6. 自定義提示詞與API設定管理
  - [ ] 6.1 本地儲存與管理、編輯 UI 與驗證、模板與預設、匯入匯出
    - 對應：R5.1, R5.2
  - [ ] 6.2 API設定管理界面：OpenAI API設定的前端配置、金鑰/URL/模型設定、驗證與測試、加密儲存、首次使用引導
    - 對應：R5.6, R5.7, R6.6
  - [ ] 6.3 與分析流程整合：組合邏輯、長度驗證與優化建議、預覽、動態替換與變數
    - 對應：R5.3, R5.4

- [ ] 7. OpenAI 直接整合
  - [ ] 7.1 `./src/shared/openai-client.ts`：Chat Completion 呼叫、前端動態配置、金鑰管理、安全與重試、模型清單獲取
    - 對應：R6.1, R6.2, R6.6, R8.3
  - [ ] 7.2 Vision 圖片分析整合：文字+圖片綜合分析、base64/格式處理、結果格式化與顯示（Main Process）
    - 對應：R6.1, R6.2, R6.4
  - [ ] 7.3 Whisper 語音轉文字：格式驗證與轉換、base64 處理、結果顯示
    - 對應：R6.5

- [ ] 8. 系統整合與穩定性
  - [ ] 8.1 托盤與狀態：托盤圖示/控制、狀態監控、啟動初始化、優雅關閉與清理
    - 對應：R7.1, R7.3, R7.5
  - [ ] 8.2 錯誤處理與恢復：全域錯誤與日誌、崩潰自動恢復、設定備份/復原、錯誤報告
    - 對應：R7.2, R7.4

- [ ] 9. 隱私保護與安全性
  - [ ] 9.1 記憶體安全：安全清理、圖片/音訊緩衝自動清理、敏感資料加密、防止交換至虛擬記憶體
    - 對應：R8.2, R8.5
  - [ ] 9.2 程序隱蔽與隱私模式：隨機程序名、隱藏視窗標題、監控軟體偵測警告、流量加密與標頭混淆、完整隱私模式
    - 對應：R8.1, R8.3, R8.4, R8.5

- [ ] 10. 測試與品質
  - [ ] 10.1 單元測試：Jest + React Testing Library（Renderer）、Jest（Main）、Mock Electron/OS/OpenAI
  - [ ] 10.2 整合與 E2E：IPC 整合、三大核心流程、隱私與安全驗證、跨平台相容性

- [ ] 11. 效能與發佈
  - [ ] 11.1 效能最佳化：啟動/記憶體最佳化、API 呼叫與快取、跨平台效能監控、資源動態調整
  - [ ] 11.2 打包與分發：跨平台打包、自動化建置/分發、簽名與驗證、安裝與更新機制

---

## 四、測試計畫（節選自設計 Testing Strategy）

- 單元測試：
  - Renderer：React Testing Library
  - Main：服務模組（Mock Electron/OS）、Mock OpenAI 回應
- 整合測試：
  - Main ⇄ Renderer IPC、熱鍵、截圖、OpenAI 整合
- 端到端（Spectron 或 Playwright for Electron）：
  - 完整使用者流程、跨平台相容性（Win/macOS/Linux）
- 效能測試：
  - 啟動時間、記憶體洩漏、API 回應時間、跨平台比較
- 安全性測試：
  - 隱私（無落地、記憶體清理、流量分析）、熱鍵安全（鍵盤記錄偵測、低層級鉤子驗證、系統日誌檢查）

---

## 五、關鍵設定與約定

- 前端設定：OpenAI API金鑰、基礎URL、模型選擇等都可在應用程式設定界面中配置
- 主要檔案與路徑（計畫中已標註）：
  - `./src/main/hotkey-manager.ts`
  - `./src/main/screenshot-manager.ts`
  - `./src/main/audio-manager.ts`
  - `./src/main/hide-manager.ts`
  - `./src/shared/openai-client.ts`
  - `./src/shared/types.ts`
  - `./src/renderer/components/TextInputComponent`

- 建置輸出路徑：
  - 產出至 `dist/`（主行程與預載），Renderer 產出至 `dist/renderer/`

---

## 技術架構與開發指令

- **純前端 Electron + TypeScript 架構**：
  - 單一桌面應用程式，完全無後端服務
  - `OPENAI_API_KEY`、`OPENAI_BASE_URL`、模型選擇等都透過前端 UI 設定
  - 模型清單使用 OpenAI SDK `client.models.list()` 動態取得
  - 所有 AI 處理都在 Main Process 中直接呼叫 OpenAI API
  - API設定使用 Electron safeStorage 進行本地加密儲存
  - 封裝使用 `electron-builder`：Windows（NSIS .exe）、macOS（.dmg）、Linux（AppImage/deb）

### Packaging scripts

- `npm run dist` — Build + package for current platform using `electron-builder`.
- `npm run dist:win` — Build + package Windows (`--win --publish never`). Outputs NSIS installer under `release/`.
- `npm run dist:win:portable` — Build + package Windows Portable (`--win portable --publish never`). Outputs a single portable `.exe` under `release/` (no installer / no elevation / no auto‑update).
- `npm run dist:mac` — Build + package macOS (`--mac --publish never`). Must run on macOS for DMG and signing.
- `npm run dist:mac:zip` — Build + package macOS ZIP (`--mac zip --publish never`).
- `npm run dist:linux` — Build + package Linux (`--linux --publish never`). Recommend running on Linux (or WSL2) for AppImage/deb.
- `npm run dist:linux:portable` — Build + package Linux AppImage only (`--linux AppImage --publish never`).

Packaging targets are defined in `electron-builder.json`:

```json
{
  "mac": { "target": ["dmg"], "category": "public.app-category.productivity" },
  "win": { "target": ["nsis", "portable"], "icon": "ghost.ico" },
  "mac": {
    "target": ["dmg", "zip"],
    "category": "public.app-category.productivity"
  },
  "linux": { "target": ["AppImage", "deb"] }
}
```

### Packaging assets

- Windows icon is set to `ghost.ico` at the repository root via `electron-builder.json` (`win.icon`).
- The icon file is also included at runtime through `extraResources` to allow the Tray and BrowserWindow to load it using `process.resourcesPath`.
- Runtime loading path helper is implemented in `src/main/main.ts` (`resolveAssetPath('ghost.ico')`), used for `BrowserWindow` `icon` and `Tray` icon.

- **開發指令**：
  - `npm install` - 安裝依賴
  - `npm run dev` - 啟動開發模式（首次使用建議進入設定面板設定 OpenAI API）
  - `npm run build` - 建置並打包應用程式
  - `npm run test` - 執行測試

---

本文件作為 Copilot 的主要參照，協助在開發過程中自動對齊需求（R1–R8）與實作計畫（1–11）。提交 PR/Commit 前，請對照本文件之驗收標準與對應開發項目，確保一致。
