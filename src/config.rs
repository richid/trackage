use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub email: EmailConfig,
}

#[derive(Debug, Deserialize)]
pub struct EmailConfig {
    #[serde(default = "default_check_interval")]
    pub check_interval_seconds: u64,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_folder")]
    pub folder: String,

    pub server: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

fn default_check_interval() -> u64 {
    300
}

fn default_port() -> u16 {
    993
}

fn default_folder() -> String {
    "INBOX".to_string()
}

/// Load configuration from config.toml and environment variables
pub fn load() -> Config {
    Figment::new()
        .merge(Toml::file("config.toml"))
        // Use double-underscore nesting for snake_case keys
        .merge(Env::prefixed("TRACKAGE_").split("__"))
        .extract()
        .expect("Failed to load configuration")
}

/// Validate configuration and return a user-friendly error
pub fn validate(config: &Config) -> Result<(), String> {
    let email = &config.email;

    if email.server.is_none() {
        return Err("email.server is required".into());
    }

    if email.username.is_none() {
        return Err("email.username is required".into());
    }

    if email.password.is_none() {
        return Err("email.password is required".into());
    }

    if email.check_interval_seconds == 0 {
        return Err("email.check_interval_seconds must be greater than 0".into());
    }

    Ok(())
}

/// A sanitized view of EmailConfig safe for logging
#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedEmailConfig {
    pub check_interval_seconds: u64,
    pub port: u16,
    pub folder: String,
    pub server: String,
    pub username: String,
    pub password: String,
}

impl EmailConfig {
    pub fn sanitized_for_log(&self) -> SanitizedEmailConfig {
        SanitizedEmailConfig {
            check_interval_seconds: self.check_interval_seconds,
            port: self.port,
            folder: self.folder.clone(),
            server: self.server.clone().unwrap_or_else(|| "<not set>".into()),
            username: self.username.clone().unwrap_or_else(|| "<not set>".into()),
            password: if self.password.is_some() {
                "******".into()
            } else {
                "<not set>".into()
            },
        }
    }
}