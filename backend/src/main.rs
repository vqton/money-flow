use backend::config::AppConfig;
use backend::database::LedgerRepository;
use backend::handlers::create_app;
use backend::ledger::LedgerEngine;
use backend::state::AppState;
use rusqlite::Connection;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Entry point for the accounting backend service.
/// Loads 12-factor configuration, initializes tracing, sets up the SQLite database, and runs the Axum HTTP server.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Load runtime configuration from environment or sensible defaults
    let config = AppConfig::from_env();

    // Initialize tracing for auditability using configured log level.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 1. Establish SQLite connection to database file
    let conn = Connection::open(&config.database_path)?;
    let repo = LedgerRepository::new(conn);

    // 2. Initialize relational database schema with WAL pragmas
    repo.init_schema()?;
    tracing::info!("Database schema initialized at '{}'", config.database_path);

    // 3. Construct application state with runtime configuration
    let engine = LedgerEngine::new();
    let state = Arc::new(AppState::with_config(engine, repo, config.clone()));

    // 4. Build Axum router with state
    let app = create_app(state);

    // 5. Bind TCP listener dynamically from config
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Accounting backend initialized and listening on http://{}", addr);

    // 6. Start Axum server
    axum::serve(listener, app).await?;

    Ok(())
}
