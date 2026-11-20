#![recursion_limit = "256"]

pub mod auth;
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

pub use auth::{AuthenticatedUser, Claims, RequireController, Role};
pub use closing::PeriodClosingEngine;
pub use config::{AppConfig, LogFormat};
pub use database::{LedgerRepository, SaveVoucherOutcome};
pub use diagnostics::{
    init_dual_sink_subscriber, log_startup_error, setup_panic_hook, should_enable_ansi,
};
pub use dto::{IdempotencyKey, compute_request_hash, validate_idempotency_key};
pub use handlers::create_app;
pub use ledger::{AccountingError, LedgerEngine};
pub use models::{
    AccountingPeriod, AuditLogEntry, BalanceSheetReport, IdempotencyRecord, IncomeStatementReport,
    JournalVoucher, VoucherLine, VoucherStatus,
};
pub use openapi::{generate_openapi_spec, swagger_ui_html};
pub use state::AppState;
