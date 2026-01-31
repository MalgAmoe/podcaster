use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use tokio::net::TcpListener;
use tokio::signal;
use axum::http::HeaderValue;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use poddyclip_api::handlers::{create_s3_job, delete_job, health, list_presets};
use poddyclip_api::openobserve::{OpenObserveConfig, OpenObserveLayer};
use poddyclip_api::require_api_key;
use poddyclip_api::state::{AppConfig, AppState};
use poddyclip_api::storage::{Storage, StorageConfig};

#[tokio::main]
async fn main() {
    // Load .env if present check
    dotenvy::dotenv().ok();

    // Initialize tracing with optional OpenObserve layer
    let openobserve_config = OpenObserveConfig::from_env();
    let openobserve_layer = openobserve_config.as_ref().map(|config| {
        OpenObserveLayer::new(config.clone())
    });

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "poddyclip_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(openobserve_layer)
        .init();

    // Log OpenObserve status after tracing is initialized
    if let Some(config) = openobserve_config {
        info!("OpenObserve logging enabled: {}", config.url);
    }

    // Load config from environment
    let config = AppConfig::from_env();
    let port = config.port;
    let max_body_size = config.max_file_size_mb * 1024 * 1024;

    info!("Starting poddyclip-api v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration:");
    info!("  Port: {}", port);
    info!("  Max file size: {} MB", config.max_file_size_mb);
    info!("  Job timeout: {}s", config.job_timeout_seconds);
    info!("  Result retention: {}s", config.result_retention_seconds);
    info!("  Chains directory: {}", config.chains_dir.display());

    // Initialize S3 storage if configured
    let storage = match StorageConfig::from_env() {
        Some(storage_config) => {
            info!("S3 storage configured:");
            info!("  Endpoint: {}", storage_config.endpoint);
            info!("  Bucket: {}", storage_config.bucket_name);
            match Storage::new(storage_config) {
                Ok(s) => {
                    info!("  Status: connected");
                    Some(s)
                }
                Err(e) => {
                    warn!("Failed to initialize S3 storage: {}. Falling back to in-memory.", e);
                    None
                }
            }
        }
        None => {
            info!("S3 storage not configured, using in-memory storage");
            None
        }
    };

    let state = AppState::new(config, storage);

    // Log API key status
    if state.config.api_key.is_some() {
        info!("  API key: configured (protected mode)");
    } else {
        warn!("  API key: NOT configured (dev mode - all requests allowed)");
    }

    // Start cleanup task
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        cleanup_task(cleanup_state).await;
    });

    // Protected routes (require API key)
    let protected_routes = Router::new()
        .route("/jobs", post(create_s3_job))
        .route("/jobs/{id}", delete(delete_job))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_api_key));

    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/presets", get(list_presets));

    // Configure CORS based on CORS_ORIGINS env var
    // - Not set or empty: allow any origin (dev mode)
    // - Comma-separated list: allow only those origins (production)
    let cors_layer = if let Some(ref origins) = state.config.cors_origins {
        let allowed: Vec<HeaderValue> = origins
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if allowed.is_empty() {
            warn!("CORS_ORIGINS set but no valid origins parsed, allowing any");
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            info!("CORS restricted to: {}", origins);
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(allowed))
                .allow_methods(Any)
                .allow_headers(Any)
        }
    } else {
        warn!("CORS_ORIGINS not set, allowing any origin (dev mode)");
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    // Build router
    let app = public_routes
        .merge(protected_routes)
        .layer(RequestBodyLimitLayer::new(max_body_size))
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server with graceful shutdown
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap_or_else(|e| {
        panic!(
            "Failed to bind to {}: {}. Check if the port is already in use or you have permission to bind.",
            addr, e
        )
    });
    info!("Listening on http://{}", addr);

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        panic!("Server error: {}. The server encountered a fatal error and must exit.", e);
    }

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}

/// Background task to clean up old completed/failed jobs
async fn cleanup_task(state: AppState) {
    let retention_seconds = state.config.result_retention_seconds;

    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut to_remove = Vec::new();

        for entry in state.jobs.iter() {
            let job = entry.value();
            let is_terminal = matches!(
                job.status,
                poddyclip_api::models::JobStatus::Completed | poddyclip_api::models::JobStatus::Failed
            );

            if is_terminal && (now - job.updated_at) > retention_seconds {
                to_remove.push((*entry.key(), job.result_s3_key.clone()));
            }
        }

        for (id, s3_key) in to_remove {
            // Delete from S3 if present
            if let (Some(key), Some(ref storage)) = (s3_key, &state.storage) {
                if let Err(e) = storage.delete(&key).await {
                    warn!("Failed to delete S3 object {} during cleanup: {}", key, e);
                }
            }

            state.jobs.remove(&id);
            info!("Cleaned up expired job {}", id);
        }
    }
}
