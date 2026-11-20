use backend::config::AppConfig;
use backend::database::LedgerRepository;
use backend::diagnostics::{init_dual_sink_subscriber, log_startup_error, setup_panic_hook};
use backend::handlers::create_app;
use backend::ledger::LedgerEngine;
use backend::state::AppState;
use rusqlite::Connection;
use std::sync::Arc;

/// Entry point for the accounting backend service.
/// Loads 12-factor configuration, initializes early tracing, sets up the SQLite database, and runs the Axum HTTP server.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Resolve log directory and install panic hook early
    let dotenv = AppConfig::read_dotenv_map();
    let log_dir = std::env::var("LOG_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            dotenv
                .get("LOG_DIR")
                .cloned()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "./logs".to_string());

    if let Err(err) = std::fs::create_dir_all(&log_dir) {
        eprintln!("[WARN] Failed to create log directory '{log_dir}': {err}");
    }

    // Install global panic hook capturing timestamp, payload, location, thread, and backtrace
    setup_panic_hook(&log_dir);

    // Initialize dual-sink tracing subscriber: stdout with ANSI + daily rolling file appender
    let initial_log_level = std::env::var("LOG_LEVEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            dotenv
                .get("LOG_LEVEL")
                .cloned()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "info".to_string());

    let initial_log_format = std::env::var("LOG_FORMAT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            dotenv
                .get("LOG_FORMAT")
                .cloned()
                .filter(|s| !s.trim().is_empty())
        })
        .and_then(|s| backend::config::LogFormat::parse(&s).ok())
        .unwrap_or_default();

    let _log_guard = init_dual_sink_subscriber(&log_dir, &initial_log_level, initial_log_format);

    tracing::info!("Initializing VAS Enterprise Accounting Engine (money-flow)...");

    // 1. Load runtime configuration from environment or optional .env file
    let config = match AppConfig::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!(
                error = %err,
                "Fatal configuration failure during startup"
            );
            let (rca, remediation) = match &err {
                backend::config::ConfigError::InvalidLogFormat(val) => (
                    format!(
                        "The configured LOG_FORMAT value '{val}' is not recognized.\n  \
                         Supported format options are 'pretty' (default) or 'compact'."
                    ),
                    "Set a valid log format before starting backend.exe:\n\n  \
                     * PowerShell:\n      \
                         $env:LOG_FORMAT = \"pretty\"\n      \
                         .\\backend.exe\n\n  \
                     * Windows Command Prompt (CMD):\n      \
                         set LOG_FORMAT=pretty\n      \
                         backend.exe\n\n  \
                     * Or specify in your .env file:\n      \
                         LOG_FORMAT=pretty"
                        .to_string(),
                ),
                backend::config::ConfigError::MissingVar(var) => (
                    format!(
                        "The application is running in production mode, which strictly requires\n  \
                         the '{var}' environment variable to secure operations."
                    ),
                    format!(
                        "Set the required environment variable before starting backend.exe:\n\n  \
                         * PowerShell:\n      \
                             $env:{var} = \"your-secret-key-or-value\"\n      \
                             .\\backend.exe\n\n  \
                         * Windows Command Prompt (CMD):\n      \
                             set {var}=your-secret-key-or-value\n      \
                             backend.exe\n\n  \
                         * Or create a .env file in the working directory:\n      \
                             {var}=your-secret-key-or-value"
                    ),
                ),
            };
            log_startup_error(
                &log_dir,
                "Configuration Initialization Failed",
                &err.to_string(),
                &rca,
                &remediation,
            );
            return Err(err.into());
        }
    };

    // 2. Establish SQLite connection to database file
    let conn = match Connection::open(&config.database_path) {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(
                database_path = %config.database_path,
                error = %err,
                "Failed to open SQLite ledger database"
            );
            let cause = format!(
                "Failed to open SQLite database at '{}': {err}",
                config.database_path
            );
            let remediation = format!(
                "1. Verify that the parent directory exists or create it.\n  \
                 2. Verify read/write permissions for the current user in that directory.\n  \
                 3. Ensure no other application or process has an exclusive lock on '{}'.\n  \
                 4. Specify a different path using the DATABASE_PATH environment variable.",
                config.database_path
            );
            log_startup_error(
                &config.log_dir,
                "SQLite Ledger Database Open Failed",
                &cause,
                "The SQLite engine failed to open or create the database file at the specified\n  \
                 path. This typically occurs when parent directories do not exist, disk permissions\n  \
                 are insufficient, or the file is locked exclusively by another process.",
                &remediation,
            );
            return Err(err.into());
        }
    };
    let repo = LedgerRepository::new(conn);

    // 3. Initialize relational database schema with WAL pragmas and immutability triggers
    if let Err(err) = repo.init_schema() {
        tracing::error!(
            database_path = %config.database_path,
            error = %err,
            "Failed to initialize database schema"
        );
        let cause = format!("Failed to initialize database schema: {err}");
        let remediation = format!(
            "1. Inspect the database file using 'sqlite3 {} \"PRAGMA integrity_check;\"'.\n  \
             2. If the database file is corrupted or schema incompatible, back up and remove\n     \
                the database file to allow a fresh schema initialization.\n  \
             3. Check available disk space and verify filesystem support for SQLite WAL mode.",
            config.database_path
        );
        log_startup_error(
            &config.log_dir,
            "Database Schema Initialization Failed",
            &cause,
            "The database schema initialization routine failed while executing DDL migrations,\n  \
             enabling WAL pragmas, or establishing immutability triggers. This indicates a corrupt\n  \
             database file, incompatible schema versions, or an interrupted previous migration.",
            &remediation,
        );
        return Err(err.into());
    }
    tracing::info!(
        "Database schema initialized at '{}' (WAL mode enabled)",
        config.database_path
    );

    // 4. Construct application state with runtime configuration
    let engine = LedgerEngine::new();
    let state = Arc::new(AppState::with_config(engine, repo, config.clone()));

    // 5. Build Axum router with state and diagnostic request-logging middleware
    let app = create_app(state);

    // 6. Bind TCP listener dynamically from config
    let addr = format!("{}:{}", config.host, config.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(err) => {
            tracing::error!(
                bind_address = %addr,
                error = %err,
                "Failed to bind TCP listener"
            );
            let cause = format!("Failed to bind TCP listener to {addr}: {err}");
            let rca = format!(
                "The network listener could not bind to address {addr}.\n  \
                 Another application or previous backend instance is already using port {},\n  \
                 or the operating system denied permission to bind to the interface.",
                config.port
            );
            let remediation = format!(
                "1. Identify and terminate the conflicting process using port {}:\n     \
                 * Windows PowerShell:\n         \
                     Get-Process -Id (Get-NetTCPConnection -LocalPort {}).OwningProcess\n     \
                 * Windows Command Prompt (CMD):\n         \
                     netstat -ano | findstr :{}\n  \
                 2. Or configure money-flow to listen on an alternate port:\n     \
                     $env:PORT=\"3001\"\n     \
                     .\\backend.exe",
                config.port, config.port, config.port
            );
            log_startup_error(
                &config.log_dir,
                "TCP Listener Port Collision / Network Bind Failed",
                &cause,
                &rca,
                &remediation,
            );
            return Err(err.into());
        }
    };

    tracing::info!(
        host = %config.host,
        port = %config.port,
        database_path = %config.database_path,
        log_level = %config.log_level,
        log_dir = %config.log_dir,
        log_format = %config.log_format,
        "Accounting backend service running and ready for traffic"
    );

    println!();
    println!("================================================================================");
    println!("  VAS Enterprise Accounting Engine (money-flow) v0.1.0");
    println!("================================================================================");
    println!("  * Service URL:       http://{addr}");
    println!("  * Swagger UI:        http://{addr}/swagger-ui");
    println!("  * OpenAPI 3.1 Spec:  http://{addr}/api-docs/openapi.json");
    println!("  * Health Endpoint:   http://{addr}/health");
    println!("  * Database Path:     {}", config.database_path);
    println!("  * Log Verbosity:     {}", config.log_level);
    println!("  * Log Directory:     {}", config.log_dir);
    println!("  * Log Format:        {}", config.log_format);
    println!("================================================================================");
    println!("  Press Ctrl+C to gracefully stop the service.");
    println!();

    // 7. Start Axum server
    axum::serve(listener, app).await?;

    Ok(())
}
