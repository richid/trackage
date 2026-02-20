use crate::config::EmailConfig;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::info;

#[derive(Debug)]
pub struct MailMessage {
    pub uid: u32,
    pub internal_date: DateTime<Utc>,
    pub headers: String,
    pub body: String,
}

#[derive(Debug)]
pub struct ParsedMessage {
    pub internal_date: DateTime<Utc>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub body_text: String,
}

pub struct ImapClient {
    session: imap::Session<Box<dyn imap::ImapConnection>>,
}

impl ImapClient {
    pub fn connect(config: &EmailConfig) -> Result<Self> {
        let server = config.server.as_ref().context("email.server missing")?;
        let username = config.username.as_ref().context("email.username missing")?;
        let password = config.password.as_ref().context("email.password missing")?;

        let client = imap::ClientBuilder::new(server, config.port)
            .connect()
            .context("Failed to connect to IMAP server")?;

        let mut session = client
            .login(username, password)
            .map_err(|e| e.0)
            .context("Failed to authenticate to IMAP server")?;

        session
            .select(&config.folder)
            .context("Failed to select IMAP folder")?;

        info!(folder = %config.folder, "IMAP folder selected");

        Ok(Self { session })
    }

    /// Fetch all messages with UIDs greater than `last_seen_uid`.
    /// This catches newly delivered, moved, and copied messages regardless
    /// of their internal date.
    pub fn fetch_messages_since_uid(&mut self, last_seen_uid: u32) -> Result<Vec<MailMessage>> {
        let search_range = format!("UID {}:*", last_seen_uid + 1);

        info!(since_uid = last_seen_uid + 1, "Searching for new messages");

        let uids = self
            .session
            .uid_search(search_range)
            .context("IMAP UID search failed")?;

        // Filter out UIDs we've already seen (IMAP `UID x:*` always includes
        // at least the highest existing UID even if it's <= x)
        let new_uids: Vec<u32> = uids.into_iter().filter(|&uid| uid > last_seen_uid).collect();

        info!(count = new_uids.len(), "New messages found");

        if new_uids.is_empty() {
            return Ok(vec![]);
        }

        let uid_list = new_uids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        let fetches = self
            .session
            .uid_fetch(uid_list, "(BODY.PEEK[HEADER] BODY.PEEK[] INTERNALDATE)")
            .context("IMAP fetch failed")?;

        let mut messages = Vec::new();

        for msg in fetches.iter() {
            let uid = match msg.uid {
                Some(uid) => uid,
                None => continue,
            };

            let internal_date = match msg.internal_date() {
                Some(d) => d.with_timezone(&Utc),
                None => continue,
            };

            let headers = msg
                .header()
                .and_then(|h| std::str::from_utf8(h).ok())
                .unwrap_or("")
                .to_string();

            let body = msg
                .body()
                .and_then(|b| std::str::from_utf8(b).ok())
                .unwrap_or("")
                .to_string();

            messages.push(MailMessage {
                uid,
                internal_date,
                headers,
                body,
            });
        }

        Ok(messages)
    }

    pub fn logout(mut self) -> Result<()> {
        info!("Closing IMAP server connection");
        self.session.logout()?;
        Ok(())
    }
}

use mailparse::{ParsedMail, parse_mail};

fn extract_text_from_part(part: &ParsedMail) -> Option<String> {
    let ctype = part.ctype.mimetype.to_lowercase();

    if ctype == "text/plain" {
        return part.get_body().ok();
    }

    if ctype == "text/html" {
        let html = part.get_body().ok()?;
        return Some(html2text::from_read(html.as_bytes(), 80));
    }

    for subpart in &part.subparts {
        if let Some(text) = extract_text_from_part(subpart) {
            return Some(text);
        }
    }

    None
}

fn get_header(headers: &str, name: &str) -> Option<String> {
    for line in headers.lines() {
        if line.to_lowercase().starts_with(&name.to_lowercase()) {
            return Some(line[name.len() + 1..].trim().to_string());
        }
    }
    None
}

pub fn parse_message(msg: &MailMessage) -> Result<ParsedMessage> {
    let parsed = parse_mail(msg.body.as_bytes())?;

    let body_text = extract_text_from_part(&parsed)
        .unwrap_or_else(|| "".to_string())
        .trim()
        .to_string();

    Ok(ParsedMessage {
        internal_date: msg.internal_date,
        subject: get_header(&msg.headers, "Subject"),
        from: get_header(&msg.headers, "From"),
        body_text,
    })
}
