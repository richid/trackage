use super::{CourierClient, CourierStatus};
use crate::config::UspsConfig;
use crate::db::{Package, PackageStatus};
use anyhow::{Context, Result};
use regex::Regex;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Substrings matched case-insensitively in USPS eventSummary text to determine status.
const SUMMARY_KEYWORD_DELIVERED: &str = "delivered";
const SUMMARY_KEYWORD_LABEL_CREATED: &str = "shipping label created";
const SUMMARY_KEYWORD_AWAITING_ITEM: &str = "awaiting item";

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

    fn map_status_category(category: &str) -> PackageStatus {
        match category {
            "Delivered" => PackageStatus::Delivered,
            "Pre-Shipment" => PackageStatus::Waiting,
            _ => PackageStatus::InTransit,
        }
    }

    fn map_summary_status(text: &str) -> PackageStatus {
        let lower = text.to_lowercase();
        if lower.contains(SUMMARY_KEYWORD_DELIVERED) {
            PackageStatus::Delivered
        } else if lower.contains(SUMMARY_KEYWORD_LABEL_CREATED)
            || lower.contains(SUMMARY_KEYWORD_AWAITING_ITEM)
        {
            PackageStatus::Waiting
        } else {
            PackageStatus::InTransit
        }
    }

    fn extract_date(text: &str) -> Option<String> {
        // Pattern 1: MM/DD/YYYY, H:MM am/pm
        let re_slash = Regex::new(
            r"(\d{1,2})/(\d{1,2})/(\d{4}),\s+(\d{1,2}):(\d{2})\s+(am|pm)"
        ).unwrap();
        if let Some(caps) = re_slash.captures(text) {
            let month: u32 = caps[1].parse().ok()?;
            let day: u32 = caps[2].parse().ok()?;
            let year: u32 = caps[3].parse().ok()?;
            let mut hour: u32 = caps[4].parse().ok()?;
            let minute: u32 = caps[5].parse().ok()?;
            let ampm = &caps[6];

            if ampm == "pm" && hour != 12 {
                hour += 12;
            } else if ampm == "am" && hour == 12 {
                hour = 0;
            }

            return Some(format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:00"));
        }

        // Pattern 2: "Month Day, Year" with optional "at H:MM am/pm on"
        let months = [
            "january", "february", "march", "april", "may", "june",
            "july", "august", "september", "october", "november", "december",
        ];

        let re_long = Regex::new(
            r"(?i)(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{1,2}),\s+(\d{4})"
        ).unwrap();

        if let Some(caps) = re_long.captures(text) {
            let month_name = caps[1].to_lowercase();
            let month = months.iter().position(|m| *m == month_name)? as u32 + 1;
            let day: u32 = caps[2].parse().ok()?;
            let year: u32 = caps[3].parse().ok()?;

            // Look for optional time: "at H:MM am/pm"
            let re_time = Regex::new(r"(\d{1,2}):(\d{2})\s+(am|pm)").unwrap();
            let (hour, minute) = if let Some(tcaps) = re_time.captures(text) {
                let mut h: u32 = tcaps[1].parse().ok()?;
                let m: u32 = tcaps[2].parse().ok()?;
                let ampm = &tcaps[3];
                if ampm == "pm" && h != 12 {
                    h += 12;
                } else if ampm == "am" && h == 12 {
                    h = 0;
                }
                (h, m)
            } else {
                (0, 0)
            };

            return Some(format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:00"));
        }

        None
    }

    fn extract_location(text: &str) -> Option<String> {
        // Pattern 1: "City, ST" with comma separator
        let re = Regex::new(r"([A-Z][A-Za-z]+(?:\s+[A-Z][A-Za-z]+)*),\s+([A-Z]{2})\b").unwrap();
        if let Some(caps) = re.captures(text) {
            return Some(format!("{}, {}", &caps[1], &caps[2]));
        }

        // Pattern 2: "CITY ST" without comma (e.g., USPS facility names like
        // "OKLAHOMA CITY OK DISTRIBUTION CENTER"). Look in the last comma-separated
        // segment to avoid false positives from the description portion.
        let last_segment = text.rsplit(',').next()?.trim();
        let re2 = Regex::new(r"([A-Z][A-Za-z]+(?:\s+[A-Z][A-Za-z]+)*)\s+([A-Z]{2})\b").unwrap();
        re2.captures(last_segment).map(|caps| format!("{}, {}", &caps[1], &caps[2]))
    }

    fn parse_event_summary(summary: &str) -> CourierStatus {
        CourierStatus {
            status: Self::map_summary_status(summary).to_string(),
            checked_at: Self::extract_date(summary),
            last_known_location: Self::extract_location(summary),
            description: Some(summary.to_string()),
            estimated_arrival_date: None,
        }
    }
}

impl CourierClient for UspsClient {
    fn check_status(&self, package: &Package) -> Result<Vec<CourierStatus>> {
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
            return Ok(vec![]);
        }

        let status_category = body["statusCategory"].as_str();

        // Structured path: statusCategory is present
        if let Some(category) = status_category {
            let mapped = Self::map_status_category(category);

            let estimated_arrival_date = body["expectedDeliveryDate"]
                .as_str()
                .map(|s| s.to_string());

            let last_known_location = body["trackingEvents"]
                .as_array()
                .and_then(|events| events.first())
                .and_then(|event| {
                    event["eventCity"].as_str().map(|city| {
                        match event["eventState"].as_str() {
                            Some(state) => format!("{city}, {state}"),
                            None => city.to_string(),
                        }
                    })
                });

            debug!(
                tracking_number = %package.tracking_number,
                usps_category = category,
                mapped_status = %mapped,
                "USPS status retrieved"
            );
            return Ok(vec![CourierStatus {
                status: mapped.to_string(),
                estimated_arrival_date,
                last_known_location,
                description: None,
                checked_at: None,
            }]);
        }

        // Fallback path: parse eventSummaries
        if let Some(summaries) = body["eventSummaries"].as_array() {
            debug!(
                tracking_number = %package.tracking_number,
                count = summaries.len(),
                "Parsing USPS eventSummaries fallback"
            );

            let statuses: Vec<CourierStatus> = summaries
                .iter()
                .rev() // reverse: oldest first so newest gets highest DB id
                .filter_map(|s| s.as_str())
                .map(Self::parse_event_summary)
                .collect();

            if !statuses.is_empty() {
                return Ok(statuses);
            }
        }

        debug!(
            tracking_number = %package.tracking_number,
            "No statusCategory or eventSummaries in USPS response"
        );
        Ok(vec![])
    }
}
