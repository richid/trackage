pub mod fedex;

use crate::db::Package;
use anyhow::Result;
use std::collections::HashMap;
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

    pub fn register(&mut self, courier: &str, client: Box<dyn CourierClient>) {
        self.clients.insert(courier.to_string(), client);
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
