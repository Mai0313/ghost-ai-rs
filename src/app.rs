use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{self, Color32, Margin, RichText, TextureOptions};
use pulldown_cmark::{Event, Parser, Tag};
use image::imageops::FilterType;
use image::GenericImageView;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::capture::{capture_screen, CaptureResult};
use crate::config::{self, AppConfig, CaptureMode, ThemeVariant};
use crate::hotkeys::{self, HotkeyAction, HotkeyHandle};
use crate::openai::{AnalyzeRequest, AnalyzeResponse, OpenAIClient, StreamEvent};
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
    stream_rx: UnboundedReceiver<(Uuid, StreamEvent)>,
    stream_tx: UnboundedSender<(Uuid, StreamEvent)>,
    hotkey_rx: UnboundedReceiver<HotkeyAction>,
    _hotkey_handle: Option<HotkeyHandle>,
    status: Option<StatusMessage>,
    settings_open: bool,
    is_hidden: bool,
    active_request: Option<Uuid>,
    auto_scroll: bool,
    prompt_files: Vec<String>,
    prompt_editor_selected: Option<String>,
    prompt_editor_content: String,
    prompt_editor_dirty: bool,
    new_prompt_name: String,
}

impl GhostApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Handle) -> Self {
        let mut config = config::load_or_default().unwrap_or_default();
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
        cc.egui_ctx.style_mut(|style| style.url_in_tooltip = true);

        let openai = Arc::new(OpenAIClient::new().unwrap_or_else(|err| {
            log::error!("failed to construct OpenAI client: {err}");
            OpenAIClient::new().expect("OpenAI client")
        }));

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (stream_tx, stream_rx) = mpsc::unbounded_channel();
        spawn_analyze_worker(
            &runtime,
            Arc::clone(&openai),
            request_rx,
            events_tx.clone(),
            stream_tx.clone(),
        );

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

