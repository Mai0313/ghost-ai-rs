use std::collections::VecDeque;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{self, Color32, Margin, RichText, TextureOptions};
use image::imageops::FilterType;
use image::GenericImageView;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::audio::AudioRecorder;
use crate::capture::{capture_screen, CaptureResult};
use crate::config::{self, AppConfig};
use crate::hotkeys::{self, HotkeyAction, HotkeyHandle};
use crate::openai::{AnalyzeRequest, AnalyzeResponse, OpenAIClient};
use crate::session::{ConversationEntry, ConversationRole, SessionManager};

pub struct GhostApp {
    runtime: Handle,
    openai: Arc<OpenAIClient>,
    config: AppConfig,
    session: Arc<SessionManager>,
    conversation: Vec<ConversationEntry>,
    ask_input: String,
    ask_panel_open: bool,
    attach: Option<ScreenshotAttachment>,
    attach_texture: Option<ScreenshotTexture>,
    events_rx: UnboundedReceiver<AppEvent>,
    events_tx: UnboundedSender<AppEvent>,
    request_tx: UnboundedSender<AnalyzeRequest>,
    hotkey_rx: UnboundedReceiver<HotkeyAction>,
    _hotkey_handle: Option<HotkeyHandle>,
    status: Option<StatusMessage>,
    settings_open: bool,
    is_hidden: bool,
    active_request: Option<Uuid>,
    auto_scroll: bool,
    audio_recorder: Option<AudioRecorder>,
    is_recording: bool,
    transcription_pending: bool,
    last_transcription: Option<String>,
}

impl GhostApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Handle) -> Self {
        let config = config::load_or_default().unwrap_or_default();
        let logs_dir = config::logs_dir().unwrap_or_else(|err| {
            log::warn!("failed to prepare logs directory: {err}");
            std::env::temp_dir().join("ghost-ai-logs")
        });
        if !logs_dir.exists() {
            if let Err(err) = fs::create_dir_all(&logs_dir) {
                log::warn!(
                    "failed to create logs directory {}: {err}",
                    logs_dir.display()
                );
            }
        }

        let session = Arc::new(SessionManager::new(logs_dir).unwrap_or_else(|err| {
            log::error!("failed to initialize session manager: {err}");
            SessionManager::new(std::env::temp_dir()).expect("session manager fallback")
        }));

        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let openai = Arc::new(OpenAIClient::new().unwrap_or_else(|err| {
            log::error!("failed to construct OpenAI client: {err}");
            OpenAIClient::new().expect("OpenAI client")
        }));

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        spawn_analyze_worker(&runtime, Arc::clone(&openai), request_rx, events_tx.clone());

        let (hotkey_tx, hotkey_rx) = mpsc::unbounded_channel();
        let hotkey_bindings = hotkeys::bindings_from_config(&config.hotkeys);
        let hotkey_handle = if hotkey_bindings.is_empty() {
            None
        } else {
            match HotkeyHandle::spawn(hotkey_bindings, hotkey_tx) {
                Ok(handle) => Some(handle),
                Err(err) => {
                    log::error!("failed to start global hotkey listener: {err}");
                    None
                }
            }
        };

