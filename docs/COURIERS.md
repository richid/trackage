# Courier Configuration

Trackage polls courier APIs to check delivery status for tracked packages. Each courier requires its own API credentials. All courier configuration is optional — if credentials are not provided for a courier, packages from that courier will still be discovered and stored, but status polling will be skipped for them.

Courier credentials are configured under the `[courier]` section of `config.toml`, or via environment variables prefixed with `TRACKAGE_COURIER__` (using `__` as the nesting separator).

## FedEx

Trackage uses the [FedEx Track API](https://developer.fedex.com/api/en-us/catalog/track/v1/docs.html) (part of the free Basic Integrated Visibility tier) to check delivery status.

### Getting API Credentials

1. Create a FedEx Developer account at https://developer.fedex.com/
2. Go to **My Projects** and create a new project.
3. Select the **Track API** when choosing which APIs to add.
4. Choose the **SANDBOX** or **PRODUCTION** environment. For real tracking, use production.
5. Once the project is created, you'll be given a **Client ID** (API Key) and **Client Secret** (Secret Key).

### Configuration

Add the credentials to `config.toml`:

```toml
[courier.fedex]
client_id = "your-client-id"
client_secret = "your-client-secret"
```

Or via environment variables:

```sh
export TRACKAGE_COURIER__FEDEX__CLIENT_ID="your-client-id"
export TRACKAGE_COURIER__FEDEX__CLIENT_SECRET="your-client-secret"
```

### Status Mapping

FedEx status codes are mapped as follows:

| FedEx Code | Trackage Status | Meaning |
|------------|-----------------|---------|
| `DL` | delivered | Package has been delivered |
| `OC` | waiting | Label created, not yet picked up |
| All others (`IT`, `OD`, `PU`, `DP`, `AR`, ...) | in_transit | Package is in transit |
