//! OpenObserve log shipping layer.
//!
//! This module provides a tracing Layer that ships logs to OpenObserve
//! via its HTTP JSON API. Logs are buffered and sent in batches.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Serialize;
use tokio::sync::mpsc::{self, Sender};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Fields that should be redacted from logs
const SENSITIVE_FIELDS: &[&str] = &[
    "password",
    "secret",
    "token",
    "api_key",
    "webhook_secret",
    "download_url",
    "authorization",
    "credential",
    "signature",
];

/// Configuration for OpenObserve connection
#[derive(Clone)]
pub struct OpenObserveConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub org: String,
    pub stream: String,
}

impl OpenObserveConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("OPENOBSERVE_URL").ok()?;
        Some(Self {
            url,
            user: std::env::var("OPENOBSERVE_USER").unwrap_or_else(|_| "admin@poddyclip.local".to_string()),
            password: std::env::var("OPENOBSERVE_PASSWORD").unwrap_or_else(|_| "dev".to_string()),
            org: std::env::var("OPENOBSERVE_ORG").unwrap_or_else(|_| "default".to_string()),
            stream: std::env::var("OPENOBSERVE_STREAM").unwrap_or_else(|_| "rust".to_string()),
        })
    }

    fn api_url(&self) -> String {
        format!("{}/api/{}/{}/_json", self.url, self.org, self.stream)
    }
}

/// A log event to be shipped to OpenObserve
#[derive(Debug, Serialize)]
struct LogEvent {
    #[serde(rename = "_timestamp")]
    timestamp: i64,
    level: String,
    message: String,
    target: String,
    service: String,
    #[serde(flatten)]
    fields: HashMap<String, serde_json::Value>,
}

/// A tracing Layer that ships events to OpenObserve
pub struct OpenObserveLayer {
    sender: Sender<LogEvent>,
}

impl OpenObserveLayer {
    /// Create a new OpenObserve layer and start the background shipper task.
    pub fn new(config: OpenObserveConfig) -> Self {
        let (sender, mut receiver) = mpsc::channel::<LogEvent>(1000);

        // Spawn the background task that ships logs
        let config = Arc::new(config);
        tokio::spawn(async move {
            let client = Client::new();
            let mut buffer = Vec::with_capacity(BATCH_SIZE);
            let mut interval = tokio::time::interval(FLUSH_INTERVAL);

            loop {
                tokio::select! {
                    Some(event) = receiver.recv() => {
                        buffer.push(event);
                        if buffer.len() >= BATCH_SIZE {
                            ship_logs(&client, &config, &mut buffer).await;
                        }
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            ship_logs(&client, &config, &mut buffer).await;
                        }
                    }
                }
            }
        });

        Self { sender }
    }
}

async fn ship_logs(client: &Client, config: &OpenObserveConfig, buffer: &mut Vec<LogEvent>) {
    let events: Vec<_> = buffer.drain(..).collect();
    let count = events.len();

    let result = client
        .post(&config.api_url())
        .basic_auth(&config.user, Some(&config.password))
        .json(&events)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match result {
        Ok(response) if response.status().is_success() => {
            // Silent on success - reduce noise
        }
        Ok(response) => {
            eprintln!(
                "[OpenObserve] Failed to ship {} logs: HTTP {}",
                count,
                response.status()
            );
        }
        Err(e) => {
            eprintln!("[OpenObserve] Failed to ship logs: {}", e);
        }
    }
}

/// Check if a field name is sensitive and should be redacted
fn is_sensitive_field(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    SENSITIVE_FIELDS.iter().any(|&s| name_lower.contains(s))
}

/// Redact presigned URLs and sensitive values in strings
fn sanitize_string_value(value: &str) -> String {
    // Redact AWS presigned URLs (contain signatures)
    if value.contains("X-Amz-Signature") || value.contains("X-Amz-Algorithm") {
        "[REDACTED PRESIGNED URL]".to_string()
    } else {
        value.to_string()
    }
}

/// Visitor that extracts fields from a tracing event
struct FieldVisitor {
    message: String,
    fields: HashMap<String, serde_json::Value>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: HashMap::new(),
        }
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if name == "message" {
            self.message = sanitize_string_value(&format!("{:?}", value));
        } else if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            let value_str = format!("{:?}", value);
            self.fields.insert(name.to_string(), serde_json::json!(sanitize_string_value(&value_str)));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = sanitize_string_value(value);
        } else if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(sanitize_string_value(value)));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }

    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        let name = field.name();
        if is_sensitive_field(name) {
            self.fields.insert(name.to_string(), serde_json::json!("[REDACTED]"));
        } else {
            self.fields.insert(name.to_string(), serde_json::json!(value));
        }
    }
}

impl<S> Layer<S> for OpenObserveLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let log_event = LogEvent {
            timestamp: chrono_timestamp_micros(),
            level: metadata.level().to_string(),
            message: visitor.message,
            target: metadata.target().to_string(),
            service: "poddyclip-api".to_string(),
            fields: visitor.fields,
        };

        // Send is non-blocking with a buffered channel
        let _ = self.sender.try_send(log_event);
    }
}

fn chrono_timestamp_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}
