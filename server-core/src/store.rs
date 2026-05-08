//! In-memory LDP resource store.
//!
//! A simple `Arc<RwLock<HashMap>>` that is shared across all request handlers.
//! Suitable for integration-test runs; swap in `solid-storage` backends for
//! production persistence.
//!
//! ## ResourceStore impl
//!
//! `LdpStore` now implements `solid_storage::resource_store::ResourceStore`
//! (delegating to the existing get / put / delete methods) so that
//! `server-core` and `solid-storage` are no longer disconnected.
//!
//! ## Logging
//!
//! Every mutation emits a `debug!` statement so that test runs clearly show
//! which keys were created, overwritten, or deleted, and which ancestor
//! containers were auto-created.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tracing::debug;

/// One stored resource entry.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Raw body bytes.
    pub body: Vec<u8>,
    /// `Content-Type` string as supplied by the PUT/POST caller.
    pub content_type: String,
    /// True when the path ends with `/` (container semantics).
    pub is_container: bool,
}

/// The shared, thread-safe in-memory store.
#[derive(Debug, Default)]
pub struct LdpStore {
    inner: RwLock<HashMap<String, Entry>>,
}

impl LdpStore {
    /// Create a new store pre-seeded with the root container `/`.
    ///
    /// The root must always exist so that `GET /` and `HEAD /` return 200
    /// even before any resource has been written (health-suite tests 1 & 3).
    pub fn new() -> Arc<Self> {
        let store = Arc::new(Self::default());
        {
            let mut map = store.inner.write().unwrap();
            map.insert(
                "/".to_owned(),
                Entry {
                    body: Vec::new(),
                    content_type: "text/turtle".to_owned(),
                    is_container: true,
                },
            );
            debug!(key = "/", "store::new seeded root container");
        }
        store
    }

    /// Normalise a URL path key (always starts with `/`).
    fn key(path: &str) -> String {
        if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        }
    }

    /// Retrieve a resource.
    pub fn get(&self, path: &str) -> Option<Entry> {
        let k = Self::key(path);
        let result = self.inner.read().unwrap().get(&k).cloned();
        match &result {
            Some(e) => debug!(
                key = %k,
                content_type = %e.content_type,
                body_bytes = e.body.len(),
                is_container = e.is_container,
                "store::get hit"
            ),
            None => debug!(key = %k, "store::get miss"),
        }
        result
    }

    /// Check existence without cloning the body.
    pub fn exists(&self, path: &str) -> bool {
        let k = Self::key(path);
        let found = self.inner.read().unwrap().contains_key(&k);
        debug!(key = %k, found = found, "store::exists");
        found
    }

    /// Store a resource.  Returns `true` if this was a create (did not exist).
    /// Auto-creates every ancestor container.
    pub fn put(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: String,
    ) -> bool {
        let k = Self::key(path);
        let is_container = k.ends_with('/');

        // Auto-create ancestor containers (logged inside ensure_ancestors).
        self.ensure_ancestors(&k);

        let mut map = self.inner.write().unwrap();
        let created = !map.contains_key(&k);
        debug!(
            key = %k,
            content_type = %content_type,
            body_bytes = body.len(),
            is_container = is_container,
            created = created,
            "store::put"
        );
        map.insert(
            k,
            Entry {
                body,
                content_type,
                is_container,
            },
        );
        created
    }

    /// Delete a resource.  Returns `true` if the resource existed.
    pub fn delete(&self, path: &str) -> bool {
        let k = Self::key(path);
        let removed = self.inner.write().unwrap().remove(&k).is_some();
        debug!(key = %k, removed = removed, "store::delete");
        removed
    }

    pub fn is_container(&self, path: &str) -> bool {
        Self::key(path).ends_with('/')
    }

    /// Walk up the path hierarchy and ensure every intermediate container
    /// exists (mirrors CSS auto-container-creation behaviour).
    fn ensure_ancestors(&self, key: &str) {
        let s = key.trim_end_matches('/');
        let mut parts: Vec<&str> = s.splitn(100, '/').collect();
        if parts.len() > 1 {
            parts.pop();
        }

        let mut map = self.inner.write().unwrap();
        let mut current = String::new();
        for part in &parts {
            if part.is_empty() {
                current.push('/');
            } else {
                if !current.ends_with('/') {
                    current.push('/');
                }
                current.push_str(part);
                current.push('/');
            }
            let k = if current == "//" {
                "/".to_owned()
            } else {
                current.clone()
            };
            let inserted = map.entry(k.clone()).or_insert_with(|| {
                debug!(ancestor = %k, "store::ensure_ancestors auto-creating container");
                Entry {
                    body: Vec::new(),
                    content_type: "text/turtle".to_owned(),
                    is_container: true,
                }
            });
            let _ = inserted;
        }
    }
}

// ── solid_storage::ResourceStore bridge ──────────────────────────────────
//
// This impl delegates to the existing LdpStore methods so that server-core
// and solid-storage are no longer disconnected.  Concretely it means any
// code that accepts an `Arc<dyn ResourceStore>` can be handed an LdpStore.

use solid_storage::resource_store::ResourceStoreEntry;

impl solid_storage::resource_store::ResourceStore for LdpStore {
    fn rs_get(&self, path: &str) -> Option<ResourceStoreEntry> {
        self.get(path).map(|e| ResourceStoreEntry {
            body:         e.body,
            content_type: e.content_type,
            is_container: e.is_container,
        })
    }

    fn rs_put(&self, path: &str, body: Vec<u8>, content_type: String) -> bool {
        self.put(path, body, content_type)
    }

    fn rs_delete(&self, path: &str) -> bool {
        self.delete(path)
    }

    fn rs_exists(&self, path: &str) -> bool {
        self.exists(path)
    }
}
