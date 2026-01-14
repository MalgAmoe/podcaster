use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use poddyclip_api::handlers::{delete_job, get_job_result, get_job_status, health, list_presets, process_audio_upload};
use poddyclip_api::state::{AppConfig, AppState};

#[tokio::main]
async fn main() {
    // Load .env if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "poddyclip_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load config from environment
    let config = AppConfig::from_env();
    let port = config.port;
    let max_body_size = config.max_file_size_mb * 1024 * 1024;

    info!("Starting poddyclip-api v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration:");
    info!("  Port: {}", port);
    info!("  Max file size: {} MB", config.max_file_size_mb);
    info!("  Max concurrent jobs: {}", config.max_concurrent_jobs);
    info!("  Job timeout: {}s", config.job_timeout_seconds);
    info!("  Result retention: {}s", config.result_retention_seconds);
    info!("  Chains directory: {}", config.chains_dir.display());

    let state = AppState::new(config);

    // Start cleanup task
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        cleanup_task(cleanup_state).await;
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/presets", get(list_presets))
        .route("/process", post(process_audio_upload))
        .route("/jobs/{id}", get(get_job_status).delete(delete_job))
        .route("/jobs/{id}/result", get(get_job_result))
        .layer(RequestBodyLimitLayer::new(max_body_size))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server with graceful shutdown
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("Listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

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
                to_remove.push(*entry.key());
            }
        }

        for id in to_remove {
            state.jobs.remove(&id);
            info!("Cleaned up expired job {}", id);
        }
    }
}