        Self {
            runtime,
            openai,
            config,
            session,
            conversation: Vec::new(),
            ask_input: String::new(),
            ask_panel_open: true,
            attach: None,
            attach_texture: None,
            events_rx,
            events_tx,
            request_tx,
            hotkey_rx,
            _hotkey_handle: hotkey_handle,
            status: None,
            settings_open: false,
            is_hidden: false,
            active_request: None,
            auto_scroll: true,
            audio_recorder: None,
            is_recording: false,
            transcription_pending: false,
            last_transcription: None,
        }
    }

    fn process_background_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                AppEvent::AnalysisStarted { request_id } => {
                    self.active_request = Some(request_id);
                    self.show_status(
                        "Analyzing with OpenAI…",
                        StatusKind::Info,
                        Some(Duration::from_secs(2)),
                    );
                }
                AppEvent::AnalysisFinished { response } => {
                    let entry = ConversationEntry::new(
                        ConversationRole::Assistant,
                        response.answer.clone(),
                    );
                    self.session.append(entry.clone());
                    self.conversation.push(entry);
                    self.auto_scroll = true;
                    self.active_request = None;
                    if let Err(err) = self.session.write_plaintext_log() {
                        log::warn!("failed to persist conversation log: {err}");
                    }
                    self.show_status(
                        "Response received",
                        StatusKind::Success,
                        Some(Duration::from_secs(2)),
                    );
                }
                AppEvent::AnalysisFailed {
                    request_id: _,
                    error,
                } => {
                    self.active_request = None;
                    self.show_status(format!("Analysis failed: {error}"), StatusKind::Error, None);
                }
                AppEvent::TranscriptionFinished { text } => {
                    self.last_transcription = Some(text.clone());
                    if self.ask_input.trim().is_empty() {
                        self.ask_input = text.clone();
                    } else {
                        self.ask_input.push('\n');
                        self.ask_input.push_str(&text);
                    }
                    self.transcription_pending = false;
                    self.show_status(
                        "Transcription completed",
                        StatusKind::Success,
                        Some(Duration::from_secs(2)),
                    );
                }
                AppEvent::TranscriptionFailed { error } => {
                    self.transcription_pending = false;
                    self.show_status(
                        format!("Transcription failed: {error}"),
                        StatusKind::Error,
                        None,
                    );
                }
                AppEvent::Status {
                    text,
                    kind,
                    duration,
                } => {
                    self.show_status(text, kind, duration);
                }
            }
        }
    }

    fn process_hotkeys(&mut self, _frame: &mut eframe::Frame) {
        while let Ok(action) = self.hotkey_rx.try_recv() {
            match action {
                HotkeyAction::ToggleAskPanel => {
                    self.ask_panel_open = !self.ask_panel_open;
                }
                HotkeyAction::ToggleRecording => {
                    if self.is_recording {
                        self.stop_recording();
                    } else if let Err(err) = self.start_recording() {
                        self.show_status(
                            format!("Failed to start recording: {err}"),
                            StatusKind::Error,
                            None,
                        );
                    }
                }
                HotkeyAction::ToggleHidden => {
                    self.is_hidden = !self.is_hidden;
                    // Note: Window visibility will be controlled in update() method
                }
                HotkeyAction::ClearSession => {
                    self.clear_session();
                }
                HotkeyAction::CaptureScreenshot => {
                    if let Err(err) = self.capture_and_attach() {
                        self.show_status(format!("Capture failed: {err}"), StatusKind::Error, None);
                    }
                }
            }
        }
    }

    fn capture_and_attach(&mut self) -> Result<()> {
        // Check if we should hide before capture
        let should_hide = self.config.capture.hide_before_capture;

        if should_hide {
            // Signal that we want to hide the window
            // Note: The actual hiding will happen in the next update cycle
            self.is_hidden = true;
            // Give the window manager time to hide the window
            std::thread::sleep(Duration::from_millis(200));
        }

        let capture = capture_screen(self.config.capture.mode.clone())?;

        if should_hide {
            // Restore window visibility
            self.is_hidden = false;
        }

        let attachment = ScreenshotAttachment::from_capture(capture)?;
        self.attach = Some(attachment);
        self.attach_texture = None;
        self.show_status(
            "Screenshot attached",
            StatusKind::Info,
            Some(Duration::from_secs(2)),
        );
        Ok(())
    }

    fn start_recording(&mut self) -> Result<()> {
        let recorder = AudioRecorder::start()?;
        self.audio_recorder = Some(recorder);
        self.is_recording = true;
        self.transcription_pending = false;
        self.show_status(
            "Recording… press hotkey again to stop",
            StatusKind::Info,
            None,
        );
        Ok(())
    }

    fn stop_recording(&mut self) {
        if let Some(recorder) = self.audio_recorder.take() {
            self.is_recording = false;
            match recorder.stop() {
                Ok(recording) => {
                    self.transcription_pending = true;
                    self.enqueue_transcription(recording);
                }
                Err(err) => {
                    self.show_status(
                        format!("Failed to stop recorder: {err}"),
                        StatusKind::Error,
                        None,
                    );
                }
            }
        }
    }

    fn enqueue_transcription(&self, recording: crate::audio::RecordingResult) {
        let tx = self.events_tx.clone();
        let client = Arc::clone(&self.openai);
        let cfg = self.config.openai.clone();
        let language = self.config.transcription.language.clone();
        let model = self.config.transcription.model.clone();
        self.runtime.spawn(async move {
            let status = AppEvent::Status {
                text: "Transcribing audio…".into(),
                kind: StatusKind::Info,
                duration: Some(Duration::from_secs(2)),
            };
            let _ = tx.send(status);
            match client.transcribe(&cfg, recording, language, &model).await {
                Ok(text) => {
                    let _ = tx.send(AppEvent::TranscriptionFinished { text });
                }
                Err(err) => {
                    let _ = tx.send(AppEvent::TranscriptionFailed {
                        error: err.to_string(),
                    });
                }
            }
        });
    }

    fn clear_session(&mut self) {
        self.session.reset();
        self.conversation.clear();
        self.attach = None;
        self.attach_texture = None;
        self.auto_scroll = true;
        self.show_status(
            "Session cleared",
            StatusKind::Info,
            Some(Duration::from_secs(2)),
        );
    }

    fn submit_current_prompt(&mut self) {
        let trimmed = self.ask_input.trim().to_string();
        if trimmed.is_empty() && self.attach.is_none() {
            self.show_status(
                "輸入問題或附加截圖後再送出",
                StatusKind::Warning,
                Some(Duration::from_secs(3)),
            );
            return;
        }
        if self.config.openai.api_key.trim().is_empty() {
            self.show_status("OpenAI API Key 未設定", StatusKind::Error, None);
            return;
        }

        if !trimmed.is_empty() {
            let entry = ConversationEntry::new(ConversationRole::User, trimmed.clone());
            self.session.append(entry.clone());
            self.conversation.push(entry);
        }

        let history: VecDeque<ConversationEntry> = self.conversation.iter().cloned().collect();
        let request_id = Uuid::new_v4();
        let custom_prompt = self.load_active_prompt();
        let screenshot = self.attach.as_ref().map(|att| att.png.clone());
        let analyze_request = AnalyzeRequest {
            request_id,
            config: self.config.openai.clone(),
            text_prompt: if trimmed.is_empty() {
                String::new()
            } else {
                trimmed
            },
            custom_prompt,
            screenshot_png: screenshot,
            history,
        };

        if let Err(err) = self.request_tx.send(analyze_request) {
            self.show_status(
                format!("Failed to queue analysis request: {err}"),
                StatusKind::Error,
                None,
            );
        } else {
            self.attach = None;
            self.attach_texture = None;
            self.ask_input.clear();
            self.auto_scroll = true;
        }
    }

    fn load_active_prompt(&self) -> Option<String> {
        let name = self.config.prompts.active_prompt_name.as_ref()?;
        let path = config::prompts_dir().ok()?.join(name);
        fs::read_to_string(path).ok()
    }

    fn show_status(
        &mut self,
        text: impl Into<String>,
        kind: StatusKind,
        duration: Option<Duration>,
    ) {
        self.status = Some(StatusMessage {
            text: text.into(),
            kind,
            expires_at: duration.map(|d| Instant::now() + d),
        });
    }

    fn update_status(&mut self) {
        if let Some(status) = &self.status {
            if let Some(expiry) = status.expires_at {
                if Instant::now() > expiry {
                    self.status = None;
                }
            }
        }
    }

    fn ensure_attachment_texture(&mut self, ctx: &egui::Context) {
        if let Some(att) = &self.attach {
            if self
                .attach_texture
                .as_ref()
                .map(|tex| tex.id == att.id)
                .unwrap_or(false)
            {
                return;
            }
            if let Some(preview) = &att.preview {
                let tex = ctx.load_texture(
                    format!("screenshot-preview-{}", att.id),
                    preview.clone(),
                    TextureOptions::LINEAR,
                );
                self.attach_texture = Some(ScreenshotTexture {
                    id: att.id,
                    texture: tex,
                });
            }
        }
    }

    fn render_hud(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            let listen_label = if self.is_recording {
                "Stop Recording"
            } else {
                "Start Recording"
            };
            if ui.button(listen_label).clicked() {
                if self.is_recording {
                    self.stop_recording();
                } else if let Err(err) = self.start_recording() {
                    self.show_status(format!("Failed to record: {err}"), StatusKind::Error, None);
                }
            }

            if ui
                .button(if self.ask_panel_open {
                    "Hide Ask Panel"
                } else {
                    "Show Ask Panel"
                })
                .clicked()
            {
                self.ask_panel_open = !self.ask_panel_open;
            }

            if ui
                .button(if self.is_hidden {
                    "Show Window"
                } else {
                    "Hide Window"
                })
                .clicked()
            {
                self.is_hidden = !self.is_hidden;
                // Note: Window visibility will be controlled in update() method
            }

            if ui.button("Settings").clicked() {
                self.settings_open = true;
            }

            if ui.button("Clear Session").clicked() {
                self.clear_session();
            }
        });
    }

    fn render_conversation(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in &self.conversation {
                    render_entry(ui, entry);
                    ui.add_space(6.0);
                }
            });
    }

    fn render_ask_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if let Some(last) = &self.last_transcription {
            ui.label(
                RichText::new(format!("Last voice input: {last}"))
                    .italics()
                    .color(Color32::from_gray(160)),
            );
        }
        let response = egui::TextEdit::multiline(&mut self.ask_input)
            .desired_rows(4)
            .hint_text("輸入問題或想法…")
            .show(ui);
        if response.response.lost_focus()
            && response.response.ctx.input(|i| {
                i.key_pressed(egui::Key::Enter) && (i.modifiers.command || i.modifiers.ctrl)
            })
        {
            self.submit_current_prompt();
        }

        ui.horizontal(|ui| {
            if ui.button("Send").clicked() {
                self.submit_current_prompt();
            }
            if ui.button("Attach Screenshot").clicked() {
                if let Err(err) = self.capture_and_attach() {
                    self.show_status(format!("Capture failed: {err}"), StatusKind::Error, None);
                }
            }
            if ui.button("Remove Attachment").clicked() {
                self.attach = None;
                self.attach_texture = None;
            }
            if ui.button("Save Config").clicked() {
                if let Err(err) = config::save(&self.config) {
                    self.show_status(
                        format!("Failed to save config: {err}"),
                        StatusKind::Error,
                        None,
                    );
                } else {
                    self.show_status(
                        "Config saved",
                        StatusKind::Success,
                        Some(Duration::from_secs(2)),
                    );
                }
            }
        });

        // Separate the attachment rendering to avoid borrow checker issues
        let (width, height) = self.attach.as_ref().map(|a| (a.width, a.height)).unwrap_or((0, 0));
        if self.attach.is_some() {
            self.ensure_attachment_texture(ctx);
            ui.separator();
            ui.label(format!("Attached screenshot: {}x{}", width, height));
            if let Some(tex) = self.attach_texture.as_ref().map(|t| t.texture.clone()) {
                let size = tex.size_vec2();
                let max_width = 240.0;
                let scale = if size.x > 0.0 {
                    f32::min(max_width / size.x, 1.0)
                } else {
                    1.0
                };
                ui.image(egui::load::SizedTexture::new(tex.id(), size * scale));
            }
        }
        if self.active_request.is_some() {
            ui.add_space(8.0);
            ui.label(RichText::new("Waiting for OpenAI response…").color(Color32::LIGHT_BLUE));
        }
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        self.update_status();
        if let Some(status) = &self.status {
            let color = status.kind.color();
            ui.horizontal(|ui| {
                ui.label(RichText::new(&status.text).color(color));
            });
        } else {
            ui.label("Ready");
        }
    }

    fn render_settings(&mut self, ctx: &egui::Context) {
        let mut settings_open = self.settings_open;
        if settings_open {
            egui::Window::new("Settings")
                .open(&mut settings_open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.heading("OpenAI");
                    ui.label("API Key");
                    ui.text_edit_singleline(&mut self.config.openai.api_key);
                    ui.label("Base URL");
                    ui.text_edit_singleline(&mut self.config.openai.base_url);
                    ui.label("Model");
                    ui.text_edit_singleline(&mut self.config.openai.model);
                    ui.horizontal(|ui| {
                        ui.label("Temperature");
                        ui.add(
                            egui::DragValue::new(&mut self.config.openai.temperature)
                                .speed(0.05)
                                .range(0.0..=2.0),
                        );
                    });
                    ui.separator();
                    ui.heading("Transcription");
                    ui.checkbox(
                        &mut self.config.transcription.enabled,
                        "Enable voice transcription",
                    );
                    ui.checkbox(&mut self.config.transcription.realtime, "Realtime mode");
                    ui.label("Model");
                    ui.text_edit_singleline(&mut self.config.transcription.model);
                    ui.label("Language");
                    egui::ComboBox::from_label("Language")
                        .selected_text(match self.config.transcription.language {
                            crate::config::TranscriptionLanguage::En => "English",
                            crate::config::TranscriptionLanguage::Zh => "Chinese",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.config.transcription.language,
                                crate::config::TranscriptionLanguage::En,
                                "English",
                            );
                            ui.selectable_value(
                                &mut self.config.transcription.language,
                                crate::config::TranscriptionLanguage::Zh,
                                "Chinese",
                            );
                        });
                    ui.separator();
                    if ui.button("Validate API Key").clicked() {
                        let cfg = self.config.openai.clone();
                        let tx = self.events_tx.clone();
                        let client = Arc::clone(&self.openai);
                        self.runtime.spawn(async move {
                            match client.validate(&cfg).await {
                                Ok(true) => {
                                    let _ = tx.send(AppEvent::Status {
                                        text: "OpenAI credentials valid".into(),
                                        kind: StatusKind::Success,
                                        duration: Some(Duration::from_secs(3)),
                                    });
                                }
                                Ok(false) => {
                                    let _ = tx.send(AppEvent::Status {
                                        text: "Validation failed".into(),
                                        kind: StatusKind::Error,
                                        duration: None,
                                    });
                                }
                                Err(err) => {
                                    let _ = tx.send(AppEvent::Status {
                                        text: format!("Validation error: {err}"),
                                        kind: StatusKind::Error,
                                        duration: None,
                                    });
                                }
                            }
                        });
                    }
                    if ui.button("Save Settings").clicked() {
                        if let Err(err) = config::save(&self.config) {
                            self.show_status(
                                format!("Failed to save config: {err}"),
                                StatusKind::Error,
                                None,
                            );
                        } else {
                            self.show_status(
                                "Settings saved",
                                StatusKind::Success,
                                Some(Duration::from_secs(2)),
                            );
                        }
                    }
                });
            self.settings_open = settings_open;
        }
    }
}

