// Original content reconstructed from backup
use std::env;
use thiserror::Error;

/// Configuration errors during startup.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// A required environment variable is missing or empty.
    #[error("Required environment variable '{0}' is missing or empty")]
    MissingVar(String),
    /// An invalid log format was supplied.
    #[error("Invalid log format '{0}': expected 'pretty' or 'compact'")]
    InvalidLogFormat(String),
    /// An invalid fiscal year start month was supplied.
    #[error("Invalid fiscal year start month '{0}': expected an integer between 1 and 12")]
    InvalidFiscalYearStartMonth(String),
}

/// Console logging output style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Multi-line layout with main event line and indented structured fields.
    #[default]
    Pretty,
    /// Single-line condensed layout with inline structured fields.
    Compact,
}

impl LogFormat {
    /// Parses a log format string case-insensitively with whitespace trimming.
    pub fn parse(s: &str) -> Result<Self, ConfigError> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("pretty") {
            Ok(Self::Pretty)
        } else if trimmed.eq_ignore_ascii_case("compact") {
            Ok(Self::Compact)
        } else {
            Err(ConfigError::InvalidLogFormat(s.to_string()))
        }
    }
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pretty => write!(f, "pretty"),
            Self::Compact => write!(f, "compact"),
        }
    }
}

/// Fallback JWT secret used exclusively in test configurations.
pub const DEFAULT_TEST_JWT_SECRET: &str = "test-secret-key-with-minimum-32-bytes-length-12345";

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
    /// Directory where diagnostic and audit log files are stored (env: LOG_DIR).
    pub log_dir: String,
    /// Log output format style for console output (env: LOG_FORMAT).
    pub log_format: LogFormat,
    /// Secret key used for signing and verifying Bearer JWT tokens (env: JWT_SECRET).
    pub jwt_secret: String,
    /// Fiscal year start month (1..=12, env: FISCAL_YEAR_START_MONTH, default: 1).
    pub fiscal_year_start_month: u8,
    /// Directory where attachment files are stored (env: ATTACHMENT_DIR).
    pub attachment_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            database_path: "accounting.db".to_string(),
            backup_dir: "./backups".to_string(),
            log_level: "info".to_string(),
            log_dir: "./logs".to_string(),
            log_format: LogFormat::Pretty,
            jwt_secret: DEFAULT_TEST_JWT_SECRET.to_string(),
            fiscal_year_start_month: 1,
            attachment_dir: "./attachments".to_string(),
        }
    }
}

impl AppConfig {
    /// Parses key-value pairs from an optional `.env` file if present, searching both the current
    /// working directory and the directory containing the binary, traversing parent directories up to project root.
    pub fn read_dotenv_map() -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        let mut search_dirs = Vec::new();

        if let Ok(cwd) = std::env::current_dir() {
            let mut curr = Some(cwd.as_path());
            for _ in 0..5 {
                if let Some(dir) = curr {
                    search_dirs.push(dir.to_path_buf());
                    curr = dir.parent();
                } else {
                    break;
                }
            }
        }

        if let Ok(exe_path) = std::env::current_exe() && let Some(exe_dir) = exe_path.parent() {
            let mut curr = Some(exe_dir);
            for _ in 0..5 {
                if let Some(dir) = curr {
                    search_dirs.push(dir.to_path_buf());
                    curr = dir.parent();
                } else {
                    break;
                }
            }
        }

