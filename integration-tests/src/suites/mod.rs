//! Individual test suite modules.
//!
//! Each module exposes a single `pub fn suite(client: Arc<SolidClient>) -> Suite`
//! function that the runner calls to collect cases.

pub mod health;
pub mod resource_crud;
pub mod containers;
pub mod content_negotiation;
pub mod error_responses;
pub mod wac;
