use super::CourierClient;
use crate::config::UspsConfig;
use crate::db::Package;
use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const TOKEN_URL: &str = "https://apis.usps.com/oauth2/v3/token";
const TRACK_URL: &str = "https://apis.usps.com/tracking/v3/tracking/";

pub struct UspsClient {
    client_id: String,
    client_secret: String,
    token: Mutex<Option<(String, Instant)>>,
}

impl UspsClient {
    pub fn new(config: &UspsConfig) -> Self {
        Self {
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            token: Mutex::new(None),
        }
    }

    fn get_token(&self) -> Result<String> {
        let mut guard = self.token.lock().unwrap();

        if let Some((ref token, expiry)) = *guard {
            if Instant::now() < expiry {
                return Ok(token.clone());
            }
        }

        let (token, ttl) = self.fetch_token()?;
        let expiry = Instant::now() + ttl;
        *guard = Some((token.clone(), expiry));
        Ok(token)
    }

    fn fetch_token(&self) -> Result<(String, Duration)> {
        debug!("Fetching new USPS OAuth token");

        let request_body = json!({
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "grant_type": "client_credentials"
        });

        let response = ureq::post(TOKEN_URL)
            .header("Content-Type", "application/json")
            .send_json(&request_body)
            .context("USPS OAuth token request failed")?;

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse USPS token response")?;

        let access_token = body["access_token"]
            .as_str()
            .context("Missing access_token in USPS response")?
            .to_string();

        let expires_in = body["expires_in"]
            .as_u64()
            .context("Missing expires_in in USPS response")?;

        // Subtract 60 seconds buffer to avoid using an about-to-expire token
        let ttl = Duration::from_secs(expires_in.saturating_sub(60));

        debug!(expires_in_secs = expires_in, "USPS OAuth token acquired");

        Ok((access_token, ttl))
    }

    fn map_status_category(category: &str) -> &'static str {
        match category {
            "Delivered" => "delivered",
            "Pre-Shipment" => "waiting",
            _ => "in_transit",
        }
    }
}

impl CourierClient for UspsClient {
    fn check_status(&self, package: &Package) -> Result<Option<String>> {
        let token = self.get_token()?;

        let url = format!("{TRACK_URL}{}", package.tracking_number);

        let response = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .context("USPS track request failed")?;

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse USPS track response")?;

        // Check for error envelope
        if let Some(error) = body["error"].as_object() {
            let code = error.get("code").and_then(|c| c.as_str()).unwrap_or("");
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
            warn!(
                tracking_number = %package.tracking_number,
                error_code = code,
                error_message = message,
                "USPS tracking error"
            );
            return Ok(None);
        }

        let status_category = body["statusCategory"].as_str();

        match status_category {
            Some(category) => {
                let mapped = Self::map_status_category(category);
                debug!(
                    tracking_number = %package.tracking_number,
                    usps_category = category,
                    mapped_status = mapped,
                    "USPS status retrieved"
                );
                Ok(Some(mapped.to_string()))
            }
            None => {
                debug!(
                    tracking_number = %package.tracking_number,
                    "No statusCategory in USPS response"
                );
                Ok(None)
            }
        }
    }
}
