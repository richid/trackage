use crate::config::EmailConfig;
use crate::db::{Database, NewPackage};
use crate::extractors;
use crate::imap_client::{ImapClient, MailMessage, parse_message};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info};

pub struct EmailPoller {
    config: EmailConfig,
    db: Box<dyn Database>,
    running: Arc<AtomicBool>,
}

impl EmailPoller {
    pub fn new(config: EmailConfig, db: Box<dyn Database>, running: Arc<AtomicBool>) -> Self {
        Self { config, db, running }
    }

    /// Run the poll loop. Blocks until the shutdown signal fires.
    pub fn run(mut self) {
        while self.running.load(Ordering::SeqCst) {
            self.poll_once();
            self.sleep();
        }

        info!("Email poller shutting down");
    }

    fn poll_once(&mut self) {
        let last_seen_uid = match self.db.get_last_seen_uid() {
            Ok(uid) => uid,
            Err(err) => {
                error!(error = %err, "Failed to read last_seen_uid from database");
                return;
            }
        };

        info!(last_seen_uid, "Connecting to server");

        let mut client = match ImapClient::connect(&self.config) {
            Ok(client) => client,
            Err(err) => {
                error!(error = %err, "IMAP connection failed");
                return;
            }
        };

        let messages = match client.fetch_messages_since_uid(last_seen_uid) {
            Ok(messages) => messages,
            Err(err) => {
                error!(error = %err, "IMAP fetch failed");
                let _ = client.logout();
                return;
            }
        };

        info!(count = messages.len(), "New messages fetched");

        let mut max_uid = last_seen_uid;

        for msg in &messages {
            if msg.uid > max_uid {
                max_uid = msg.uid;
            }
            self.process_message(msg);
        }

        if let Err(err) = self.db.set_last_seen_uid(max_uid) {
            error!(error = %err, "Failed to save last_seen_uid to database");
        }

        let _ = client.logout();
    }

    fn process_message(&mut self, msg: &MailMessage) {
        let parsed = match parse_message(msg) {
            Ok(parsed) => parsed,
            Err(err) => {
                error!(error = %err, uid = msg.uid, "Failed to parse MIME message");
                return;
            }
        };

        info!(
            uid = msg.uid,
            date = %parsed.internal_date,
            subject = parsed.subject.as_deref().unwrap_or("<none>"),
            body_len = parsed.body_text.len(),
            "Parsed email"
        );

        debug!(
            body_preview = &parsed.body_text[..parsed.body_text.len().min(200)],
            "Email body preview"
        );

        let results = extractors::extract_tracking_numbers(&parsed.body_text);

        for result in &results {
            info!(
                tracking_number = %result.tracking_number,
                courier = %result.courier,
                service = %result.service,
                "Validated tracking number"
            );

            let new_package = NewPackage {
                tracking_number: result.tracking_number.clone(),
                courier: result.courier.clone(),
                service: result.service.clone(),
                source_email_uid: msg.uid,
                source_email_subject: parsed.subject.clone(),
                source_email_from: parsed.from.clone(),
                source_email_date: parsed.internal_date,
            };

            match self.db.insert_package(&new_package) {
                Ok(true) => {
                    info!(
                        tracking_number = %result.tracking_number,
                        "New package saved to database"
                    );
                }
                Ok(false) => {
                    debug!(
                        tracking_number = %result.tracking_number,
                        "Package already exists in database"
                    );
                }
                Err(err) => {
                    error!(
                        error = %err,
                        tracking_number = %result.tracking_number,
                        "Failed to save package to database"
                    );
                }
            }
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
