use super::{CourierClient, CourierStatus};
use crate::db::{Package, PackageStatus};
use anyhow::Result;
use std::time::Duration;
use tracing::{debug, info, warn};

const TRACK_URL: &str = "https://www.ups.com/track/api/Track/GetStatus?loc=en_US";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct UpsWebClient {
    agent: ureq::Agent,
}

impl UpsWebClient {
    pub fn new() -> Self {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(60)))
                .user_agent(USER_AGENT)
                .build(),
        );
        Self { agent }
    }
}

impl CourierClient for UpsWebClient {
    fn check_status(&self, package: &Package) -> Result<Vec<CourierStatus>> {
        let payload = serde_json::json!({
            "Locale": "en_US",
            "TrackingNumber": [&package.tracking_number],
        });

        debug!(
            tracking_number = %package.tracking_number,
            url = TRACK_URL,
            payload = %payload,
            "UPS web tracking request"
        );

        let result = self.agent
            .post(TRACK_URL)
            .header("Content-Type", "application/json")
            .send(payload.to_string().as_bytes());

        let response = match result {
            Ok(resp) => resp,
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    "UPS web tracking request failed"
                );
                return Ok(vec![]);
            }
        };

        let body: serde_json::Value = match response.into_body().read_json() {
            Ok(json) => json,
            Err(e) => {
                warn!(
                    tracking_number = %package.tracking_number,
                    error = %e,
                    "Failed to parse UPS web tracking response"
                );
                return Ok(vec![]);
            }
        };

        debug!(
            tracking_number = %package.tracking_number,
            response = %body,
            "UPS web tracking response"
        );

        let details = &body["trackDetails"][0];

        let status_code = details["packageStatusType"].as_str();

        match status_code {
            Some(code) => {
                let mapped = map_status_code(code);

                let estimated_arrival_date = details["scheduledDeliveryDate"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                let last_known_location = details["lastLocation"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                let description = details["packageStatus"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                info!(
                    tracking_number = %package.tracking_number,
                    ups_code = code,
                    mapped_status = %mapped,
                    "UPS web status retrieved"
                );

                Ok(vec![CourierStatus {
                    status: mapped.to_string(),
                    estimated_arrival_date,
                    last_known_location,
                    description,
                    checked_at: None,
                }])
            }
            None => {
                warn!(
                    tracking_number = %package.tracking_number,
                    response = %body,
                    "No status code in UPS web response"
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
