use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub email: EmailConfig,

    #[serde(default)]
    pub database: DatabaseConfig,

    #[serde(default)]
    pub status: StatusPollerConfig,

    #[serde(default)]
    pub courier: CourierConfig,

    #[serde(default)]
    pub web: WebConfig,
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

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StatusPollerConfig {
    #[serde(default = "default_status_check_interval")]
    pub check_interval_seconds: u64,
}

impl Default for StatusPollerConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: default_status_check_interval(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CourierConfig {
    pub fedex: Option<FedexConfig>,
    pub ups: Option<UpsConfig>,
    pub usps: Option<UspsConfig>,
}

impl Default for CourierConfig {
    fn default() -> Self {
        Self {
            fedex: None,
            ups: None,
            usps: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FedexConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct UspsConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct WebConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_web_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_web_port(),
        }
    }
}

fn default_web_port() -> u16 {
    3000
}

fn default_status_check_interval() -> u64 {
    3600
}

fn default_db_path() -> String {
    "trackage.db".to_string()
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

const MASKED: &str = "******";
const NOT_SET: &str = "<not set>";

fn mask_option(opt: &Option<String>) -> &'static str {
    if opt.is_some() { MASKED } else { NOT_SET }
}

/// A sanitized view of the full configuration, safe for logging.
#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedConfig {
    pub email: SanitizedEmailConfig,
    pub database: SanitizedDatabaseConfig,
    pub status: SanitizedStatusPollerConfig,
    pub courier: SanitizedCourierConfig,
    pub web: SanitizedWebConfig,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedEmailConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: &'static str,
    pub folder: String,
    pub check_interval_seconds: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedDatabaseConfig {
    pub path: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedStatusPollerConfig {
    pub check_interval_seconds: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedCourierConfig {
    pub fedex: Option<SanitizedCourierCredentials>,
    pub ups: Option<SanitizedCourierCredentials>,
    pub usps: Option<SanitizedCourierCredentials>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedCourierCredentials {
    pub client_id: String,
    pub client_secret: &'static str,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SanitizedWebConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Config {
    pub fn sanitized_for_log(&self) -> SanitizedConfig {
        SanitizedConfig {
            email: SanitizedEmailConfig {
                server: self.email.server.clone().unwrap_or_else(|| NOT_SET.into()),
                port: self.email.port,
                username: self.email.username.clone().unwrap_or_else(|| NOT_SET.into()),
                password: mask_option(&self.email.password),
                folder: self.email.folder.clone(),
                check_interval_seconds: self.email.check_interval_seconds,
            },
            database: SanitizedDatabaseConfig {
                path: self.database.path.clone(),
            },
            status: SanitizedStatusPollerConfig {
                check_interval_seconds: self.status.check_interval_seconds,
            },
            courier: SanitizedCourierConfig {
                fedex: self.courier.fedex.as_ref().map(|c| SanitizedCourierCredentials {
                    client_id: c.client_id.clone(),
                    client_secret: MASKED,
                }),
                ups: self.courier.ups.as_ref().map(|c| SanitizedCourierCredentials {
                    client_id: c.client_id.clone(),
                    client_secret: MASKED,
                }),
                usps: self.courier.usps.as_ref().map(|c| SanitizedCourierCredentials {
                    client_id: c.client_id.clone(),
                    client_secret: MASKED,
                }),
            },
            web: SanitizedWebConfig {
                enabled: self.web.enabled,
                port: self.web.port,
            },
        }
    }
}