impl eframe::App for GhostApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.process_background_events();
        self.process_hotkeys(frame);

        // Handle window visibility based on is_hidden state
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!self.is_hidden));

        // Set transparent background
        let frame_bg = egui::Color32::from_rgba_premultiplied(0, 0, 0, (255.0 * 0.95) as u8);
        let mut style = (*ctx.style()).clone();
        style.visuals.window_fill = frame_bg;
        style.visuals.panel_fill = frame_bg;
        ctx.set_style(style);

        egui::TopBottomPanel::top("hud").show(ctx, |ui| self.render_hud(ui, frame));

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_conversation(ui);
        });

        if self.ask_panel_open {
            egui::TopBottomPanel::bottom("ask-panel")
                .frame(egui::Frame::default().inner_margin(Margin::same(8.0)))
                .min_height(200.0)
                .show(ctx, |ui| {
                    self.render_ask_panel(ctx, ui);
                });
        }

        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .show(ctx, |ui| self.render_status_bar(ui));

        self.render_settings(ctx);
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

fn render_entry(ui: &mut egui::Ui, entry: &ConversationEntry) {
    let label = match entry.role {
        ConversationRole::User => "You",
        ConversationRole::Assistant => "Ghost",
        ConversationRole::System => "System",
        ConversationRole::Reasoning => "Reasoning",
        ConversationRole::Error => "Error",
    };
    let color = match entry.role {
        ConversationRole::User => Color32::from_rgb(180, 220, 255),
        ConversationRole::Assistant => Color32::from_rgb(190, 255, 190),
        ConversationRole::System => Color32::from_rgb(200, 200, 200),
        ConversationRole::Reasoning => Color32::from_rgb(200, 200, 255),
        ConversationRole::Error => Color32::from_rgb(255, 180, 180),
    };
    let header = RichText::new(format!("{} — {}", label, entry.timestamp.to_rfc2822()))
        .color(color)
        .strong();
    ui.label(header);
    ui.separator();
    ui.label(&entry.content);
}