        let mut prompt_files = match Self::list_prompt_files() {
            Ok(files) => files,
            Err(err) => {
                log::warn!("failed to enumerate prompts directory: {err}");
                Vec::new()
            }
        };
        prompt_files.sort();
        if config
            .prompts
            .active_prompt_name
            .as_ref()
            .map(|name| !prompt_files.contains(name))
            .unwrap_or(false)
        {
            if let Some(name) = config.prompts.active_prompt_name.take() {
                log::warn!("configured active prompt '{name}' missing; clearing reference");
            }
        }
        if config
            .prompts
            .default_prompt_name
            .as_ref()
            .map(|name| !prompt_files.contains(name))
            .unwrap_or(false)
        {
            if let Some(name) = config.prompts.default_prompt_name.take() {
                log::warn!("configured default prompt '{name}' missing; clearing reference");
            }
        }
        let prompt_editor_selected = config
            .prompts
            .active_prompt_name
            .clone()
            .filter(|name| prompt_files.contains(name));
        let prompt_editor_content = prompt_editor_selected
            .as_ref()
            .and_then(|name| Self::read_prompt_file(name).ok())
            .unwrap_or_default();

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
            stream_rx,
            stream_tx,
            hotkey_rx,
            _hotkey_handle: hotkey_handle,
            status: None,
            settings_open: false,
            is_hidden: false,
            active_request: None,
            auto_scroll: true,
            prompt_files,
            prompt_editor_selected,
            prompt_editor_content,
            prompt_editor_dirty: false,
            new_prompt_name: String::new(),
        }
    }

    fn list_prompt_files() -> Result<Vec<String>> {
        let dir = config::prompts_dir()?;
        let mut files = Vec::new();
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') {
                            files.push(name.to_string());
                        }
                    }
                }
            }
        }
        files.sort();
        Ok(files)
    }

    fn prompt_path(name: &str) -> Result<PathBuf> {
        Ok(config::prompts_dir()?.join(name))
    }

    fn read_prompt_file(name: &str) -> Result<String> {
        let path = Self::prompt_path(name)?;
        Ok(fs::read_to_string(path)?)
    }

    fn write_prompt_file(name: &str, content: &str) -> Result<()> {
        let path = Self::prompt_path(name)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn delete_prompt_file(name: &str) -> Result<()> {
        let path = Self::prompt_path(name)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn sanitize_prompt_name(input: &str) -> Option<String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.contains(['/', '\\']) || trimmed.contains("..") {
            return None;
        }
        let mut sanitized = String::with_capacity(trimmed.len());
        for ch in trimmed.chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                sanitized.push(ch);
            } else if ch.is_whitespace() {
                sanitized.push('_');
            } else {
                sanitized.push('_');
            }
        }
        while sanitized.starts_with('.') {
            sanitized.remove(0);
        }
        if sanitized.is_empty() || !sanitized.chars().any(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        if !sanitized.ends_with(".md") && !sanitized.ends_with(".txt") {
            sanitized.push_str(".md");
        }
        Some(sanitized)
    }

    fn refresh_prompt_files(&mut self) {
        match Self::list_prompt_files() {
            Ok(files) => {
                self.prompt_files = files;
                self.ensure_prompt_references();
            }
            Err(err) => {
                log::error!("failed to refresh prompts: {err}");
                self.show_status(
                    format!("Failed to refresh prompts: {err}"),
                    StatusKind::Error,
                    None,
                );
            }
        }
    }

    fn ensure_prompt_references(&mut self) {
        if self
            .config
            .prompts
            .active_prompt_name
            .as_ref()
            .map(|name| !self.prompt_files.contains(name))
            .unwrap_or(false)
        {
            if let Some(name) = self.config.prompts.active_prompt_name.take() {
                log::warn!("configured active prompt '{name}' missing; clearing reference");
            }
        }
        if self
            .config
            .prompts
            .default_prompt_name
            .as_ref()
            .map(|name| !self.prompt_files.contains(name))
            .unwrap_or(false)
        {
            if let Some(name) = self.config.prompts.default_prompt_name.take() {
                log::warn!("configured default prompt '{name}' missing; clearing reference");
            }
        }
        if let Some(current) = self.prompt_editor_selected.clone() {
            if !self.prompt_files.contains(&current) {
                self.prompt_editor_selected = None;
                self.prompt_editor_content.clear();
                self.prompt_editor_dirty = false;
            }
        }
    }

    fn load_prompt_into_editor(&mut self, selection: Option<String>) {
        match selection {
            Some(name) => match Self::read_prompt_file(&name) {
                Ok(content) => {
                    self.prompt_editor_selected = Some(name);
                    self.prompt_editor_content = content;
                    self.prompt_editor_dirty = false;
                }
                Err(err) => {
                    log::error!("failed to load prompt: {err}");
                    self.show_status(
                        format!("Failed to load prompt: {err}"),
                        StatusKind::Error,
                        None,
                    );
                    self.prompt_editor_selected = None;
                    self.prompt_editor_content.clear();
                    self.prompt_editor_dirty = false;
                }
            },
            None => {
                self.prompt_editor_selected = None;
                self.prompt_editor_content.clear();
                self.prompt_editor_dirty = false;
            }
        }
    }

    fn save_current_prompt(&mut self) {
        if let Some(name) = self.prompt_editor_selected.clone() {
            match Self::write_prompt_file(&name, &self.prompt_editor_content) {
                Ok(()) => {
                    self.prompt_editor_dirty = false;
                    self.show_status(
                        format!("Prompt '{name}' saved"),
                        StatusKind::Success,
                        Some(Duration::from_secs(2)),
                    );
                    self.refresh_prompt_files();
                    self.load_prompt_into_editor(Some(name));
                }
                Err(err) => {
                    self.show_status(
                        format!("Failed to save prompt: {err}"),
                        StatusKind::Error,
                        None,
                    );
                }
            }
        } else {
            let Some(sanitized) = Self::sanitize_prompt_name(&self.new_prompt_name) else {
                self.show_status(
                    "Enter a valid prompt name (letters, numbers, '-' or '_')",
                    StatusKind::Warning,
                    Some(Duration::from_secs(3)),
                );
                return;
            };
            if self
                .prompt_files
                .iter()
                .any(|existing| existing == &sanitized)
            {
                self.show_status(
                    format!("Prompt '{sanitized}' already exists"),
                    StatusKind::Error,
                    None,
                );
                return;
            }
            match Self::write_prompt_file(&sanitized, &self.prompt_editor_content) {
                Ok(()) => {
                    self.prompt_editor_dirty = false;
                    self.show_status(
                        format!("Prompt '{sanitized}' created"),
                        StatusKind::Success,
                        Some(Duration::from_secs(2)),
                    );
                    self.refresh_prompt_files();
                    self.load_prompt_into_editor(Some(sanitized.clone()));
                    self.new_prompt_name.clear();
                }
                Err(err) => {
                    self.show_status(
                        format!("Failed to create prompt: {err}"),
                        StatusKind::Error,
                        None,
                    );
                }
            }
        }
    }

    fn delete_selected_prompt(&mut self) {
        let Some(name) = self.prompt_editor_selected.clone() else {
            self.show_status(
                "Select a prompt to delete",
                StatusKind::Warning,
                Some(Duration::from_secs(3)),
            );
            return;
        };
        match Self::delete_prompt_file(&name) {
            Ok(()) => {
                self.show_status(
                    format!("Prompt '{name}' deleted"),
                    StatusKind::Info,
                    Some(Duration::from_secs(2)),
                );
                self.prompt_editor_selected = None;
                self.prompt_editor_content.clear();
                self.prompt_editor_dirty = false;
                self.new_prompt_name.clear();
                self.refresh_prompt_files();
            }
            Err(err) => {
                self.show_status(
                    format!("Failed to delete prompt: {err}"),
                    StatusKind::Error,
                    None,
                );
            }
        }
    }
    fn process_background_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                AppEvent::AnalysisStarted { request_id } => {
                    self.active_request = Some(request_id);
                    // Create placeholder entry for streaming
                    let entry = ConversationEntry::new(ConversationRole::Assistant, String::new());
                    self.conversation.push(entry);
                    self.auto_scroll = true;
                    self.show_status(
                        "Analyzing with OpenAI…",
                        StatusKind::Info,
                        Some(Duration::from_secs(2)),
                    );
                }
                AppEvent::AnalysisFinished { response } => {
                    // Update the last entry with the final answer
                    if let Some(last) = self.conversation.last_mut() {
                        if matches!(last.role, ConversationRole::Assistant) {
                            last.content = response.answer.clone();
                            self.session.append(last.clone());
                        }
                    }
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
                AppEvent::Status {
                    text,
                    kind,
                    duration,
                } => {
                    self.show_status(text, kind, duration);
                }
            }
        }

        // Process streaming events
        while let Ok((request_id, stream_event)) = self.stream_rx.try_recv() {
            if Some(request_id) != self.active_request {
                continue;
            }

            match stream_event {
                StreamEvent::Delta(delta) => {
                    if let Some(last) = self.conversation.last_mut() {
                        if matches!(last.role, ConversationRole::Assistant) {
                            last.content.push_str(&delta);
                            self.auto_scroll = true;
                        }
                    }
                }
                StreamEvent::Done(full_text) => {
                    if let Some(last) = self.conversation.last_mut() {
                        if matches!(last.role, ConversationRole::Assistant) {
                            last.content = full_text;
                        }
                    }
                }
                StreamEvent::Error(error) => {
                    log::error!("Stream error: {error}");
                    self.show_status(format!("Stream error: {error}"), StatusKind::Error, None);
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
            if ui
                .button(if self.ask_panel_open {
                    "Hide Ask"
                } else {
                    "Show Ask"
                })
                .clicked()
            {
                self.ask_panel_open = !self.ask_panel_open;
            }

            if ui.button("Capture Screenshot").clicked() {
                if let Err(err) = self.capture_and_attach() {
                    self.show_status(format!("Capture failed: {err}"), StatusKind::Error, None);
                }
            }

            if ui.button("Settings").clicked() {
                self.settings_open = true;
            }

            if ui.button("Clear Session").clicked() {
                self.clear_session();
            }

            if ui
                .button(if self.is_hidden { "Show" } else { "Hide" })
                .clicked()
            {
                self.is_hidden = !self.is_hidden;
            }
        });
    }

    fn render_conversation(&mut self, ui: &mut egui::Ui) {
        let scroll_to_bottom = self.auto_scroll;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in &self.conversation {
                    self.render_entry(ui, entry);
                    ui.add_space(6.0);
                }
                if scroll_to_bottom {
                    ui.scroll_to_cursor(egui::Align::BOTTOM);
                }
            });
        if scroll_to_bottom {
            self.auto_scroll = false;
        }
    }

    fn render_markdown(ui: &mut egui::Ui, text: &str) {
        use pulldown_cmark::{Event, Parser, Tag};

        enum Block {
            Heading(usize, String),
            Paragraph(String),
            Code(String),
            ListItem(String),
        }

        let mut blocks: Vec<Block> = Vec::new();
        let mut current = String::new();
        let mut list_level = 0usize;
        let mut in_item = false;
        let mut heading_level: Option<usize> = None;
        let mut in_code_block = false;

        for event in Parser::new(text) {
            match event {
                Event::Start(Tag::Heading(level)) => {
                    heading_level = Some(level as usize);
                    current.clear();
                }
                Event::End(Tag::Heading(_)) => {
                    let content = current.trim().to_string();
                    if !content.is_empty() {
                        blocks.push(Block::Heading(heading_level.unwrap_or(1), content));
                    }
                    current.clear();
                    heading_level = None;
                }
                Event::Start(Tag::Paragraph) => {
                    current.clear();
                }
                Event::End(Tag::Paragraph) => {
                    if !in_item {
                        let content = current.trim().to_string();
                        if !content.is_empty() {
                            blocks.push(Block::Paragraph(content));
                        }
                    }
                    current.clear();
                }
                Event::Start(Tag::BulletList(_)) => {
                    list_level += 1;
                }
                Event::End(Tag::BulletList(_)) => {
                    if list_level > 0 {
                        list_level -= 1;
                    }
                }
                Event::Start(Tag::Item) => {
                    in_item = true;
                    current.clear();
                }
                Event::End(Tag::Item) => {
                    let indent = "  ".repeat(list_level.saturating_sub(1));
                    let content = current.trim().to_string();
                    if !content.is_empty() {
                        blocks.push(Block::ListItem(format!("{}* {}", indent, content)));
                    }
                    current.clear();
                    in_item = false;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    current.clear();
                }
                Event::End(Tag::CodeBlock(_)) => {
                    let content = current.trim_matches('\n').to_string();
                    if !content.is_empty() {
                        blocks.push(Block::Code(content));
                    }
                    current.clear();
                    in_code_block = false;
                }
                Event::Text(text_chunk) => {
                    current.push_str(&text_chunk);
                }
                Event::Code(code) => {
                    current.push_str(&format !("`{code}`")); 
                }
                Event::SoftBreak => {
                    if in_code_block {
                        current.push('\n');
                    } else {
                        current.push(' ');
                    }
                }
                Event::HardBreak => {
                    current.push('\n');
                }
                Event::Html(_) | Event::FootnoteReference(_) | Event::TaskListMarker(_) => {}
                Event::Start(_) | Event::End(_) => {}
            }
        }

        if in_code_block && !current.trim().is_empty() {
            blocks.push(Block::Code(current.trim_matches('\n').to_string()));
        } else if in_item && !current.trim().is_empty() {
            let indent = "  ".repeat(list_level.saturating_sub(1));
            blocks.push(Block::ListItem(format!("{}* {}", indent, current.trim())));
        } else if let Some(level) = heading_level {
            let content = current.trim().to_string();
            if !content.is_empty() {
                blocks.push(Block::Heading(level, content));
            }
        } else if !current.trim().is_empty() {
            blocks.push(Block::Paragraph(current.trim().to_string()));
        }

        for block in blocks {
            match block {
                Block::Heading(level, content) => {
                    if content.is_empty() {
                        continue;
                    }
                    let mut rich = RichText::new(content);
                    match level {
                        1 => rich = rich.heading(),
                        2 => rich = rich.heading().size(20.0),
                        3 => rich = rich.strong().size(18.0),
                        _ => rich = rich.strong(),
                    }
                    ui.label(rich);
                    ui.add_space(4.0);
                }
                Block::Paragraph(content) => {
                    if !content.is_empty() {
                        ui.label(content);
                    }
                    ui.add_space(4.0);
                }
                Block::ListItem(content) => {
                    ui.label(content);
                    ui.add_space(2.0);
                }
                Block::Code(content) => {
                    let mut code = content;
                    ui.add(
                        egui::TextEdit::multiline(&mut code)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(code.lines().count().max(1) as u32)
                            .lock_focus(true)
                            .interactive(false),
                    );
                    ui.add_space(6.0);
                }
            }
        }
    }

    fn render_ask_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
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
        let (width, height) = self
            .attach
            .as_ref()
            .map(|a| (a.width, a.height))
            .unwrap_or((0, 0));
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
                .default_width(520.0)
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
                    ui.horizontal(|ui| {
                        ui.label("Max output tokens");
                        let mut max_tokens = self.config.openai.max_output_tokens.unwrap_or(0);
                        if ui
                            .add(
                                egui::DragValue::new(&mut max_tokens)
                                    .speed(16.0)
                                    .clamp_range(0..=32768),
                            )
                            .changed()
                        {
                            self.config.openai.max_output_tokens = if max_tokens == 0 {
                                None
                            } else {
                                Some(max_tokens)
                            };
                        }
                        ui.label("(0 disables the limit)");
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
                    egui::ComboBox::from_id_source("transcription-language")
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
                    ui.heading("Capture");
                    ui.checkbox(
                        &mut self.config.capture.attach_screenshots,
                        "Attach screenshots when sending prompts",
                    );
                    ui.checkbox(
                        &mut self.config.capture.hide_before_capture,
                        "Hide window before capturing",
                    );
                    egui::ComboBox::from_id_source("capture-mode")
                        .selected_text(match self.config.capture.mode {
                            CaptureMode::ActiveMonitor => "Active monitor",
                            CaptureMode::Primary => "Primary monitor",
                            CaptureMode::Region => "Select region (coming soon)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.config.capture.mode,
                                CaptureMode::ActiveMonitor,
                                "Active monitor",
                            );
                            ui.selectable_value(
                                &mut self.config.capture.mode,
                                CaptureMode::Primary,
                                "Primary monitor",
                            );
                            ui.selectable_value(
                                &mut self.config.capture.mode,
                                CaptureMode::Region,
                                "Select region (coming soon)",
                            );
                        });

                    ui.separator();
                    ui.heading("Hotkeys");
                    ui.label(
                        "Use platform key names like Ctrl+Shift+Enter. Leave blank to disable.",
                    );
                    ui.horizontal(|ui| {
                        ui.label("Toggle Ask panel");
                        ui.text_edit_singleline(&mut self.config.hotkeys.toggle_ask_panel);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Toggle visibility");
                        ui.text_edit_singleline(&mut self.config.hotkeys.toggle_hide);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Clear session");
                        ui.text_edit_singleline(&mut self.config.hotkeys.clear_session);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Capture screenshot");
                        ui.text_edit_singleline(&mut self.config.hotkeys.capture_screenshot);
                    });

                    ui.separator();
                    ui.heading("UI");
                    ui.add(
                        egui::Slider::new(&mut self.config.ui.opacity, 0.4..=1.0)
                            .text("Window opacity")
                            .clamp_to_range(true),
                    );
                    ui.checkbox(&mut self.config.ui.compact_mode, "Compact layout");
                    egui::ComboBox::from_id_source("theme-variant")
                        .selected_text(match self.config.ui.theme {
                            ThemeVariant::Light => "Light",
                            ThemeVariant::Dark => "Dark",
                            ThemeVariant::System => "Follow system",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.config.ui.theme,
                                ThemeVariant::Light,
                                "Light",
                            );
                            ui.selectable_value(
                                &mut self.config.ui.theme,
                                ThemeVariant::Dark,
                                "Dark",
                            );
                            ui.selectable_value(
                                &mut self.config.ui.theme,
                                ThemeVariant::System,
                                "Follow system",
                            );
                        });

                    ui.separator();
                    ui.heading("Prompts");
                    ui.horizontal(|ui| {
                        if ui.button("Refresh list").clicked() {
                            self.refresh_prompt_files();
                        }
                        if let Ok(dir) = config::prompts_dir() {
                            ui.label(format!("Location: {}", dir.display()));
                        }
                    });

                    {
                        let mut active = self.config.prompts.active_prompt_name.clone();
                        ui.horizontal(|ui| {
                            ui.label("Active prompt");
                            egui::ComboBox::from_id_source("active-prompt")
                                .selected_text(active.clone().unwrap_or_else(|| "None".to_string()))
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(active.is_none(), "None").clicked() {
                                        active = None;
                                    }
                                    for name in &self.prompt_files {
                                        let is_selected = active.as_deref() == Some(name.as_str());
                                        if ui.selectable_label(is_selected, name).clicked() {
                                            active = Some(name.clone());
                                        }
                                    }
                                });
                        });
                        if active != self.config.prompts.active_prompt_name {
                            self.config.prompts.active_prompt_name = active;
                        }
                    }

                    {
                        let mut default_prompt = self.config.prompts.default_prompt_name.clone();
                        ui.horizontal(|ui| {
                            ui.label("Default prompt");
                            egui::ComboBox::from_id_source("default-prompt")
                                .selected_text(
                                    default_prompt.clone().unwrap_or_else(|| "None".to_string()),
                                )
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(default_prompt.is_none(), "None")
                                        .clicked()
                                    {
                                        default_prompt = None;
                                    }
                                    for name in &self.prompt_files {
                                        let is_selected =
                                            default_prompt.as_deref() == Some(name.as_str());
                                        if ui.selectable_label(is_selected, name).clicked() {
                                            default_prompt = Some(name.clone());
                                        }
                                    }
                                });
                        });
                        if default_prompt != self.config.prompts.default_prompt_name {
                            self.config.prompts.default_prompt_name = default_prompt;
                        }
                    }

                    ui.separator();
                    ui.heading("Prompt editor");
                    let mut editor_selection = self.prompt_editor_selected.clone();
                    egui::ComboBox::from_id_source("prompt-editor")
                        .selected_text(editor_selection.clone().unwrap_or_else(|| {
                            if self.prompt_files.is_empty() {
                                "Create new".to_string()
                            } else {
                                "Select prompt".to_string()
                            }
                        }))
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(editor_selection.is_none(), "Create new")
                                .clicked()
                            {
                                editor_selection = None;
                            }
                            for name in &self.prompt_files {
                                let selected = editor_selection.as_deref() == Some(name.as_str());
                                if ui.selectable_label(selected, name).clicked() {
                                    editor_selection = Some(name.clone());
                                }
                            }
                        });
                    if editor_selection != self.prompt_editor_selected {
                        self.load_prompt_into_editor(editor_selection.clone());
                        if editor_selection.is_some() {
                            self.new_prompt_name.clear();
                        }
                    }

                    if self.prompt_editor_selected.is_none() {
                        ui.label("File name");
                        ui.text_edit_singleline(&mut self.new_prompt_name);
                    } else if let Some(name) = &self.prompt_editor_selected {
                        ui.label(format!("Editing: {name}"));
                    }

                    let editor_response =
                        egui::TextEdit::multiline(&mut self.prompt_editor_content)
                            .desired_rows(8)
                            .hint_text("Write prompt instructions here...")
                            .show(ui);
                    if editor_response.response.changed() {
                        self.prompt_editor_dirty = true;
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Save Prompt").clicked() {
                            self.save_current_prompt();
                        }
                        if let Some(_) = self.prompt_editor_selected {
                            if ui.button("Reload").clicked() {
                                let selected = self.prompt_editor_selected.clone();
                                self.load_prompt_into_editor(selected);
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_selected_prompt();
                            }
                        }
                    });

                    if self.prompt_editor_dirty {
                        ui.label(
                            RichText::new("Unsaved changes")
                                .color(Color32::from_rgb(255, 220, 120)),
                        );
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
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

        // Set transparent background with configurable opacity
        let opacity = self.config.ui.opacity;
        let frame_bg = egui::Color32::from_rgba_premultiplied(22, 22, 22, (255.0 * opacity) as u8);
        let mut style = (*ctx.style()).clone();
        style.visuals.window_fill = frame_bg;
        style.visuals.panel_fill = frame_bg;

        // Customize colors to match plan.md theme
        style.visuals.widgets.noninteractive.bg_fill =
            egui::Color32::from_rgba_premultiplied(30, 30, 30, (255.0 * opacity) as u8);
        style.visuals.widgets.inactive.bg_fill =
            egui::Color32::from_rgba_premultiplied(43, 102, 246, (255.0 * opacity * 0.3) as u8);
        style.visuals.widgets.hovered.bg_fill =
            egui::Color32::from_rgba_premultiplied(43, 102, 246, (255.0 * opacity * 0.5) as u8);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(43, 102, 246);

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

fn spawn_analyze_worker(
    runtime: &Handle,
    client: Arc<OpenAIClient>,
    mut requests: UnboundedReceiver<AnalyzeRequest>,
    events: UnboundedSender<AppEvent>,
    stream_tx: UnboundedSender<(Uuid, StreamEvent)>,
) {
    let events_clone = events.clone();
    runtime.spawn(async move {
        while let Some(request) = requests.recv().await {
            let request_id = request.request_id;
            let _ = events_clone.send(AppEvent::AnalysisStarted { request_id });
            match client.analyze_stream(request, stream_tx.clone()).await {
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
    Status {
        text: String,
        kind: StatusKind,
        duration: Option<Duration>,
    },
}
