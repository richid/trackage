mod config;
mod courier;
mod db;
mod email_poller;
mod extractors;
mod imap_client;
mod status_poller;

use config::{load as config_load, validate as config_validate};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("trackage starting");

    let config = config_load();

    if let Err(err) = config_validate(&config) {
        eprintln!("Configuration error: {err}");
        std::process::exit(1);
    }

    info!(config = ?config.sanitized_for_log(), "Effective configuration");

    let email_db = match db::SqliteDatabase::open(&config.database.path) {
        Ok(db) => db,
        Err(err) => {
            error!(error = %err, "Failed to open database");
            std::process::exit(1);
        }
    };

    let status_db = match db::SqliteDatabase::open(&config.database.path) {
        Ok(db) => db,
        Err(err) => {
            error!(error = %err, "Failed to open status poller database connection");
            std::process::exit(1);
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let running_signal = Arc::clone(&running);

    ctrlc::set_handler(move || {
        info!("Ctrl-C received, shutting down gracefully");
        running_signal.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let email_poller = email_poller::EmailPoller::new(
        config.email,
        Box::new(email_db),
        Arc::clone(&running),
    );
    let email_handle = std::thread::Builder::new()
        .name("email-poller".into())
        .spawn(move || email_poller.run())
        .expect("Failed to spawn email poller thread");

    let mut router = courier::CourierRouter::new();
    if let Some(ref fedex_config) = config.courier.fedex {
        info!("FedEx courier client enabled");
        router.register(&courier::CourierCode::FedEx, Box::new(courier::fedex::FedexClient::new(fedex_config)));
    }
    if let Some(ref ups_config) = config.courier.ups {
        info!("UPS courier client enabled");
        router.register(&courier::CourierCode::UPS, Box::new(courier::ups::UpsClient::new(ups_config)));
    }
    if let Some(ref usps_config) = config.courier.usps {
        info!("USPS courier client enabled");
        router.register(&courier::CourierCode::USPS, Box::new(courier::usps::UspsClient::new(usps_config)));
    }

    let status_poller = status_poller::StatusPoller::new(
        config.status,
        Box::new(status_db),
        Box::new(router),
        Arc::clone(&running),
    );
    let status_handle = std::thread::Builder::new()
        .name("status-poller".into())
        .spawn(move || status_poller.run())
        .expect("Failed to spawn status poller thread");

    let mut exit_code = 0;

    if let Err(err) = email_handle.join() {
        error!("Email poller thread panicked: {:?}", err);
        exit_code = 1;
    }

    if let Err(err) = status_handle.join() {
        error!("Status poller thread panicked: {:?}", err);
        exit_code = 1;
    }

    if exit_code == 0 {
        info!("trackage stopped");
    } else {
        std::process::exit(exit_code);
    }
}