fn spawn_analyze_worker(
    runtime: &Handle,
    client: Arc<OpenAIClient>,
    mut requests: UnboundedReceiver<AnalyzeRequest>,
    events: UnboundedSender<AppEvent>,
) {
    let events_clone = events.clone();
    runtime.spawn(async move {
        while let Some(request) = requests.recv().await {
            let request_id = request.request_id;
            let _ = events_clone.send(AppEvent::AnalysisStarted { request_id });
            match client.analyze(request).await {
                Ok(response) => {
                    let _ = events_clone.send(AppEvent::AnalysisFinished { response });
                }
                Err(err) => {
                    let _ = events_clone.send(AppEvent::AnalysisFailed {
                        request_id,
                        error: err.to_string(),
                    });
                }
            }
        }
    });
}

#[derive(Clone)]
struct ScreenshotAttachment {
    id: Uuid,
    png: Vec<u8>,
    width: u32,
    height: u32,
    preview: Option<egui::ColorImage>,
}

impl ScreenshotAttachment {
    fn from_capture(capture: CaptureResult) -> Result<Self> {
        let preview = decode_preview(&capture.png_bytes)?;
        Ok(Self {
            id: Uuid::new_v4(),
            png: capture.png_bytes,
            width: capture.width,
            height: capture.height,
            preview,
        })
    }
}