        for dir in search_dirs {
            let dotenv_path = dir.join(".env");
            if dotenv_path.is_file() && let Ok(content) = std::fs::read_to_string(&dotenv_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if let Some((key, val)) = trimmed.split_once('=') {
                        let key = key.trim().to_string();
                        let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
                        map.insert(key, val);
                    }
                }
                break;
            }
        }
        map
    }

    /// Loads configuration values from environment variables or falls back to specified defaults.
    /// In non-test runs, requires `JWT_SECRET` to be provided and non-empty.
    pub fn from_env() -> Result<Self, ConfigError> {
        #[cfg(test)] {
            Self::from_env_with_fallback(|k| env::var(k).ok())
        }
        #[cfg(not(test))] {
            let dotenv = Self::read_dotenv_map();
            let lookup = |k: &str| env::var(k).ok().or_else(|| dotenv.get(k).cloned());
            Self::from_env_strict(lookup)
        }
    }

    /// Internal loader enforcing strict validation for production environments.
    pub fn from_env_strict<F>(getter: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let default = Self::default();
        let host = getter("HOST").filter(|s| !s.trim().is_empty()).unwrap_or(default.host);
        let port = getter("PORT").and_then(|p| p.parse::<u16>().ok()).unwrap_or(default.port);
        let database_path = getter("DATABASE_PATH").filter(|s| !s.trim().is_empty()).unwrap_or(default.database_path);
        let backup_dir = getter("BACKUP_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.backup_dir);
        let log_level = getter("LOG_LEVEL").filter(|s| !s.trim().is_empty()).unwrap_or(default.log_level);
        let log_dir = getter("LOG_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.log_dir);
        let log_format = match getter("LOG_FORMAT") {
            Some(s) if !s.trim().is_empty() => LogFormat::parse(&s)?,
            _ => default.log_format,
        };
        let fiscal_year_start_month = match getter("FISCAL_YEAR_START_MONTH") {
            Some(s) if !s.trim().is_empty() => {
                let trimmed = s.trim();
                let month: u8 = trimmed
                    .parse()
                    .map_err(|_| ConfigError::InvalidFiscalYearStartMonth(s.to_string()))?;
                if !(1..=12).contains(&month) {
                    return Err(ConfigError::InvalidFiscalYearStartMonth(s.to_string()));
                }
                month
            }
            _ => default.fiscal_year_start_month,
        };
        let attachment_dir = getter("ATTACHMENT_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.attachment_dir);
        let jwt_secret = match getter("JWT_SECRET") {
            Some(secret) if !secret.trim().is_empty() => secret,
            _ => return Err(ConfigError::MissingVar("JWT_SECRET".to_string())),
        };
        Ok(Self {
            host,
            port,
            database_path,
            backup_dir,
            log_level,
            log_dir,
            log_format,
            jwt_secret,
            fiscal_year_start_month,
            attachment_dir,
        })
    }

    /// Internal loader falling back to test secret when `JWT_SECRET` is unset.
    pub fn from_env_with_fallback<F>(getter: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let default = Self::default();
        let host = getter("HOST").filter(|s| !s.trim().is_empty()).unwrap_or(default.host);
        let port = getter("PORT").and_then(|p| p.parse::<u16>().ok()).unwrap_or(default.port);
        let database_path = getter("DATABASE_PATH").filter(|s| !s.trim().is_empty()).unwrap_or(default.database_path);
        let backup_dir = getter("BACKUP_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.backup_dir);
        let log_level = getter("LOG_LEVEL").filter(|s| !s.trim().is_empty()).unwrap_or(default.log_level);
        let log_dir = getter("LOG_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.log_dir);
        let log_format = match getter("LOG_FORMAT") {
            Some(s) if !s.trim().is_empty() => LogFormat::parse(&s)?,
            _ => default.log_format,
        };
        let fiscal_year_start_month = match getter("FISCAL_YEAR_START_MONTH") {
            Some(s) if !s.trim().is_empty() => {
                let trimmed = s.trim();
                let month: u8 = trimmed
                    .parse()
                    .map_err(|_| ConfigError::InvalidFiscalYearStartMonth(s.to_string()))?;
                if !(1..=12).contains(&month) {
                    return Err(ConfigError::InvalidFiscalYearStartMonth(s.to_string()));
                }
                month
            }
            _ => default.fiscal_year_start_month,
        };
        let attachment_dir = getter("ATTACHMENT_DIR").filter(|s| !s.trim().is_empty()).unwrap_or(default.attachment_dir);
        let jwt_secret = getter("JWT_SECRET").unwrap_or_else(|| DEFAULT_TEST_JWT_SECRET.to_string());
        Ok(Self {
            host,
            port,
            database_path,
            backup_dir,
            log_level,
            log_dir,
            log_format,
            jwt_secret,
            fiscal_year_start_month,
            attachment_dir,
        })
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
        assert_eq!(config.log_dir, "./logs");
        assert_eq!(config.jwt_secret, DEFAULT_TEST_JWT_SECRET);
    }

    #[test]
    fn test_app_config_from_env_fallback() {
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.database_path, "accounting.db");
        assert_eq!(config.log_dir, "./logs");
        assert_eq!(config.jwt_secret, DEFAULT_TEST_JWT_SECRET);
    }

    #[test]
    fn test_app_config_custom_log_dir() {
        let config = AppConfig::from_env_with_fallback(|k| {
            if k == "LOG_DIR" {
                Some("/var/log/money-flow".to_string())
            } else {
                None
            }
        })
        .unwrap();
        assert_eq!(config.log_dir, "/var/log/money-flow");

        let strict_config = AppConfig::from_env_strict(|k| match k {
            "JWT_SECRET" => Some("custom-production-secret-key-12345".to_string()),
            "LOG_DIR" => Some("D:\\money-flow\\custom-logs".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(strict_config.log_dir, "D:\\money-flow\\custom-logs");
    }

    #[test]
    fn test_app_config_strict_missing_secret() {
        let err = AppConfig::from_env_strict(|_| None).unwrap_err();
        assert_eq!(err, ConfigError::MissingVar("JWT_SECRET".to_string()));
    }

    #[test]
    fn test_app_config_strict_empty_secret() {
        let err = AppConfig::from_env_strict(|k| {
            if k == "JWT_SECRET" {
                Some("   ".to_string())
            } else {
                None
            }
        })
        .unwrap_err();
        assert_eq!(err, ConfigError::MissingVar("JWT_SECRET".to_string()));
    }

    #[test]
    fn test_app_config_strict_with_secret() {
        let config = AppConfig::from_env_strict(|k| {
            if k == "JWT_SECRET" {
                Some("custom-production-secret-key-12345".to_string())
            } else {
                None
            }
        })
        .unwrap();
        assert_eq!(config.jwt_secret, "custom-production-secret-key-12345");
    }

    #[test]
    fn test_app_config_whitespace_log_dir_falls_back_to_default() {
        let config = AppConfig::from_env_with_fallback(|k| {
            if k == "LOG_DIR" {
                Some("   ".to_string())
            } else {
                None
            }
        })
        .unwrap();
        assert_eq!(config.log_dir, "./logs");

        let strict_config = AppConfig::from_env_strict(|k| match k {
            "JWT_SECRET" => Some("custom-production-secret-key-12345".to_string()),
            "LOG_DIR" => Some("   \t\n ".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(strict_config.log_dir, "./logs");
    }

    #[test]
    fn test_log_format_default() {
        let default_format = LogFormat::default();
        assert_eq!(default_format, LogFormat::Pretty);

        let config = AppConfig::default();
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn test_log_format_parsing_valid() {
        assert_eq!(LogFormat::parse("pretty").unwrap(), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("PRETTY").unwrap(), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("  pretty  ").unwrap(), LogFormat::Pretty);
        assert_eq!(LogFormat::parse("compact").unwrap(), LogFormat::Compact);
        assert_eq!(LogFormat::parse("COMPACT").unwrap(), LogFormat::Compact);
        assert_eq!(LogFormat::parse(" compact\t").unwrap(), LogFormat::Compact);
    }

    #[test]
    fn test_log_format_parsing_invalid() {
        let err = LogFormat::parse("json").unwrap_err();
        assert_eq!(err, ConfigError::InvalidLogFormat("json".to_string()));

        let err2 = LogFormat::parse("").unwrap_err();
        assert_eq!(err2, ConfigError::InvalidLogFormat("".to_string()));
    }

    #[test]
    fn test_app_config_log_format_from_env() {
        let config = AppConfig::from_env_with_fallback(|k| match k {
            "LOG_FORMAT" => Some("compact".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.log_format, LogFormat::Compact);

        let strict_config = AppConfig::from_env_strict(|k| match k {
            "JWT_SECRET" => Some("custom-production-secret-key-12345".to_string()),
            "LOG_FORMAT" => Some("  compact  ".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(strict_config.log_format, LogFormat::Compact);
    }

    #[test]
    fn test_app_config_strict_invalid_log_format_fails() {
        let err = AppConfig::from_env_strict(|k| match k {
            "JWT_SECRET" => Some("custom-production-secret-key-12345".to_string()),
            "LOG_FORMAT" => Some("json".to_string()),
            _ => None,
        })
        .unwrap_err();
        assert_eq!(err, ConfigError::InvalidLogFormat("json".to_string()));
    }

    #[test]
    fn test_fiscal_year_start_month_default() {
        let config = AppConfig::default();
        assert_eq!(config.fiscal_year_start_month, 1);
    }

    #[test]
    fn test_fiscal_year_start_month_valid() {
        let config = AppConfig::from_env_with_fallback(|k| match k {
            "FISCAL_YEAR_START_MONTH" => Some(" 4 ".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.fiscal_year_start_month, 4);

        let strict = AppConfig::from_env_strict(|k| match k {
            "JWT_SECRET" => Some("production-secret-32-bytes-long!!!".to_string()),
            "FISCAL_YEAR_START_MONTH" => Some("12".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(strict.fiscal_year_start_month, 12);
    }

    #[test]
    fn test_fiscal_year_start_month_invalid() {
        for invalid in ["0", "13", "abc", "-1"] {
            let err = AppConfig::from_env_strict(|k| match k {
                "JWT_SECRET" => Some("production-secret-32-bytes-long!!!".to_string()),
                "FISCAL_YEAR_START_MONTH" => Some(invalid.to_string()),
                _ => None,
            })
            .unwrap_err();
            assert_eq!(
                err,
                ConfigError::InvalidFiscalYearStartMonth(invalid.to_string())
            );
        }
    }

    #[test]
    fn test_fiscal_year_start_month_empty_fallback() {
        let config = AppConfig::from_env_with_fallback(|k| match k {
            "FISCAL_YEAR_START_MONTH" => Some("   ".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.fiscal_year_start_month, 1);
    }
}
