pub mod fedex;
pub mod ups;
pub mod usps;

use crate::db::Package;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use tracing::warn;

pub struct CourierStatus {
    pub status: String,
    pub estimated_arrival_date: Option<String>,
    pub last_known_location: Option<String>,
    pub description: Option<String>,
    pub checked_at: Option<String>,
}

pub trait CourierClient: Send {
    fn check_status(&self, package: &Package) -> Result<Vec<CourierStatus>>;
}

pub struct CourierRouter {
    clients: HashMap<String, Box<dyn CourierClient>>,
}

impl CourierRouter {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn register(&mut self, courier_code: &CourierCode, client: Box<dyn CourierClient>) {
        self.clients.insert(courier_code.to_string(), client);
    }
}

impl CourierClient for CourierRouter {
    fn check_status(&self, package: &Package) -> Result<Vec<CourierStatus>> {
        match self.clients.get(&package.courier) {
            Some(client) => client.check_status(package),
            None => {
                warn!(
                    courier = %package.courier,
                    tracking_number = %package.tracking_number,
                    "No client registered for this courier"
                );
                Ok(vec![])
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CourierCode {
    FedEx,
    UPS,
    USPS,
}

impl CourierCode {
    /// Human-readable display name for UI use.
    pub fn display_name(&self) -> &'static str {
        match self {
            CourierCode::FedEx => "FedEx",
            CourierCode::UPS   => "UPS",
            CourierCode::USPS  => "USPS",
        }
    }
}

impl fmt::Display for CourierCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CourierCode::FedEx => write!(f, "fedex"),
            CourierCode::UPS   => write!(f, "ups"),
            CourierCode::USPS  => write!(f, "usps"),
        }
    }
}

impl FromStr for CourierCode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "fedex" | "FedEx" => Ok(CourierCode::FedEx),
            "ups"   | "UPS" => Ok(CourierCode::UPS),
            "usps"  | "United States Postal Service" => Ok(CourierCode::USPS),
            other => Err(anyhow::anyhow!("Unknown courier code: {other}")),
        }
    }
}
