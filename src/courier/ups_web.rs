use super::{CourierClient, CourierStatus};
use crate::db::{Package, PackageStatus};
use crate::util::parse_date_yyyymmdd;
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{self, HeaderMap, HeaderValue};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

const TRACK_PAGE_URL: &str = "https://www.ups.com/track";
const TRACK_API_URL: &str = "https://webapis.ups.com/track/api/Track/GetStatus?loc=en_US";
const XSRF_COOKIE_NAME: &str = "X-XSRF-TOKEN-ST";

fn browser_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    h.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br, zstd"));
    h.insert(header::DNT, HeaderValue::from_static("1"));
    h.insert("Sec-GPC", HeaderValue::from_static("1"));
    h.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    h
}

pub struct UpsWebClient {
    client: Client,
}

impl UpsWebClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:147.0) Gecko/20100101 Firefox/147.0")
            .default_headers(browser_headers())
            .build()
            .expect("Failed to build UPS web HTTP client");

        Self { client }
    }

    /// Load the UPS tracking page to establish session cookies (including XSRF token).
    fn establish_session(&self, tracking_number: &str) -> Result<String> {
        let url = format!(
            "{}?loc=en_US&tracknum={}&requester=ST/trackdetails",
            TRACK_PAGE_URL, tracking_number
        );

        debug!(
            tracking_number = tracking_number,
            url = %url,
            "UPS web: establishing session"
        );

        let start = Instant::now();
        let resp = self.client
            .get(&url)
            .header(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Upgrade-Insecure-Requests", "1")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "none")
            .header("Pragma", "no-cache")
            .send()
            .context("UPS web: session request failed")?;
        let elapsed = start.elapsed();

        debug!(
            tracking_number = tracking_number,
            status = %resp.status(),
            elapsed_ms = elapsed.as_millis() as u64,
            "UPS web: session response received"
        );

        // Extract the XSRF token from cookies
        let xsrf_token = resp.cookies()
            .find(|c| c.name() == XSRF_COOKIE_NAME)
            .map(|c| c.value().to_string());

        // Consume the body to release the connection
        let _ = resp.text();

        match xsrf_token {
            Some(token) => {
                debug!(
                    tracking_number = tracking_number,
                    "UPS web: XSRF token acquired"
                );
                Ok(token)
            }
            None => {
                anyhow::bail!("UPS web: no XSRF token cookie found in session response")
            }
        }
    }
}

impl CourierClient for UpsWebClient {
    fn check_status(&self, package: &Package) -> Result<Vec<CourierStatus>> {
        // Step 1: Establish session and get XSRF token
        let xsrf_token = match self.establish_session(&package.tracking_number) {
            Ok(token) => token,
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    "UPS web: failed to establish session"
                );
                return Ok(vec![]);
            }
        };

        // Step 2: POST to the tracking API with session cookies and XSRF token
        let client_url = format!(
            "https://www.ups.com/track?loc=en_US&tracknum={}&requester=ST/trackdetails",
            package.tracking_number
        );
        let payload = serde_json::json!({
            "Locale": "en_US",
            "TrackingNumber": [&package.tracking_number],
            "Requester": "st/trackdetails",
            "returnToValue": "",
            "ClientUrl": client_url,
            "isBarcodeScanned": false,
        });

        debug!(
            tracking_number = %package.tracking_number,
            url = TRACK_API_URL,
            payload = %payload,
            "UPS web: tracking API request"
        );

        let start = Instant::now();
        let result = self.client
            .post(TRACK_API_URL)
            .header(header::ACCEPT, "application/json, text/plain, */*")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, "https://www.ups.com")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-site")
            .header("X-XSRF-TOKEN", &xsrf_token)
            .body(payload.to_string())
            .send();
        let elapsed = start.elapsed();

        let response = match result {
            Ok(resp) => {
                debug!(
                    tracking_number = %package.tracking_number,
                    status = %resp.status(),
                    elapsed_ms = elapsed.as_millis() as u64,
                    "UPS web: tracking API response received"
                );
                resp
            }
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "UPS web: tracking API request failed"
                );
                return Ok(vec![]);
            }
        };

        let body_text = match response.text() {
            Ok(text) => text,
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    "UPS web: failed to read tracking API response body"
                );
                return Ok(vec![]);
            }
        };

        debug!(
            tracking_number = %package.tracking_number,
            body = %body_text,
            "UPS web: tracking API response body"
        );

        let body: serde_json::Value = match serde_json::from_str(&body_text) {
            Ok(json) => json,
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    body = %body_text,
                    "UPS web: failed to parse tracking API response as JSON"
                );
                return Ok(vec![]);
            }
        };

        let details = &body["trackDetails"][0];
        let status_code = details["packageStatusType"].as_str();

        match status_code {
            Some(code) => {
                let mapped = map_status_code(code);

                // Parse scheduled delivery date from the raw "sdd" field (YYYYMMDD → YYYY-MM-DD)
                let estimated_arrival_date = details["sdd"]
                    .as_str()
                    .and_then(parse_date_yyyymmdd);

                info!(
                    tracking_number = %package.tracking_number,
                    ups_code = code,
                    mapped_status = %mapped,
                    activity_count = details["shipmentProgressActivities"].as_array().map_or(0, |a| a.len()),
                    "UPS web: status retrieved"
                );

                // Build a CourierStatus for each activity in shipmentProgressActivities.
                // Activities are returned newest-first; we reverse so oldest is first,
                // meaning the last entry (most recent) determines the package's current status.
                let mut statuses = Vec::new();

                if let Some(activities) = details["shipmentProgressActivities"].as_array() {
                    for (i, activity) in activities.iter().rev().enumerate() {
                        let is_latest = i == activities.len() - 1;

                        let description = activity["activityScan"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.trim().to_string());

                        let location = activity["location"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());

                        // Use GMT-normalized fields for proper UTC timestamps
                        let checked_at = match (activity["gmtDate"].as_str(), activity["gmtTime"].as_str()) {
                            (Some(gd), Some(gt)) if gd.len() == 8 => {
                                Some(format!("{}-{}-{}T{}Z", &gd[0..4], &gd[4..6], &gd[6..8], gt))
                            }
                            _ => None,
                        };

                        // Use the overall package status for the most recent activity,
                        // in_transit for all historical activities
                        let status = if is_latest {
                            mapped
                        } else {
                            PackageStatus::InTransit
                        };

                        statuses.push(CourierStatus {
                            status: status.to_string(),
                            estimated_arrival_date: estimated_arrival_date.clone(),
                            last_known_location: location,
                            description,
                            checked_at,
                        });
                    }
                }

                // If no activities were found, still return the overall status
                if statuses.is_empty() {
                    statuses.push(CourierStatus {
                        status: mapped.to_string(),
                        estimated_arrival_date,
                        last_known_location: None,
                        description: details["packageStatus"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string()),
                        checked_at: None,
                    });
                }

                Ok(statuses)
            }
            None => {
                warn!(
                    tracking_number = %package.tracking_number,
                    response = %body,
                    "UPS web: no status code in response"
                );
                Ok(vec![])
            }
        }
    }
}

fn map_status_code(code: &str) -> PackageStatus {
    match code {
        "D" => PackageStatus::Delivered,
        "M" | "P" => PackageStatus::Waiting,
        _ => PackageStatus::InTransit,
    }
}
