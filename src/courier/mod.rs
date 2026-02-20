pub mod fedex;
pub mod ups;
pub mod usps;

use crate::db::Package;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use tracing::debug;

pub trait CourierClient: Send {
    fn check_status(&self, package: &Package) -> Result<Option<String>>;
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
    fn check_status(&self, package: &Package) -> Result<Option<String>> {
        match self.clients.get(&package.courier) {
            Some(client) => client.check_status(package),
            None => {
                debug!(
                    courier = %package.courier,
                    tracking_number = %package.tracking_number,
                    "No courier client registered for this courier"
                );
                Ok(None)
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

impl fmt::Display for CourierCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CourierCode::FedEx => write!(f, "fedex"),
            CourierCode::UPS => write!(f, "ups"),
            CourierCode::USPS => write!(f, "usps"),
        }
    }
}

impl FromStr for CourierCode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "fedex" => Ok(CourierCode::FedEx),
            "ups"   => Ok(CourierCode::UPS),
            "usps"  => Ok(CourierCode::USPS),
            other   => Err(anyhow::anyhow!("Unknown courier code: {other}")),
        }
    }
}
