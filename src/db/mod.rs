mod sqlite;

pub use sqlite::SqliteDatabase;

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageStatus {
    Waiting,
    InTransit,
    Delivered,
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageStatus::Waiting => write!(f, "waiting"),
            PackageStatus::InTransit => write!(f, "in_transit"),
            PackageStatus::Delivered => write!(f, "delivered"),
        }
    }
}

impl FromStr for PackageStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "waiting" => Ok(PackageStatus::Waiting),
            "in_transit" => Ok(PackageStatus::InTransit),
            "delivered" => Ok(PackageStatus::Delivered),
            other => Err(anyhow::anyhow!("Unknown package status: {other}")),
        }
    }
}

pub struct Package {
    pub id: i64,
    pub tracking_number: String,
    pub courier: String,
    pub service: String,
    pub status: PackageStatus,
}

pub struct NewPackage {
    pub tracking_number: String,
    pub courier: String,
    pub service: String,
    pub source_email_uid: u32,
    pub source_email_subject: Option<String>,
    pub source_email_from: Option<String>,
    pub source_email_date: DateTime<Utc>,
}

pub trait Database: Send {
    /// Get the highest IMAP UID we have processed.
    fn get_last_seen_uid(&self) -> Result<u32>;

    /// Update the highest IMAP UID we have processed.
    fn set_last_seen_uid(&mut self, uid: u32) -> Result<()>;

    /// Insert a package if the tracking number doesn't already exist.
    /// Returns `true` if a new row was inserted.
    fn insert_package(&mut self, package: &NewPackage) -> Result<bool>;

    /// Get all packages that have not yet been delivered.
    fn get_active_packages(&self) -> Result<Vec<Package>>;

    /// Insert a status check record into package_status history.
    fn insert_package_status(
        &mut self,
        package_id: i64,
        status: &PackageStatus,
        estimated_arrival_date: Option<&str>,
        last_known_location: Option<&str>,
    ) -> Result<()>;
}
