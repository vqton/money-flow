//! Data Transfer Objects (DTOs) for the VAS Accounting API.
//!
//! Submodules:
//! - [`requests`]: HTTP request and response models.
//! - [`error`]: `ApiError` Anti-Corruption Layer mapping domain errors to HTTP statuses.
//! - [`idempotency`]: Idempotency key extractor and request hashing.

pub mod error;
pub mod idempotency;
pub mod requests;

pub use error::ApiError;
pub use idempotency::{IdempotencyKey, compute_request_hash, validate_idempotency_key};
pub use requests::*;
