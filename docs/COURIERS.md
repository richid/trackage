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

## UPS

Trackage uses the [UPS Tracking API](https://developer.ups.com/api/reference?loc=en_US#tag/Tracking_x0020_API) to check delivery status.

### Getting API Credentials

1. Create a UPS Developer account at https://developer.ups.com/
2. Go to **Apps** and create a new application.
3. Select the **Tracking API** when choosing which APIs to add.
4. Once the application is created, you'll be given a **Client ID** and **Client Secret**.

### Configuration

Add the credentials to `config.toml`:

```toml
[courier.ups]
client_id = "your-client-id"
client_secret = "your-client-secret"
```

Or via environment variables:

```sh
export TRACKAGE_COURIER__UPS__CLIENT_ID="your-client-id"
export TRACKAGE_COURIER__UPS__CLIENT_SECRET="your-client-secret"
```

### Status Mapping

UPS status codes are mapped as follows:

| UPS Code | Trackage Status | Meaning |
|----------|-----------------|---------|
| `D` | delivered | Package has been delivered |
| `M` | waiting | Manifest created, not yet in UPS system |
| `P` | waiting | Picked up |
| All others (`I`, `X`, `U`, ...) | in_transit | Package is in transit |

## USPS

Trackage uses the [USPS Tracking API v3](https://developers.usps.com/trackingv3r2) to check delivery status.

### Getting API Credentials

1. Create a USPS Business account at https://developers.usps.com/
2. Go to **Apps** and register a new application.
3. Ensure the **Tracking** API is included in your application's granted scopes. If the `tracking` scope is not included in your OAuth tokens, [submit a service request](https://emailus.usps.com/s/web-tools-inquiry) to have it added.
4. Once the application is created, you'll be given a **Consumer Key** (client ID) and **Consumer Secret** (client secret).

### Configuration

Add the credentials to `config.toml`:

```toml
[courier.usps]
client_id = "your-consumer-key"
client_secret = "your-consumer-secret"
```

Or via environment variables:

```sh
export TRACKAGE_COURIER__USPS__CLIENT_ID="your-consumer-key"
export TRACKAGE_COURIER__USPS__CLIENT_SECRET="your-consumer-secret"
```

### Status Mapping

USPS status categories are mapped as follows:

| USPS `statusCategory` | Trackage Status | Meaning |
|------------------------|-----------------|---------|
| `Delivered` | delivered | Package has been delivered |
| `Pre-Shipment` | waiting | Label created, not yet in USPS system |
| All others (`Accepted`, `In Transit`, `Out for Delivery`, `Alert`, ...) | in_transit | Package is in transit |
