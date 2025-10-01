use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            model: default_model(),
            temperature: default_temperature(),
            max_output_tokens: Some(2048),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    #[serde(default = "CaptureSettings::default_attach_screenshots")]
    pub attach_screenshots: bool,
    #[serde(default = "CaptureSettings::default_hide_before_capture")]
    pub hide_before_capture: bool,
    #[serde(default = "CaptureSettings::default_capture_mode")]
    pub mode: CaptureMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    ActiveMonitor,
    Primary,
    Region,
}

impl Default for CaptureMode {
    fn default() -> Self {
        CaptureMode::ActiveMonitor
    }
}

impl CaptureSettings {
    fn default_attach_screenshots() -> bool {
        true
    }

    fn default_hide_before_capture() -> bool {
        true
    }

    fn default_capture_mode() -> CaptureMode {
        CaptureMode::ActiveMonitor
    }
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            attach_screenshots: Self::default_attach_screenshots(),
            hide_before_capture: Self::default_hide_before_capture(),
            mode: Self::default_capture_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionLanguage {
    En,
    Zh,
}

impl Default for TranscriptionLanguage {
    fn default() -> Self {
        TranscriptionLanguage::En
    }
}

fn default_transcription_model() -> String {
    "whisper-1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub realtime: bool,
    #[serde(default)]
    pub language: TranscriptionLanguage,
    #[serde(default = "default_transcription_model")]
    pub model: String,
}

impl Default for TranscriptionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            realtime: true,
            language: TranscriptionLanguage::En,
            model: default_transcription_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "HotkeyConfig::default_toggle_ask")]
    pub toggle_ask_panel: String,
    #[serde(default = "HotkeyConfig::default_toggle_record")]
    pub toggle_record: String,
    #[serde(default = "HotkeyConfig::default_toggle_hide")]
    pub toggle_hide: String,
    #[serde(default = "HotkeyConfig::default_clear_session")]
    pub clear_session: String,
    #[serde(default = "HotkeyConfig::default_capture")]
    pub capture_screenshot: String,
}

impl HotkeyConfig {
    fn default_toggle_ask() -> String {
        "Ctrl+Enter".into()
    }

    fn default_toggle_record() -> String {
        "Ctrl+Shift+Enter".into()
    }

    fn default_toggle_hide() -> String {
        "Ctrl+\\".into()
    }

    fn default_clear_session() -> String {
        "Ctrl+R".into()
    }

    fn default_capture() -> String {
        "Ctrl+Shift+S".into()
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle_ask_panel: Self::default_toggle_ask(),
            toggle_record: Self::default_toggle_record(),
            toggle_hide: Self::default_toggle_hide(),
            clear_session: Self::default_clear_session(),
            capture_screenshot: Self::default_capture(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSettings {
    #[serde(default)]
    pub default_prompt_name: Option<String>,
    #[serde(default)]
    pub active_prompt_name: Option<String>,
}

impl Default for PromptSettings {
    fn default() -> Self {
        Self {
            default_prompt_name: None,
            active_prompt_name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeVariant {
    Light,
    Dark,
    System,
}

impl Default for ThemeVariant {
    fn default() -> Self {
        ThemeVariant::Dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    #[serde(default = "UiSettings::default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub compact_mode: bool,
    #[serde(default)]
    pub theme: ThemeVariant,
}

impl UiSettings {
    fn default_opacity() -> f32 {
        0.92
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            opacity: Self::default_opacity(),
            compact_mode: false,
            theme: ThemeVariant::Dark,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub openai: OpenAIConfig,
    #[serde(default)]
    pub capture: CaptureSettings,
    #[serde(default)]
    pub transcription: TranscriptionSettings,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub prompts: PromptSettings,
    #[serde(default)]
    pub ui: UiSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            openai: OpenAIConfig::default(),
            capture: CaptureSettings::default(),
            transcription: TranscriptionSettings::default(),
            hotkeys: HotkeyConfig::default(),
            prompts: PromptSettings::default(),
            ui: UiSettings::default(),
        }
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("com", "ghost", "ghost-ai")
        .context("unable to determine platform-specific config directory")
}

pub fn config_dir() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.config_dir();
    if !dir.exists() {
        fs::create_dir_all(dir).context("failed to create config directory")?;
    }
    Ok(dir.to_path_buf())
}

pub fn data_dir() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.data_dir();
    if !dir.exists() {
        fs::create_dir_all(dir).context("failed to create data directory")?;
    }
    Ok(dir.to_path_buf())
}

pub fn logs_dir() -> Result<PathBuf> {
    let dir = data_dir()?.join("logs");
    if !dir.exists() {
        fs::create_dir_all(&dir).context("failed to create logs directory")?;
    }
    Ok(dir)
}

pub fn prompts_dir() -> Result<PathBuf> {
    let dir = data_dir()?.join("prompts");
    if !dir.exists() {
        fs::create_dir_all(&dir).context("failed to create prompts directory")?;
    }
    Ok(dir)
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load_or_default() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;
    let cfg = serde_json::from_str::<AppConfig>(&raw)
        .with_context(|| format!("failed to parse config at {}", path.display()))?;
    Ok(cfg)
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let json = serde_json::to_string_pretty(cfg).context("failed to serialize config")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write config file at {}", path.display()))?;
    Ok(())
}
