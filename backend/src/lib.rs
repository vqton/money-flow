#![recursion_limit = "256"]

pub mod closing;
pub mod config;
pub mod database;
pub mod dto;
pub mod handlers;
pub mod ledger;
pub mod models;
pub mod openapi;
pub mod state;

pub use closing::PeriodClosingEngine;
pub use config::AppConfig;
pub use database::LedgerRepository;
pub use handlers::create_app;
pub use ledger::{AccountingError, LedgerEngine};
pub use models::{
    AccountingPeriod, AuditLogEntry, BalanceSheetReport, IncomeStatementReport, JournalVoucher,
    VoucherLine, VoucherStatus,
};
pub use openapi::{generate_openapi_spec, swagger_ui_html};
pub use state::AppState;
