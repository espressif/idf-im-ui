

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::time::Duration;
use uuid::Uuid;

/// Compile-time injected by GitHub Actions (see below).
/// When the secret is missing we inject the empty string → telemetry disabled.
static CONNECTION_STRING: Option<&str> = option_env!("APP_INSIGHTS_CONNECTION_STRING");

static CLIENT: Lazy<Option<reqwest::Client>> = Lazy::new(|| {
    if CONNECTION_STRING.is_none() {
        return None;
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
});

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryItem {
    name: String,
    time: String,
    i_key: String,
    tags: serde_json::Value,
    data: serde_json::Value,
}

/// Fire-and-forget custom event.  If telemetry is disabled this is a no-op.
pub async fn track_event(name: &str, properties: serde_json::Value) {
    let client = match &*CLIENT {
        Some(c) => c,
        None => return,
    };
    // Parse the connection string only once.
    let (i_key, ingest_base) = parse_connection_string();
    let url = format!("{}/v2/track", ingest_base);

    let payload = TelemetryItem {
        name: "Microsoft.ApplicationInsights.Event".into(),
        time: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        i_key,
        tags: serde_json::json!({ "ai.cloud.role": "desktop-app" }),
        data: serde_json::json!({ "baseType": "EventData",
                                   "baseData": { "name": name, "properties": properties }}),
    };

    // Swallow errors – we never want to break the app because telemetry failed.
    let _ = client
        .post(&url)
        .header("Content-Type", "application/x-json-stream")
        .body(serde_json::to_string(&payload).unwrap())
        .send()
        .await;
}

fn parse_connection_string() -> (String, String) {
    let mut i_key = String::new();
    let mut base = "https://dc.services.visualstudio.com".to_string(); // fallback
    for kv in CONNECTION_STRING.unwrap().split(';') {
        let mut parts = kv.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("InstrumentationKey"), Some(v)) => i_key = v.into(),
            (Some("IngestionEndpoint"), Some(v)) => base = v.trim_end_matches('/').into(),
            _ => {}
        }
    }
    (i_key, base)
}
