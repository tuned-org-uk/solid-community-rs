//! Middleware stack: CORS, credential extraction, and authz wiring.
//!
//! ## Logging
//!
//! `info!`  — every incoming request (method + URI) and its final status after
//!             CORS decoration.
//! `debug!` — origin header value used when setting `Access-Control-Allow-Origin`;
//!             credential extraction outcome.

use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request, Response},
    middleware::Next,
};
use tracing::{debug, info};

use authz::credentials::Credentials;

// ── credential extraction ─────────────────────────────────────────────────

/// Extract a `Credentials` value from an HTTP request.
///
/// Supports `Authorization: Bearer <token>` only for now.
/// The resulting `Credentials` is injected into Axum request extensions so
/// that the authz middleware can read it without re-parsing headers.
///
/// When no `Authorization` header is present an **anonymous** `Credentials`
/// (empty agent, no issuer) is returned — the authz layer decides whether
/// anonymous access is permitted for the requested resource.
pub fn extract_credentials(req: &Request<Body>) -> Credentials {
    let maybe_bearer = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::to_owned);

    match maybe_bearer {
        Some(token) => {
            debug!(token_prefix = &token[..token.len().min(8)], "extract_credentials: Bearer token present");
            Credentials {
                agent:  Some(token),
                issuer: None,
            }
        }
        None => {
            debug!("extract_credentials: no Authorization header → anonymous");
            Credentials {
                agent:  None,
                issuer: None,
            }
        }
    }
}

// ── CORS middleware ───────────────────────────────────────────────────────

/// Adds CORS headers to every response.
///
/// Also extracts credentials from the `Authorization` header and stores them
/// in request extensions for downstream middleware / handlers.
///
/// Mirrors the TypeScript `CorsHandler`.
pub async fn cors_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let method = req.method().clone();
    let uri    = req.uri().clone();

    let origin = req.headers().get("origin").cloned();
    debug!(
        method = %method,
        uri = %uri,
        origin = origin
            .as_ref()
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<none>"),
        "cors_middleware: incoming request"
    );

    // Extract credentials and inject into extensions.
    let credentials = extract_credentials(&req);
    req.extensions_mut().insert(credentials);

    let mut res = next.run(req).await;

    let status = res.status();
    info!(method = %method, uri = %uri, status = status.as_u16(),
          "cors_middleware: response ready");

    let headers = res.headers_mut();
    if let Some(o) = origin {
        debug!(origin = ?o, "cors_middleware: echoing Origin in ACAO header");
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            o,
        );
    } else {
        debug!("cors_middleware: no Origin header, using wildcard ACAO");
        headers.insert(
            HeaderName::from_static("access-control-allow-origin"),
            HeaderValue::from_static("*"),
        );
    }
    headers.insert(
        HeaderName::from_static("access-control-allow-headers"),
        HeaderValue::from_static(
            "Authorization, Content-Type, If-Match, If-None-Match, \
             If-Modified-Since, If-Unmodified-Since",
        ),
    );
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static(
            "ETag, Last-Modified, Location, Link, WAC-Allow",
        ),
    );
    res
}
