use ghost_ai::config::{AppConfig, CaptureMode, ThemeVariant};

#[test]
fn default_openai_config_values_are_expected() {
    let config = AppConfig::default();

    assert!(config.openai.api_key.is_empty());
    assert_eq!(config.openai.base_url, "https://api.openai.com/v1");
    assert_eq!(config.openai.model, "gpt-4o-mini");
    assert!((config.openai.temperature - 0.7).abs() < f32::EPSILON);
    assert_eq!(config.openai.max_output_tokens, Some(2048));
}

#[test]
fn capture_defaults_enable_core_features() {
    let config = AppConfig::default();

    assert!(config.capture.attach_screenshots);
    assert!(config.capture.hide_before_capture);
    assert!(matches!(config.capture.mode, CaptureMode::ActiveMonitor));
}

#[test]
fn ui_defaults_are_dark_and_non_compact() {
    let config = AppConfig::default();

    assert!((config.ui.opacity - 0.92).abs() < f32::EPSILON);
    assert!(!config.ui.compact_mode);
    assert!(matches!(config.ui.theme, ThemeVariant::Dark));
}

#[test]
fn transcription_is_enabled_by_default() {
    let config = AppConfig::default();

    assert!(config.transcription.enabled);
    assert!(config.transcription.realtime);
    assert_eq!(config.transcription.model, "whisper-1");
}

#[test]
fn prompts_default_names_are_none() {
    let config = AppConfig::default();

    assert!(config.prompts.default_prompt_name.is_none());
    assert!(config.prompts.active_prompt_name.is_none());
}
