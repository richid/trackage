use super::CourierClient;
use crate::config::UpsConfig;
use crate::db::Package;
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const TOKEN_URL: &str = "https://onlinetools.ups.com/security/v1/oauth/token";
const TRACK_URL: &str = "https://onlinetools.ups.com/api/track/v1/details/";

pub struct UpsClient {
    client_id: String,
    client_secret: String,
    token: Mutex<Option<(String, Instant)>>,
}

impl UpsClient {
    pub fn new(config: &UpsConfig) -> Self {
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
        debug!("Fetching new UPS OAuth token");

        let credentials = BASE64.encode(format!("{}:{}", self.client_id, self.client_secret));

        let response = ureq::post(TOKEN_URL)
            .header("Authorization", &format!("Basic {credentials}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send("grant_type=client_credentials".as_bytes())
            .context("UPS OAuth token request failed")?;

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse UPS token response")?;

        let access_token = body["access_token"]
            .as_str()
            .context("Missing access_token in UPS response")?
            .to_string();

        // UPS returns expires_in as a string, not a number
        let expires_in: u64 = body["expires_in"]
            .as_str()
            .context("Missing expires_in in UPS response")?
            .parse()
            .context("Failed to parse UPS expires_in as integer")?;

        // Subtract 60 seconds buffer to avoid using an about-to-expire token
        let ttl = Duration::from_secs(expires_in.saturating_sub(60));

        debug!(expires_in_secs = expires_in, "UPS OAuth token acquired");

        Ok((access_token, ttl))
    }

    fn map_status_code(code: &str) -> &'static str {
        match code {
            "D" => "delivered",
            "M" | "P" => "waiting",
            _ => "in_transit",
        }
    }
}

impl CourierClient for UpsClient {
    fn check_status(&self, package: &Package) -> Result<Option<String>> {
        let token = self.get_token()?;

        let url = format!("{TRACK_URL}{}", package.tracking_number);
        let trans_id = format!("trackage-{}", chrono::Utc::now().timestamp());

        let result = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("transId", &trans_id)
            .header("transactionSrc", "trackage")
            .call();

        let response = match result {
            Ok(resp) => resp,
            Err(ureq::Error::StatusCode(404)) => {
                debug!(
                    tracking_number = %package.tracking_number,
                    "UPS tracking number not found"
                );
                return Ok(None);
            }
            Err(e) => return Err(e).context("UPS track request failed"),
        };

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse UPS track response")?;

        // Navigate the UPS response structure:
        // trackResponse.shipment[0].package[0].currentStatus.code
        let status_code = body["trackResponse"]["shipment"][0]["package"][0]["currentStatus"]
            ["code"]
            .as_str();

        match status_code {
            Some(code) => {
                let mapped = Self::map_status_code(code);
                debug!(
                    tracking_number = %package.tracking_number,
                    ups_code = code,
                    mapped_status = mapped,
                    "UPS status retrieved"
                );
                Ok(Some(mapped.to_string()))
            }
            None => {
                warn!(
                    tracking_number = %package.tracking_number,
                    "No status code in UPS response"
                );
                Ok(None)
            }
        }
    }
}
