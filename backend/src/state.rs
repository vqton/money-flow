use crate::config::AppConfig;
use crate::database::LedgerRepository;
use crate::ledger::LedgerEngine;

/// Application state shared across Axum HTTP handlers.
pub struct AppState {
    /// Pure accounting invariant validation engine.
    pub engine: LedgerEngine,
    /// Thread-safe SQLite ledger repository.
    pub repo: LedgerRepository,
    /// Application runtime configuration.
    pub config: AppConfig,
}

impl AppState {
    /// Creates application state with default configuration.
    pub fn new(engine: LedgerEngine, repo: LedgerRepository) -> Self {
        Self {
            engine,
            repo,
            config: AppConfig::default(),
        }
    }

    /// Creates application state with custom configuration.
    pub fn with_config(engine: LedgerEngine, repo: LedgerRepository, config: AppConfig) -> Self {
        Self {
            engine,
            repo,
            config,
        }
    }
}
