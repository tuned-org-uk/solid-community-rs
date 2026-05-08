//! WAC (Web Access Control) integration test suite.
//!
//! Tests the access-control enforcement layer:
//!   - Unprotected resources are accessible without credentials.
//!   - Resources whose path ends in `/.acl` (or whose path is listed in
//!     the server's ACL store as restricted) return 401 to anonymous requests.
//!   - The `WAC-Allow` header is present on responses to protected resources.
//!
//! These tests exercise the `authz_middleware` wired in `pipeline.rs`.
//!
//! Note: the default `PassThroughAuthorizer` used in integration-test runs
//! permits all access, so the 401 test relies on hitting the dedicated
//! `.acl` sentinel path — the server returns 401 for anonymous GET on any
//! path whose name ends with `.acl` to prevent credential leakage.

use std::sync::Arc;

use anyhow::Result;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{
    client::SolidClient,
    runner::{case, Suite},
};

/// Generate a unique base path for this test run.
fn uniq_container() -> String {
    format!("/wac-test-{}/", Uuid::new_v4())
}

pub fn suite(client: Arc<SolidClient>) -> Suite {
    Suite {
        name: "wac".into(),
        cases: vec![
            // ── 1. GET on an unprotected resource returns 200 ─────────────
            {
                let c = Arc::clone(&client);
                case("GET on unprotected resource returns 200", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let path = format!("/wac-public-{}.txt", Uuid::new_v4());
                        c.put(&path, "public content", "text/plain").await?;

                        let resp = c.get(&path).await?;
                        SolidClient::assert_status(&resp, StatusCode::OK)?;

                        let _ = c.delete(&path).await;
                        Ok(())
                    }
                })
            },

            // ── 2. GET on an ACL resource without credentials → 401 ───────
            //
            // The server MUST NOT expose raw ACL documents to anonymous
            // agents (Solid Protocol §4.1, WAC §3).  A GET on any path
            // ending with `.acl` without an `Authorization` header must
            // return 401 Unauthorized.
            {
                let c = Arc::clone(&client);
                case("GET on ACL resource without credentials returns 401", move || {
                    let c = Arc::clone(&c);
                    async move {
                        // First PUT the resource so the path exists.
                        let acl_path = format!("/wac-test-{}.ttl.acl", Uuid::new_v4());
                        c.put(
                            &acl_path,
                            "@prefix acl: <http://www.w3.org/ns/auth/acl#> .",
                            "text/turtle",
                        )
                        .await?;

                        // Unauthenticated GET on an .acl path → 401.
                        let resp = c.get(&acl_path).await?;
                        let status = resp.status();
                        // 401 is the expected governance response.
                        // 200 would mean the server is leaking ACL documents.
                        if status != StatusCode::UNAUTHORIZED {
                            anyhow::bail!(
                                "expected 401 Unauthorized on .acl path, got {status}"
                            );
                        }

                        let _ = c.delete(&acl_path).await;
                        Ok(())
                    }
                })
            },

            // ── 3. WAC-Allow header is present in 401 response ────────────
            //
            // When a 401 is returned the server SHOULD include a `WAC-Allow`
            // header advertising what public access modes are permitted
            // (which for a protected resource is none, but the header must
            // still be present so clients can inspect it).
            {
                let c = Arc::clone(&client);
                case("WAC-Allow header is present on protected resource response", move || {
                    let c = Arc::clone(&c);
                    async move {
                        let acl_path = format!("/wac-test-{}.ttl.acl", Uuid::new_v4());
                        c.put(
                            &acl_path,
                            "@prefix acl: <http://www.w3.org/ns/auth/acl#> .",
                            "text/turtle",
                        )
                        .await?;

                        let resp = c.get(&acl_path).await?;
                        // The WAC-Allow header must be present regardless of
                        // whether the response is 200 or 401.
                        let has_wac_allow = resp
                            .headers()
                            .contains_key("wac-allow");
                        if !has_wac_allow {
                            anyhow::bail!(
                                "expected WAC-Allow header on response to .acl path, \
                                 got headers: {:?}",
                                resp.headers()
                            );
                        }

                        let _ = c.delete(&acl_path).await;
                        Ok(())
                    }
                })
            },
        ],
    }
}
