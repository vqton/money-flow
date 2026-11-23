#![recursion_limit = "256"]

pub mod accounts;
pub mod auth;
pub mod catalog;
pub mod closing;
pub mod config;
pub mod database;
pub mod diagnostics;
pub mod dto;
pub mod handlers;
pub mod ledger;
pub mod models;
pub mod openapi;
pub mod state;

pub use accounts::{CreateAccountRequest, ListAccountsQuery};
pub use auth::{AuthenticatedUser, Claims, RequireAdmin, RequireController, Role};
pub use catalog::{AccountSeed, STATUTORY_ACCOUNTS};
pub use closing::{
    PeriodClosingEngine, fiscal_period_for_date, fiscal_year_bounds, fiscal_year_for_date,
};
pub use config::{AppConfig, ConfigError, LogFormat};
pub use database::{LedgerRepository, RestoreOutcome, SaveVoucherOutcome, execute_cli_restore};
pub use diagnostics::{
    init_dual_sink_subscriber, log_startup_error, setup_panic_hook, should_enable_ansi,
};
pub use dto::{
    IdempotencyKey, RestoreRequest, RestoreResponse, compute_request_hash, validate_idempotency_key,
};
pub use handlers::create_app;
pub use ledger::{AccountValidator, AccountingError, LedgerEngine, StatutoryAccountCatalog};
pub use models::{
    Account, AccountClass, AccountingPeriod, AuditLogEntry, BalanceSheetReport, IdempotencyRecord,
    IncomeStatementReport, JournalVoucher, NormalBalance, VoucherLine, VoucherStatus,
};
pub use openapi::{generate_openapi_spec, swagger_ui_html};
pub use state::AppState;
