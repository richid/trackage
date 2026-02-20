CREATE TABLE packages (
    id INTEGER PRIMARY KEY,
    tracking_number TEXT NOT NULL UNIQUE,
    courier TEXT NOT NULL,
    service TEXT NOT NULL,
    source_email_uid INTEGER NOT NULL,
    source_email_subject TEXT,
    source_email_from TEXT,
    source_email_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
