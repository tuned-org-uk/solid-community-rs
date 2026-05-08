//! Test suite runner — collects suites, runs every case, prints a summary.
//!
//! Output format mirrors TAP (Test Anything Protocol) so it is readable by
//! humans and parseable by CI tooling:
//!
//! ```text
//! TAP version 14
//! # resource-crud
//! ok 1 - PUT creates a document
//! ok 2 - GET returns the stored body
//! not ok 3 - DELETE returns 204
//!   ---
//!   message: expected HTTP 204, got 404
//!   ...
//! # containers
//! ok 4 - POST to container creates child
//! …
//! 1..N
//! # passed: N-1  failed: 1
//! ```

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::client::SolidClient;

// ── public API ────────────────────────────────────────────────────────────

/// Configuration passed to the test runner.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Base URL of the server under test (e.g. `http://localhost:3000/`).
    pub base_url:   String,
    /// Optional substring filter; only suites whose name contains this value
    /// are executed (case-insensitive).
    pub filter:     Option<String>,
    /// When `true`, print every request/response line even for passing tests.
    pub verbose:    bool,
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
}

/// A single test case: a name and an async function.
pub struct Case {
    pub name: String,
    pub run:  Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>,
}

/// A named group of test cases.
pub struct Suite {
    pub name:  String,
    pub cases: Vec<Case>,
}

/// Owns the list of suites and drives execution.
pub struct TestSuite {
    config: RunConfig,
    suites: Vec<Suite>,
}

impl TestSuite {
    /// Build all known test suites from `config`.
    pub fn new(config: RunConfig) -> Self {
        let client = Arc::new(
            SolidClient::new(&config.base_url, config.timeout_ms, config.verbose)
                .expect("building integration-test HTTP client"),
        );

        let mut all: Vec<Suite> = Vec::new();

        // Register every suite module here.
        all.push(crate::suites::health::suite(Arc::clone(&client)));
        all.push(crate::suites::resource_crud::suite(Arc::clone(&client)));
        all.push(crate::suites::containers::suite(Arc::clone(&client)));
        all.push(crate::suites::content_negotiation::suite(Arc::clone(&client)));
        all.push(crate::suites::error_responses::suite(Arc::clone(&client)));
        all.push(crate::suites::wac::suite(Arc::clone(&client)));

        // Apply optional name filter.
        let suites = if let Some(ref f) = config.filter {
            let f = f.to_lowercase();
            all.into_iter()
                .filter(|s| s.name.to_lowercase().contains(&f))
                .collect()
        } else {
            all
        };

        Self { config, suites }
    }

    /// Execute all suites and print a TAP-formatted report.
    ///
    /// Returns `true` if all cases passed.
    pub async fn run(self) -> bool {
        println!("TAP version 14");

        let mut counter  = 0usize;
        let mut failures = 0usize;

        for suite in self.suites {
            println!("# {}", suite.name);

            for case in suite.cases {
                counter += 1;
                let n    = counter;
                let name = case.name;

                match case.run.await {
                    Ok(()) => {
                        println!("ok {n} - {name}");
                    }
                    Err(e) => {
                        failures += 1;
                        println!("not ok {n} - {name}");
                        println!("  ---");
                        for line in e.to_string().lines() {
                            println!("  {line}");
                        }
                        println!("  ...");
                    }
                }
            }
        }

        println!("1..{counter}");
        println!("# passed: {}  failed: {failures}", counter - failures);

        failures == 0
    }
}

// ── builder helpers (used by suite modules) ───────────────────────────────

/// Construct a [`Case`] from a name and a `Future`.
pub fn case<F, Fut>(name: impl Into<String>, f: F) -> Case
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Case {
        name: name.into(),
        run:  Box::pin(f()),
    }
}
