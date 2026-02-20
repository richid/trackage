CREATE TABLE package_status (
    id INTEGER PRIMARY KEY,
    package_id INTEGER NOT NULL REFERENCES packages(id),
    status TEXT NOT NULL,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);
