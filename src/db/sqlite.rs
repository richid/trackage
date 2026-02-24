use super::{Database, NewPackage, Package, PackageStatus, PackageWithStatus, StatusHistoryEntry};
use crate::courier::CourierCode;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::str::FromStr;
use tracing::info;

pub struct SqliteDatabase {
    conn: Connection,
}

impl SqliteDatabase {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {path}"))?;

        conn.pragma_update(None, "journal_mode", "WAL")
            .context("Failed to enable WAL mode")?;

        let mut db = Self { conn };
        db.migrate()?;

        Ok(db)
    }

    fn migrate(&mut self) -> Result<()> {
        const MIGRATIONS: &[&str] = &[
            include_str!("../../migrations/0001_create_packages_and_metadata.sql"),
            include_str!("../../migrations/0002_create_package_status.sql"),
            include_str!("../../migrations/0003_add_eta_and_location.sql"),
            include_str!("../../migrations/0004_add_status_description.sql"),
            include_str!("../../migrations/0005_add_tracking_url.sql"),
            include_str!("../../migrations/0006_add_deleted_at.sql"),
        ];

        let version: u32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read user_version")?;

        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let target = (i + 1) as u32;
            if version < target {
                info!("Running database migration: v{} → v{}", target - 1, target);
                self.conn
                    .execute_batch(sql)
                    .with_context(|| format!("Migration v{} → v{} failed", target - 1, target))?;
                self.conn
                    .pragma_update(None, "user_version", target)
                    .with_context(|| format!("Failed to set user_version to {target}"))?;
            }
        }

        Ok(())
    }
}

impl Database for SqliteDatabase {
    fn get_last_seen_uid(&self) -> Result<u32> {
        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM metadata WHERE key = 'last_seen_uid'",
                [],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query last_seen_uid")?;

        match result {
            Some(val) => val
                .parse::<u32>()
                .context("Invalid last_seen_uid value in metadata"),
            None => Ok(0),
        }
    }

    fn set_last_seen_uid(&mut self, uid: u32) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO metadata (key, value) VALUES ('last_seen_uid', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [uid.to_string()],
            )
            .context("Failed to update last_seen_uid")?;

        Ok(())
    }

    fn insert_package(&mut self, package: &NewPackage) -> Result<bool> {
        let changes = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO packages
                    (tracking_number, courier, service, tracking_url, source_email_uid,
                     source_email_subject, source_email_from, source_email_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    package.tracking_number,
                    package.courier,
                    package.service,
                    package.tracking_url,
                    package.source_email_uid,
                    package.source_email_subject,
                    package.source_email_from,
                    package.source_email_date.to_rfc3339(),
                ],
            )
            .context("Failed to insert package")?;

        Ok(changes > 0)
    }

    fn get_active_packages(&self) -> Result<Vec<Package>> {
        let mut stmt = self
            .conn
            .prepare(
                "WITH current_status AS (
                    SELECT p.id, p.tracking_number, p.courier, p.service,
                           COALESCE(
                               (SELECT ps.status FROM package_status ps
                                WHERE ps.package_id = p.id
                                ORDER BY ps.id DESC LIMIT 1),
                               'waiting'
                           ) AS status
                    FROM packages p
                    WHERE p.deleted_at IS NULL
                )
                SELECT * FROM current_status WHERE status NOT IN ('delivered', 'not_found')",
            )
            .context("Failed to prepare get_active_packages query")?;

        let packages = stmt
            .query_map([], |row| {
                let status_str: String = row.get(4)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    status_str,
                ))
            })
            .context("Failed to query active packages")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read active packages rows")?;

        packages
            .into_iter()
            .map(|(id, tracking_number, courier, service, status_str)| {
                let status = PackageStatus::from_str(&status_str)
                    .with_context(|| format!("Invalid status '{status_str}' for package {id}"))?;
                let courier = courier
                    .parse::<CourierCode>()
                    .map(|c| c.to_string())
                    .unwrap_or(courier);
                Ok(Package {
                    id,
                    tracking_number,
                    courier,
                    service,
                    status,
                })
            })
            .collect()
    }

    fn get_all_packages_with_status(&self) -> Result<Vec<PackageWithStatus>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT p.id, p.tracking_number, p.courier, p.service,
                        COALESCE(ps.status, 'waiting') AS status,
                        ps.last_known_location,
                        p.tracking_url,
                        p.source_email_from,
                        p.created_at
                 FROM packages p
                 LEFT JOIN package_status ps ON ps.id = (
                     SELECT ps2.id FROM package_status ps2
                     WHERE ps2.package_id = p.id
                     ORDER BY ps2.id DESC LIMIT 1
                 )
                 WHERE p.deleted_at IS NULL
                 ORDER BY p.created_at DESC",
            )
            .context("Failed to prepare get_all_packages_with_status query")?;

        let packages = stmt
            .query_map([], |row| {
                let courier_raw: String = row.get(2)?;
                let courier = courier_raw
                    .parse::<CourierCode>()
                    .map(|c| c.display_name().to_string())
                    .unwrap_or(courier_raw);
                Ok(PackageWithStatus {
                    id: row.get(0)?,
                    tracking_number: row.get(1)?,
                    courier,
                    service: row.get(3)?,
                    status: row.get(4)?,
                    last_known_location: row.get(5)?,
                    tracking_url: row.get(6)?,
                    source_email_from: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .context("Failed to query packages with status")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read packages with status rows")?;

        Ok(packages)
    }

    fn get_package_status_history(&self, package_id: i64) -> Result<Vec<StatusHistoryEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT status, description, last_known_location, checked_at
                 FROM package_status
                 WHERE package_id = ?1
                 ORDER BY id DESC",
            )
            .context("Failed to prepare get_package_status_history query")?;

        let entries = stmt
            .query_map([package_id], |row| {
                Ok(StatusHistoryEntry {
                    status: row.get(0)?,
                    description: row.get(1)?,
                    last_known_location: row.get(2)?,
                    checked_at: row.get(3)?,
                })
            })
            .context("Failed to query package status history")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to read package status history rows")?;

        Ok(entries)
    }

    fn insert_package_status(
        &mut self,
        package_id: i64,
        status: &PackageStatus,
        estimated_arrival_date: Option<&str>,
        last_known_location: Option<&str>,
        description: Option<&str>,
        checked_at: Option<&str>,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO package_status
                    (package_id, status, estimated_arrival_date, last_known_location, description, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, datetime('now')))",
                rusqlite::params![
                    package_id,
                    status.to_string(),
                    estimated_arrival_date,
                    last_known_location,
                    description,
                    checked_at,
                ],
            )
            .context("Failed to insert package status")?;

        Ok(())
    }

    fn delete_package(&mut self, package_id: i64) -> Result<bool> {
        let changes = self
            .conn
            .execute(
                "UPDATE packages SET deleted_at = datetime('now')
                 WHERE id = ?1 AND deleted_at IS NULL",
                [package_id],
            )
            .context("Failed to soft-delete package")?;

        Ok(changes > 0)
    }
}

use rusqlite::OptionalExtension;
