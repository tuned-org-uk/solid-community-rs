//! Builds the Axum router that represents the full request pipeline.
//!
//! Wires together middleware, route handlers, and the authz layer.
//!
//! ## Authz wiring
//!
//! Every request flows through:
//!   1. `cors_middleware`   — CORS headers + credential extraction
//!   2. `authz_middleware`  — PermissionReader → Authorizer
//!   3. LDP handler         — actual store operation
//!
//! A `PassThroughAuthorizer` is provided as the default so that existing
//! integration tests continue to pass without credentials.

use axum::{Router, body::Body, http::{Request, StatusCode}, middleware::{self, Next}, response::{IntoResponse, Response}};
use std::sync::Arc;
use tracing::debug;

use authz::{
    authorizer::Authorizer,
    credentials::Credentials,
    permissions::{AccessMap, PermissionReader},
};
use http_types::ResourceIdentifier;

use crate::{
    middleware::cors_middleware,
    routing::ldp_router,
    store::LdpStore,
};

// ── pass-through default implementations ─────────────────────────────────

/// An `Authorizer` that permits every request.
/// Used as the default so existing tests pass without supplying credentials.
pub struct PassThroughAuthorizer;

#[async_trait::async_trait]
impl Authorizer for PassThroughAuthorizer {
    async fn authorize(
        &self,
        _credentials: &Credentials,
        _requested_modes: &AccessMap,
        _available_permissions: &authz::permissions::PermissionMap,
    ) -> Result<(), http_types::SolidError> {
        Ok(())
    }
}

/// A `PermissionReader` that reports all modes as permitted.
pub struct PassThroughPermissionReader;

#[async_trait::async_trait]
impl PermissionReader for PassThroughPermissionReader {
    async fn read(
        &self,
        _credentials: &Credentials,
        requested_modes: &AccessMap,
    ) -> anyhow::Result<authz::permissions::PermissionMap> {
        use std::collections::HashMap;
        let mut map = authz::permissions::PermissionMap::new();
        for (resource, modes) in requested_modes {
            let mut perms = HashMap::new();
            for mode in modes {
                perms.insert(*mode, true);
            }
            map.insert(resource.clone(), perms);
        }
        Ok(map)
    }
}

// ── authz middleware ──────────────────────────────────────────────────────

/// Axum middleware that enforces access control on every request.
///
/// It reads the `Credentials` injected by `cors_middleware`, builds an
/// `AccessMap` from the request method and path, calls the `PermissionReader`,
/// then asks the `Authorizer` to approve or reject.
///
/// For now the access map contains a single entry: the requested resource +
/// the HTTP method mapped to its corresponding `AccessMode`.
pub async fn authz_middleware(
    axum::extract::State(authz_state): axum::extract::State<AuthzState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    use http_types::operation::AccessMode;
    use std::collections::{HashMap, HashSet};

    // Derive resource identifier from the request URI.
    let path = req.uri().path().to_owned();
    let resource = ResourceIdentifier { path: path.clone() };

    // Map HTTP method → AccessMode.
    let mode = match req.method().as_str() {
        "GET" | "HEAD" | "OPTIONS" => AccessMode::Read,
        "PUT" | "POST" | "PATCH"   => AccessMode::Write,
        "DELETE"                   => AccessMode::Write,
        _                          => AccessMode::Read,
    };

    let mut modes_set = HashSet::new();
    modes_set.insert(mode);
    let mut access_map: AccessMap = HashMap::new();
    access_map.insert(resource, modes_set);

    // Pull credentials from extensions (set by cors_middleware).
    let credentials = req
        .extensions()
        .get::<Credentials>()
        .cloned()
        .unwrap_or(Credentials { agent: None, issuer: None });

    debug!(
        path = %path,
        agent = ?credentials.agent,
        "authz_middleware: checking permissions"
    );

    // Read permissions then authorize.
    let permission_map = match authz_state.permission_reader.read(&credentials, &access_map).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "authz_middleware: permission_reader failed → 500");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(e) = authz_state.authorizer.authorize(&credentials, &access_map, &permission_map).await {
        debug!(path = %path, error = %e, "authz_middleware: denied → 401");
        return (
            StatusCode::UNAUTHORIZED,
            [(axum::http::header::WWW_AUTHENTICATE, "Bearer")],
            format!("401 Unauthorized: {e}"),
        ).into_response();
    }

    next.run(req).await
}

// ── shared authz state ────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AuthzState {
    pub permission_reader: Arc<dyn PermissionReader>,
    pub authorizer:        Arc<dyn Authorizer>,
}

// ── pipeline ──────────────────────────────────────────────────────────────

/// Owns the assembled request pipeline.
pub struct RequestPipeline {
    store:            Arc<LdpStore>,
    authz_state:      AuthzState,
}

impl RequestPipeline {
    /// Create a pipeline with explicit authz implementations.
    pub fn with_authz(
        permission_reader: Arc<dyn PermissionReader>,
        authorizer:        Arc<dyn Authorizer>,
    ) -> Self {
        Self {
            store:       LdpStore::new(),
            authz_state: AuthzState { permission_reader, authorizer },
        }
    }

    /// Construct the full Axum `Router`.
    pub fn build_router(&self) -> Router {
        ldp_router(Arc::clone(&self.store))
            .layer(middleware::from_fn_with_state(
                self.authz_state.clone(),
                authz_middleware,
            ))
            .layer(middleware::from_fn(cors_middleware))
    }
}

impl Default for RequestPipeline {
    /// Default pipeline uses pass-through authz (permits everything).
    fn default() -> Self {
        Self::with_authz(
            Arc::new(PassThroughPermissionReader),
            Arc::new(PassThroughAuthorizer),
        )
    }
}
