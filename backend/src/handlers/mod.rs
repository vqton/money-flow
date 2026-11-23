//! Axum HTTP request handlers, middleware, and application router.

pub mod reports_period;
pub mod system;
pub mod voucher;

use axum::{
    Json, Router,
    extract::Request,
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

pub use reports_period::{
    close_period, get_balance_sheet, get_income_statement, get_trial_balance, list_periods,
    reopen_period,
};
pub use system::{create_backup, restore_backup_handler};
pub use voucher::{
    create_voucher, get_voucher, list_audit_logs_handler, list_vouchers, void_voucher_handler,
};

/// Health check handler returning service status and version.
pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": "0.1.0"
    }))
}

/// Serves the OpenAPI 3.1 JSON specification.
pub async fn openapi_json_handler() -> Json<serde_json::Value> {
    Json(crate::openapi::generate_openapi_spec())
}

/// Serves the interactive Swagger UI interface.
pub async fn swagger_ui_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(crate::openapi::swagger_ui_html())
}

/// Request logging middleware recording HTTP method, URI path, status code, and latency.
pub async fn request_logging_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = std::time::Instant::now();

    let response = next.run(req).await;

    let latency = start.elapsed();
    let status = response.status();

    if status.is_server_error() {
        tracing::error!(
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request failed with server error"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request client error"
        );
    } else {
        tracing::info!(
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request processed"
        );
    }

    response
}

/// Constructs the primary Axum router wiring all endpoints and application state.
pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/api-docs/openapi.json", get(openapi_json_handler))
        .route("/swagger-ui", get(swagger_ui_handler))
        .route("/api/v1/vouchers", post(create_voucher).get(list_vouchers))
        .route("/api/v1/vouchers/{id}", get(get_voucher))
        .route("/api/v1/vouchers/{id}/void", post(void_voucher_handler))
        .route("/api/v1/audit-logs", get(list_audit_logs_handler))
        .route("/api/v1/reports/trial-balance", get(get_trial_balance))
        .route("/api/v1/reports/balance-sheet", get(get_balance_sheet))
        .route(
            "/api/v1/reports/income-statement",
            get(get_income_statement),
        )
        .route("/api/v1/periods", get(list_periods))
        .route("/api/v1/periods/close", post(close_period))
        .route("/api/v1/periods/reopen", post(reopen_period))
        .route("/api/v1/system/backup", post(create_backup))
        .route("/api/v1/system/restore", post(restore_backup_handler))
        .route(
            "/api/v1/accounts",
            get(crate::accounts::list_accounts).post(crate::accounts::create_account),
        )
        .layer(from_fn(request_logging_middleware))
        .with_state(state)
}
