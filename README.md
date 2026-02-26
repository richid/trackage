# trackage-rs

Rust-based application to automatically keep tabs on your incoming packages. Trackage scans an IMAP mailbox for emails containing tracking numbers, stores discovered packages in a SQLite database, and periodically checks courier APIs for delivery status updates. A built-in web UI lets you view package status, history, and manually add tracking numbers.

## How It Works

1. **Email polling** — Connects to your IMAP mailbox on a configurable interval, fetches new messages, and extracts tracking numbers from email bodies.
2. **Status polling** — Periodically checks each active package's delivery status with the appropriate courier API. Once a package is delivered, it is marked as such.
3. **Web UI** — Optional browser-based dashboard showing all tracked packages with status, location, and full activity history. Supports manual tracking number entry and per-package actions (delete, rescan).

## Building

```sh
cargo build --release
```

## Configuration

Copy `config.toml.sample` to `config.toml` and fill in your values. All settings can also be provided via environment variables prefixed with `TRACKAGE_`, using `__` as the nesting separator (e.g. `TRACKAGE_EMAIL__SERVER`).

### Email (required)

```toml
[email]
server   = "imap.example.com"
port     = 993
username = "you@example.com"
password = "your-password"
folder   = "INBOX"
check_interval_seconds = 300
```

### Database (optional)

```toml
[database]
path = "trackage.db"    # defaults to trackage.db
```

### Status Polling (optional)

```toml
[status]
check_interval_seconds = 3600    # defaults to 3600 (1 hour)
```

### Web UI (optional)

```toml
[web]
enabled = true
port = 3000    # defaults to 3000
```

When enabled, the web UI is available at `http://localhost:3000`.

### Couriers (optional)

Courier API credentials enable live delivery status checks. See [docs/COURIERS.md](docs/COURIERS.md) for setup instructions. Currently supported:

- **FedEx** — via the FedEx Track API
- **USPS** — via the USPS Tracking API v3
- **UPS** — via the UPS Tracking API, or automatically via a credential-free web fallback when no API credentials are configured

## Running

```sh
cargo run
```

Logging is controlled via the `RUST_LOG` environment variable (defaults to `info`):

```sh
RUST_LOG=debug cargo run
```
