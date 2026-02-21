use super::{CourierClient, CourierStatus};
use crate::config::FedexConfig;
use crate::db::Package;
use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const TOKEN_URL: &str = "https://apis-sandbox.fedex.com/oauth/token";
const TRACK_URL: &str = "https://apis-sandbox.fedex.com/track/v1/trackingnumbers";

pub struct FedexClient {
    client_id: String,
    client_secret: String,
    token: Mutex<Option<(String, Instant)>>,
}

impl FedexClient {
    pub fn new(config: &FedexConfig) -> Self {
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
        debug!("Fetching new FedEx OAuth token");

        let form_body = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}",
            self.client_id, self.client_secret
        );

        let response = ureq::post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(form_body.as_bytes())
            .context("FedEx OAuth token request failed")?;

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse FedEx token response")?;

        let access_token = body["access_token"]
            .as_str()
            .context("Missing access_token in FedEx response")?
            .to_string();

        let expires_in = body["expires_in"]
            .as_u64()
            .context("Missing expires_in in FedEx response")?;

        // Subtract 60 seconds buffer to avoid using an about-to-expire token
        let ttl = Duration::from_secs(expires_in.saturating_sub(60));

        debug!(expires_in_secs = expires_in, "FedEx OAuth token acquired");

        Ok((access_token, ttl))
    }

    fn map_status_code(code: &str) -> &'static str {
        match code {
            "DL" => "delivered",
            "OC" => "waiting",
            _ => "in_transit",
        }
    }
}

impl CourierClient for FedexClient {
    fn check_status(&self, package: &Package) -> Result<Option<CourierStatus>> {
        let token = self.get_token()?;

        let request_body = json!({
            "trackingInfo": [{
                "trackingNumberInfo": {
                    "trackingNumber": package.tracking_number
                }
            }],
            "includeDetailedScans": false
        });

        let response = ureq::post(TRACK_URL)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .send_json(&request_body)
            .context("FedEx track request failed")?;

        let body: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse FedEx track response")?;

        // Navigate the FedEx response structure:
        // output.completeTrackResults[0].trackResults[0].latestStatusDetail.code
        let track_result = &body["output"]["completeTrackResults"][0]["trackResults"][0];

        // Check for tracking-number-not-found errors
        if let Some(error) = track_result["error"].as_object() {
            let code = error.get("code").and_then(|c| c.as_str()).unwrap_or("");
            warn!(
                tracking_number = %package.tracking_number,
                error_code = code,
                "FedEx tracking error"
            );
            return Ok(None);
        }

        let status_code = track_result["latestStatusDetail"]["code"]
            .as_str();

        match status_code {
            Some(code) => {
                let mapped = Self::map_status_code(code);

                // Extract estimated delivery from dateAndTimes array
                let estimated_arrival_date = track_result["dateAndTimes"]
                    .as_array()
                    .and_then(|dates| {
                        dates.iter().find(|d| {
                            d["type"].as_str() == Some("ESTIMATED_DELIVERY")
                        })
                    })
                    .and_then(|d| d["dateTime"].as_str())
                    .map(|s| s.to_string());

                // Extract last known location from latestStatusDetail.scanLocation
                let scan_location = &track_result["latestStatusDetail"]["scanLocation"];
                let last_known_location = scan_location["city"].as_str().map(|city| {
                    match scan_location["stateOrProvinceCode"].as_str() {
                        Some(state) => format!("{city}, {state}"),
                        None => city.to_string(),
                    }
                });

                debug!(
                    tracking_number = %package.tracking_number,
                    fedex_code = code,
                    mapped_status = mapped,
                    "FedEx status retrieved"
                );
                Ok(Some(CourierStatus {
                    status: mapped.to_string(),
                    estimated_arrival_date,
                    last_known_location,
                }))
            }
            None => {
                debug!(
                    tracking_number = %package.tracking_number,
                    "No status code in FedEx response"
                );
                Ok(None)
            }
        }
    }
}
