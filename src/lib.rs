pub mod app;
pub mod audio;
pub mod capture;
pub mod config;
pub mod hotkeys;
pub mod logging;
pub mod openai;
pub mod session;

pub use config::{AppConfig, OpenAIConfig};

#[cfg(test)]
mod tests {
    use super::config::AppConfig;

    #[test]
    fn default_config_matches_expected_values() {
        let config = AppConfig::default();
        assert!(config.openai.api_key.is_empty());
        assert_eq!(config.openai.base_url, "https://api.openai.com/v1");
        assert_eq!(config.openai.model, "gpt-4o-mini");
        assert!((config.openai.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.openai.max_output_tokens, Some(2048));
    }
}
