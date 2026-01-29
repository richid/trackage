use crate::config::EmailConfig;
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use tracing::info;
//use imap::types::Fetch;

#[derive(Debug)]
pub struct MailMessage {
    pub internal_date: DateTime<Utc>,
    pub headers: String,
    pub body: String,
}

#[derive(Debug)]
pub struct ParsedMessage {
    pub internal_date: chrono::DateTime<chrono::Utc>,
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

    pub fn fetch_message_dates_since(
        &mut self,
        last_checked_at: u64,
    ) -> Result<Vec<MailMessage>> {
        let since_date = Utc
            .timestamp_opt(last_checked_at as i64, 0)
            .single()
            .unwrap()
            .format("%d-%b-%Y")
            .to_string();

        info!(since = %since_date, "Searching for messages");

        let seq_nums = self
            .session
            .uid_search(format!("SINCE {}", since_date))
            .context("IMAP search failed")?;

        info!("Found messsages! {:?}", seq_nums);

        if seq_nums.is_empty() {
            return Ok(vec![]);
        }

        let fetches = self
            .session
            .fetch(
                seq_nums
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                "(RFC822.HEADER RFC822 INTERNALDATE)",
            )
            .context("IMAP fetch failed")?;

        let mut messages = Vec::new();

        for msg in fetches.iter() {
            let internal_date = match msg.internal_date() {
                Some(d) => d.with_timezone(&Utc),
                None => continue,
            };

            if internal_date.timestamp() as u64 <= last_checked_at {
                info!("skibbidi");
                continue
            }

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
                internal_date,
                headers,
                body
            });
        }

        Ok(messages)
    }

    /// Fetch message INTERNALDATE values since the given UNIX timestamp
    /*
    pub fn fetch_message_dates_since(
        &mut self,
        last_checked_at: u64,
    ) -> Result<Vec<u64>> {
        let since_date = Utc
            .timestamp_opt(last_checked_at as i64, 0)
            .single()
            .unwrap()
            .format("%d-%b-%Y")
            .to_string();

        info!(since = %since_date, "Searching for messages");

        let seq_nums = self
            .session
            .search(format!("SINCE {}", since_date))
            .context("IMAP search failed")?;

        if seq_nums.is_empty() {
            return Ok(vec![]);
        }

        let fetches = self
            .session
            .fetch(
                seq_nums
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                "INTERNALDATE",
            )
            .context("IMAP fetch failed")?;

        let mut timestamps = Vec::new();

        for msg in fetches.iter() {
            if let Some(date) = msg.internal_date() {
                let dt: DateTime<Utc> = date.into();
                timestamps.push(dt.timestamp() as u64);
            }
        }

        Ok(timestamps)
    }
    */

    pub fn logout(mut self) -> Result<()> {
        info!("Closing IMAP server connection");
        self.session.logout()?;
        Ok(())
    }
}

// Split to new crate
use mailparse::{parse_mail, ParsedMail};

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
