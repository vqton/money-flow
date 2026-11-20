use std::env;

/// Application runtime configuration adhering to 12-factor appliance principles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Server host IP / hostname to bind (env: HOST).
    pub host: String,
    /// Server TCP port to bind (env: PORT).
    pub port: u16,
    /// Path to the SQLite ledger database file (env: DATABASE_PATH).
    pub database_path: String,
    /// Directory where disaster recovery database snapshots are saved (env: BACKUP_DIR).
    pub backup_dir: String,
    /// Log verbosity filter for tracing (env: LOG_LEVEL).
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            database_path: "accounting.db".to_string(),
            backup_dir: "./backups".to_string(),
            log_level: "info".to_string(),
        }
    }
}

impl AppConfig {
    /// Loads configuration values from environment variables or falls back to specified defaults.
    pub fn from_env() -> Self {
        let default = Self::default();

        let host = env::var("HOST").unwrap_or(default.host);
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(default.port);
        let database_path = env::var("DATABASE_PATH").unwrap_or(default.database_path);
        let backup_dir = env::var("BACKUP_DIR").unwrap_or(default.backup_dir);
        let log_level = env::var("LOG_LEVEL").unwrap_or(default.log_level);

        Self {
            host,
            port,
            database_path,
            backup_dir,
            log_level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.database_path, "accounting.db");
        assert_eq!(config.backup_dir, "./backups");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_app_config_from_env_fallback() {
        // Without setting env vars (or with invalid PORT), it should fallback to defaults
        let config = AppConfig::from_env();
        assert_eq!(config.database_path, "accounting.db");
    }
}
