use crate::config::StatusPollerConfig;
use crate::courier::CourierClient;
use crate::db::{Database, Package, PackageStatus};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info};

pub struct StatusPoller {
    config: StatusPollerConfig,
    db: Box<dyn Database>,
    courier: Box<dyn CourierClient>,
    running: Arc<AtomicBool>,
}

impl StatusPoller {
    pub fn new(
        config: StatusPollerConfig,
        db: Box<dyn Database>,
        courier: Box<dyn CourierClient>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            db,
            courier,
            running,
        }
    }

    /// Run the poll loop. Blocks until the shutdown signal fires.
    pub fn run(mut self) {
        info!("Status poller starting");

        while self.running.load(Ordering::SeqCst) {
            self.poll_once();
            self.sleep();
        }

        info!("Status poller shutting down");
    }

    fn poll_once(&mut self) {
        let packages = match self.db.get_active_packages() {
            Ok(packages) => packages,
            Err(err) => {
                error!(error = %err, "Failed to query active packages");
                return;
            }
        };

        if packages.is_empty() {
            debug!("No active packages to check");
            return;
        }

        info!(count = packages.len(), "Checking active packages");

        for package in &packages {
            self.check_package(package);
        }
    }

    fn check_package(&mut self, package: &Package) {
        let result = match self.courier.check_status(package) {
            Ok(result) => result,
            Err(err) => {
                error!(
                    error = %err,
                    tracking_number = %package.tracking_number,
                    "Courier status check failed"
                );
                return;
            }
        };

        let Some(courier_status) = result else {
            info!(
                tracking_number = %package.tracking_number,
                "No status update available"
            );
            return;
        };

        let status = match PackageStatus::from_str(&courier_status.status) {
            Ok(s) => s,
            Err(err) => {
                error!(
                    error = %err,
                    tracking_number = %package.tracking_number,
                    status = %courier_status.status,
                    "Invalid status from courier"
                );
                return;
            }
        };

        if status != package.status {
            info!(
                tracking_number = %package.tracking_number,
                old_status = %package.status,
                new_status = %status,
                "Package status changed"
            );
        } else {
            info!(
                tracking_number = %package.tracking_number,
                "Updating status information"
            );
        }

        if let Err(err) = self.db.insert_package_status(
            package.id,
            &status,
            courier_status.estimated_arrival_date.as_deref(),
            courier_status.last_known_location.as_deref(),
        ) {
            error!(
                error = %err,
                tracking_number = %package.tracking_number,
                "Failed to insert package status history"
            );
        }
    }

    fn sleep(&self) {
        let mut slept = 0;
        while slept < self.config.check_interval_seconds && self.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
            slept += 1;
        }
    }
}
