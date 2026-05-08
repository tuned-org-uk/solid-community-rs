# solid-data-governance-rs

A Rust port of the [Solid Community Server](https://github.com/CommunitySolidServer/CommunitySolidServer) (CSS) — a standards-compliant [Solid](https://solidproject.org/) server implementing the LDP, WAC, and WebID-TLS specifications.

---

## Table of Contents

- [Requirements](#requirements)
- [Workspace layout](#workspace-layout)
- [Quickstart](#quickstart)
- [Running the server](#running-the-server)
- [Running integration tests](#running-integration-tests)
- [Unit tests](#unit-tests)
- [CLI reference](#cli-reference)
  - [solid-server](#solid-server)
  - [solid-test](#solid-test)
- [Environment variables](#environment-variables)
- [Architecture](#architecture)
- [Contributing](#contributing)

---

## Requirements

| Tool | Minimum version |
|------|-----------------|
| Rust | 1.76 (stable) |
| Cargo | ships with Rust |

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Workspace layout

Crates live directly at the repository root — there is **no** `crates/` subdirectory.

```
solid-data-governance-rs/
├── Cargo.toml              # workspace manifest
│
├── http-types/             # core domain types
│   └── src/
│       ├── error.rs        # SolidError hierarchy (4xx / 5xx)
│       ├── handler.rs      # OperationHandler async trait
│       ├── identifier.rs   # ResourceIdentifier + helpers
│       ├── metadata.rs     # RepresentationMetadata (Content-Type, ETag, …)
│       ├── operation.rs    # Operation enum (GET, PUT, POST, PATCH, DELETE, HEAD, OPTIONS)
│       └── representation.rs  # Representation (body + metadata)
│
├── solid-storage/          # storage traits and backends
│   └── src/
│       ├── error.rs        # StorageError (NotFound, AlreadyExists, AlreadyExpired, …)
│       ├── key_value.rs    # KeyValueStorage, ExpiringStorage, PassthroughKeyValueStorage
│       ├── resource_store.rs  # ResourceStore, BaseResourceStore, PassthroughStore, ReadOnlyStore
│       └── backends/
│           ├── expiring.rs # WrappedExpiringStorage (TTL + background sweep)
│           ├── file.rs     # JsonFileStorage<V> (file-backed KV store)
│           ├── memory.rs   # MemoryMapStorage<V> (in-memory KV store)
│           └── mod.rs
│
├── authz/                  # authorisation traits
│   └── src/
│       ├── authorizer.rs   # Authorizer async trait
│       ├── credentials.rs  # Credentials (agent WebID + issuer)
│       └── permissions.rs  # PermissionReader async trait + AclPermission flags
│
├── identity/               # account and pod management traits
│   └── src/
│       ├── account.rs      # AccountStore async trait (CRUD + password)
│       ├── client_credentials.rs  # ClientCredentialsStore trait
│       ├── pod.rs          # PodManager + PodStore traits
│       └── webid.rs        # WebIdStore + WebIdLinkStore traits
│
├── static-assets/          # static file serving
│   └── src/
│       ├── entry.rs        # StaticAssetEntry (URL prefix → filesystem path)
│       └── handler.rs      # StaticAssetHandler (MIME detection, path traversal guard)
│
├── server-core/            # HTTP server: router, middleware, pipeline, lifecycle
│   └── src/
│       ├── app.rs          # App + AppConfig (bind, start, graceful shutdown)
│       ├── handler.rs      # OperationHandler bridge to Axum extractors
│       ├── middleware.rs   # CORS + credential extraction + authz tower layers
│       ├── pipeline.rs     # RequestPipeline (operation dispatch + authz wiring)
│       └── routing.rs      # ldp_router() — LDP method × path table
│
├── cli/                    # two runnable binaries
│   └── src/
│       ├── main.rs         # subcommand dispatch (serve | test)
│       ├── serve.rs        # solid-server: CLI args, hostname resolution, server start
│       └── test_runner.rs  # solid-test: CLI args, suite dispatch
│
└── integration-tests/      # HTTP integration test library (used by solid-test)
    └── src/
        ├── client.rs       # SolidClient (reqwest wrapper, TAP assertions)
        ├── lib.rs          # public re-exports
        ├── runner.rs       # TestRunner (suite registry, TAP printer, exit code)
        └── suites/
            ├── containers.rs          # LDP container behaviour
            ├── content_negotiation.rs # Accept / Content-Type negotiation
            ├── error_responses.rs     # 4xx error body and header shape
            ├── health.rs              # root resource liveness checks
            ├── mod.rs
            ├── resource_crud.rs       # PUT / GET / DELETE on documents
            └── wac.rs                 # WAC access control (401, WAC-Allow)
```

---

## Quickstart

```bash
# 1. Clone
git clone https://github.com/Genefold/solid-data-governance-rs.git
cd solid-data-governance-rs

# 2. Build everything
cargo build --release

# 3. Start the server (defaults: http://localhost:3000/)
cargo run --bin solid-server

# 4. In a second terminal — run the integration tests
cargo run --bin solid-test

# how to Log
RUST_LOG=server_core=debug cargo run --bin solid-server
```

---

## Running the server

```bash
# Development build (fastest compile)
cargo run --bin solid-server

# Release build (optimised)
cargo run --release --bin solid-server

# Custom port, host, and base URL
cargo run --bin solid-server -- \
  --port 4000 \
  --host 0.0.0.0 \
  --base-url http://my-server.example/

# File-backed storage (persists data to disk)
cargo run --bin solid-server -- --root-dir ./data

# Verbose logging
cargo run --bin solid-server -- --log-level debug
```

> **Hostname resolution** — `--host` accepts hostnames (e.g. `localhost`) as
> well as IP literals. The server resolves the name via the OS DNS resolver
> and binds to the resulting IP. If DNS fails it falls back to `0.0.0.0:<port>`
> and logs a warning.

---

## Running integration tests

The `solid-test` binary connects to a **running** Solid server and exercises its HTTP API with a TAP-formatted report. It works against both this Rust server and the original TypeScript CSS.

```bash
# 1. Start the server (in one terminal)
cargo run --bin solid-server -- --port 3001

# 2. Run all integration tests (in another terminal)
cargo run --bin solid-test -- --base-url http://localhost:3001/

# Run only a specific suite (substring match, case-insensitive)
cargo run --bin solid-test -- --filter resource-crud

# Verbose: print every request and response line
cargo run --bin solid-test -- --verbose

# Against an external / TypeScript CSS instance
cargo run --bin solid-test -- --base-url https://my-css-instance.example/
```

### Example output

```
TAP version 14
# health
ok 1 - GET / returns 200 or 401
ok 2 - OPTIONS / returns 204 or 200 with Allow header
ok 3 - HEAD / does not return a body
# resource-crud
ok 4 - PUT creates a new document and returns 201
ok 5 - GET returns the stored body after PUT
ok 6 - GET on absent resource returns 404
ok 7 - PUT on existing resource overwrites and returns 200 or 204
ok 8 - DELETE returns 204 and removes the resource
ok 9 - DELETE on absent resource returns 404
ok 10 - GET returns matching Content-Type
# containers
ok 11 - GET / responds with 200 or 401 (root is a container)
ok 12 - PUT to container URL creates container (201)
ok 13 - POST to container creates child resource (201)
ok 14 - GET on container includes ldp:Container Link header
ok 15 - DELETE on empty container returns 204
# content-negotiation
ok 16 - GET with Accept: text/turtle returns Turtle
ok 17 - GET with Accept: application/ld+json returns JSON-LD or 406
ok 18 - GET plain-text resource echoes text/plain
ok 19 - GET with unsupported Accept type returns 406 or 200
# error-responses
ok 20 - GET unknown path returns 404
ok 21 - PATCH without supported patch Content-Type returns 415 or 405
ok 22 - 404 response body describes the error
ok 23 - PUT to path with missing parent returns 201 (auto-create) or 404/409
# wac
ok 24 - GET on unprotected resource returns 200
ok 25 - GET on ACL-protected resource without credentials returns 401
ok 26 - WAC-Allow header is present on protected resource
1..26
# passed: 26  failed: 0
```

The runner exits with code `0` on full pass and `1` if any test fails.

---

## Unit tests

Each crate carries its own `#[cfg(test)]` modules. Run them with:

```bash
# All crates at once
cargo test

# Single crate (use the Cargo package name)
cargo test -p http-types
cargo test -p solid-storage
cargo test -p authz
cargo test -p identity
cargo test -p static-assets
cargo test -p server-core

# Single test by name
cargo test -p http-types not_found_is_client_error

# With captured output (useful for debugging)
cargo test -- --nocapture
```

### Test coverage by crate

| Crate | Tests / ported from TypeScript |
|-------|--------------------------------|
| `http-types` | `SolidError` hierarchy, `ResourceIdentifier` helpers · ported from `HttpError.test.ts`, `ResourceIdentifier.test.ts` |
| `solid-storage` | `MemoryMapStorage`, `JsonFileStorage`, `WrappedExpiringStorage`, `PassthroughKeyValueStorage`, `BaseResourceStore`, `PassthroughStore`, `ReadOnlyStore` · ported from `MemoryMapStorage.test.ts`, `BaseResourceStore.test.ts`, `PassthroughStore.test.ts`, `ReadOnlyStore.test.ts` |
| `authz` | `Authorizer` + `PermissionReader` trait contracts |
| `identity` | `AccountStore`, `PodManager`, `WebIdStore`, `ClientCredentialsStore` trait contracts |
| `static-assets` | `StaticAssetHandler` path-traversal guard, MIME detection |
| `server-core` | Router construction, middleware layer ordering |
| `integration-tests` | `SolidClient` assertion helpers |

---

## CLI reference

### `solid-server`

Starts the HTTP server.

```
Usage: solid-server [OPTIONS]

Options:
  -b, --base-url <URL>     Base URL advertised to clients
                           [env: CSS_BASE_URL]  [default: http://localhost:3000/]
  -p, --port <PORT>        TCP port to listen on
                           [env: CSS_PORT]      [default: 3000]
      --host <HOST>        Hostname or IP to bind to (DNS-resolved)
                           [env: CSS_HOST]      [default: localhost]
  -l, --log-level <LEVEL>  Log level: trace | debug | info | warn | error
                           [env: CSS_LOG_LEVEL] [default: info]
      --root-dir <PATH>    Root directory for file-backed storage
                           [env: CSS_ROOT_DIR]  (omit for in-memory storage)
  -h, --help               Print help
  -V, --version            Print version
```

### `solid-test`

Runs the HTTP integration test suite against any live Solid server.

```
Usage: solid-test [OPTIONS]

Options:
  -b, --base-url <URL>    Base URL of the server under test
                          [env: CSS_BASE_URL]  [default: http://localhost:3000/]
      --filter <SUBSTR>   Only run suites whose name contains SUBSTR
                          (case-insensitive)
  -v, --verbose           Print each request/response line for all tests
      --timeout-ms <MS>   Per-request timeout in milliseconds [default: 10000]
  -h, --help              Print help
  -V, --version           Print version
```

---

## Environment variables

All CLI flags can be set via environment variables. Explicit flags override environment variables; both override built-in defaults.

| Variable | Flag equivalent | Default |
|----------|-----------------|---------|
| `CSS_BASE_URL` | `--base-url` | `http://localhost:3000/` |
| `CSS_PORT` | `--port` | `3000` |
| `CSS_HOST` | `--host` | `localhost` |
| `CSS_LOG_LEVEL` | `--log-level` | `info` |
| `CSS_ROOT_DIR` | `--root-dir` | *(in-memory)* |
| `RUST_LOG` | *(overrides `--log-level` entirely)* | — |

`RUST_LOG` follows the standard `tracing-subscriber` filter syntax, e.g.:

```bash
RUST_LOG=solid_storage=debug,solid_server=info cargo run --bin solid-server
```

---

## Architecture

The server is structured as focused crates that mirror the TypeScript CSS package layout:

```
HTTP Request
     │
     ▼
  cli  (solid-server binary)
  ├─ serve.rs       parse CLI args, resolve hostname → SocketAddr, build AppConfig
     │
     ▼
  server-core
  ├─ app.rs         App lifecycle: bind TCP, start Axum, graceful shutdown
  ├─ middleware.rs  CORS + credential extraction + authz tower layers
  ├─ routing.rs     ldp_router()  /*path + / for all LDP methods
  ├─ pipeline.rs    RequestPipeline — authz wiring + dispatch to per-operation handlers
  └─ handler.rs     bridge: Axum extractors → OperationHandler trait
         │
         ├──► http-types
         │     ├─ error.rs          SolidError (BadRequest … InternalServerError)
         │     ├─ handler.rs        OperationHandler async trait
         │     ├─ identifier.rs     ResourceIdentifier + path utilities
         │     ├─ metadata.rs       RepresentationMetadata (Content-Type, ETag, …)
         │     ├─ operation.rs      Operation enum (GET, PUT, POST, PATCH, DELETE, …)
         │     └─ representation.rs Representation (streaming body + metadata)
         │
         ├──► solid-storage
         │     ├─ resource_store.rs  ResourceStore trait (implemented by LdpStore)
         │     ├─ key_value.rs       KeyValueStorage, ExpiringStorage, Passthrough
         │     ├─ error.rs           StorageError (NotFound, AlreadyExpired, …)
         │     └─ backends/
         │          ├─ memory.rs     MemoryMapStorage<V>
         │          ├─ file.rs       JsonFileStorage<V>
         │          └─ expiring.rs   WrappedExpiringStorage (TTL + background sweep)
         │
         ├──► authz
         │     ├─ credentials.rs    Credentials (agent WebID + issuer)
         │     ├─ permissions.rs    PermissionReader + AclPermission flags
         │     └─ authorizer.rs     Authorizer async trait
         │                          (wired into pipeline via authz_middleware)
         │
         ├──► identity
         │     ├─ account.rs              AccountStore (CRUD + password verify)
         │     ├─ pod.rs                  PodManager + PodStore
         │     ├─ webid.rs               WebIdStore + WebIdLinkStore
         │     └─ client_credentials.rs  ClientCredentialsStore
         │
         └──► static-assets
               ├─ entry.rs    StaticAssetEntry (URL prefix → fs path)
               └─ handler.rs  StaticAssetHandler (MIME detection, path traversal guard)

  integration-tests  (used by solid-test binary)
  ├─ client.rs       SolidClient: reqwest wrapper + TAP assertion helpers
  ├─ runner.rs       TestRunner: suite registry, TAP printer, exit code
  └─ suites/
       ├─ health.rs              root resource liveness
       ├─ resource_crud.rs       PUT / GET / DELETE documents
       ├─ containers.rs          LDP container behaviour
       ├─ content_negotiation.rs Accept / Content-Type negotiation
       ├─ error_responses.rs     4xx shape and headers
       └─ wac.rs                 WAC access control (401, WAC-Allow)
```

### Key design principles

- **Trait-based, not class-based.** Every storage backend, authoriser, and handler is an `async_trait` — swap implementations without touching call sites.
- **`ChangeMap` for reactive updates.** Every mutating `ResourceStore` method returns a `HashMap<Url, ChangeMetadata>` so monitoring layers (notifications, webhooks) can react to fine-grained changes.
- **Mirror the TypeScript.** Module paths, type names, and test names intentionally match their CSS counterparts to make cross-referencing straightforward.
- **Layered authz.** Every request passes through `extract_credentials()` → `PermissionReader::read()` → `Authorizer::authorize()` before reaching an LDP handler.

---

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md) for the full contributor guidelines,
licence assignment terms, and code of conduct.

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Test
cargo test --all

# All-in-one pre-push check
cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --all
```

Please open an issue before submitting large pull requests.
