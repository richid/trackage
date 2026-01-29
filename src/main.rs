mod config;
mod extractors;
mod imap_client;
mod state;

use config::{load as config_load, validate as config_validate};
use state::{load as state_load, save as state_save};
use imap_client::{ImapClient, parse_message};
use std::{
    process::exit, sync::{
        Arc, atomic::{AtomicBool, Ordering}
    }, thread, time::{Duration, SystemTime, UNIX_EPOCH}
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use parcel::{track, Tracking};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let c = track("adf");
    for t in c.iter() {
        info!("{}", t.tracking_number);
        info!("{}", t.courier);
    }
    info!("wat");
    exit(1);

    let config = config_load();

    if let Err(err) = config_validate(&config) {
        eprintln!("Configuration error: {err}");
        std::process::exit(1);
    }

    info!(
        email_config = ?config.email.sanitized_for_log(),
        "Effective configuration loaded"
    );

    info!(
        check_interval_seconds = config.email.check_interval_seconds,
        "trackage starting"
    );

    let mut state = match state_load() {
        Ok(state) => {
            info!(last_checked_at = state.last_checked_at, "Loaded state");
            state
        }
        Err(err) => {
            error!(error = %err, "Failed to load state");
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

    while running.load(Ordering::SeqCst) {

        info!(state.last_checked_at, "Connecting to server");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match ImapClient::connect(&config.email) {
            Ok(mut client) => {
                match client.fetch_message_dates_since(state.last_checked_at) {
                    Ok(messages) => {
                        info!(count = messages.len(), "New messages fetched");

                        for msg in messages {
                            match parse_message(&msg) {
                                Ok(parsed) => {
                                    tracing::info!(
                                        date = %parsed.internal_date,
                                        subject = parsed.subject.as_deref().unwrap_or("<none>"),
                                        body_len = parsed.body_text.len(),
                                        "Parsed email"
                                    );

                                    tracing::debug!(
                                        body_preview = &parsed.body_text[..parsed.body_text.len().min(200)],
                                        "Email body preview"
                                    );

                                    let candidates = extractors::extract_candidates(&parsed.body_text);

                                    for candidate in candidates {
                                        tracing::info!(candidate = %candidate, "Found tracking candidate");
                                    }
                                }
                                Err(err) => {
                                    tracing::error!(error = %err, "Failed to parse MIME message");
                                }
                            }
                        }

                        state.last_checked_at = now;
                        let _ = state_save(&state);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "IMAP fetch failed");
                    }
                }

                let _ = client.logout();
            }
            Err(err) => {
                tracing::error!(error = %err, "IMAP connection failed");
            }
        }

        let mut slept = 0;
        while slept < config.email.check_interval_seconds
            && running.load(Ordering::SeqCst)
        {
            thread::sleep(Duration::from_secs(1));
            slept += 1;
        }
    }

    info!("trackage stopped");
}