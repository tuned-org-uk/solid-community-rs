//! Core server: lifecycle, request pipeline, routing, and middleware.
//!
//! Mirrors the TypeScript `src/init`, `src/server`, and `src/http` modules.

pub mod app;
pub mod handler;
pub mod ldp_handlers;
pub mod middleware;
pub mod pipeline;
pub mod routing;
pub mod store;

pub use app::App;
pub use pipeline::{AuthzState, PassThroughAuthorizer, PassThroughPermissionReader, RequestPipeline};