fn decode_preview(png: &[u8]) -> Result<Option<egui::ColorImage>> {
    let dyn_image = match image::load_from_memory(png) {
        Ok(img) => img,
        Err(err) => {
            log::warn!("failed to decode screenshot preview: {err}");
            return Ok(None);
        }
    };
    let (width, height) = dyn_image.dimensions();
    let mut rgba = dyn_image.to_rgba8();
    let max_width: u32 = 720;
    if width > max_width {
        let new_height = (height as f32 * (max_width as f32 / width as f32))
            .round()
            .max(1.0) as u32;
        rgba = image::imageops::resize(&rgba, max_width, new_height, FilterType::Triangle);
    }
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    Ok(Some(egui::ColorImage::from_rgba_unmultiplied(
        size, &pixels,
    )))
}

struct ScreenshotTexture {
    id: Uuid,
    texture: egui::TextureHandle,
}

#[derive(Clone)]
struct StatusMessage {
    text: String,
    kind: StatusKind,
    expires_at: Option<Instant>,
}

#[derive(Clone, Copy)]
enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

impl StatusKind {
    fn color(self) -> Color32 {
        match self {
            StatusKind::Info => Color32::from_rgb(150, 200, 255),
            StatusKind::Success => Color32::from_rgb(160, 240, 160),
            StatusKind::Warning => Color32::from_rgb(255, 220, 120),
            StatusKind::Error => Color32::from_rgb(255, 160, 160),
        }
    }
}

enum AppEvent {
    AnalysisStarted {
        request_id: Uuid,
    },
    AnalysisFinished {
        response: AnalyzeResponse,
    },
    AnalysisFailed {
        request_id: Uuid,
        error: String,
    },
    TranscriptionFinished {
        text: String,
    },
    TranscriptionFailed {
        error: String,
    },
    Status {
        text: String,
        kind: StatusKind,
        duration: Option<Duration>,
    },
}
