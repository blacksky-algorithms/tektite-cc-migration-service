# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Tektite CC Migration Service is a fully client-side AT Protocol account migration tool built with Rust + WebAssembly. It runs entirely in the browser with zero server dependencies, enabling users to transfer accounts between Personal Data Servers (PDS) — primarily for migrating to BlackSky (`blacksky.app`).

Live at: **tektite.cc**

## Build & Development Commands

```bash
# Install Dioxus CLI (required)
cargo install dioxus-cli

# Development server with hot reload (serves at http://localhost:8080)
dx serve

# Production build
dx build --release --features web --package web

# Production bundle (used in Docker/CI)
dx bundle --platform web --package web

# Check compilation (WASM target)
cargo check --target wasm32-unknown-unknown

# Run clippy
cargo clippy --target wasm32-unknown-unknown

# Run tests (must target WASM)
cargo test --target wasm32-unknown-unknown

# Format code
cargo fmt
```

The WASM target `wasm32-unknown-unknown` must be installed: `rustup target add wasm32-unknown-unknown`

## Architecture

### Workspace Structure

This is a Cargo workspace with two crates:

- **`ui/`** — Core library crate containing all migration logic, UI components, services, and state management
- **`web/`** — Thin binary crate that launches the Dioxus app, defines routing, and imports from `ui`

### Framework

- **Dioxus 0.6** — Rust reactive UI framework compiled to WASM
- **Signal-based state** — `MigrationState` with `MigrationAction` enum (reducer pattern via `reduce`/`reduce_in_place`)
- **Client-side routing** via `dioxus::prelude::Router`
- Static assets live in `web/assets/` and are referenced via `asset!()` macro
- Configuration in `web/Dioxus.toml`

### Key Module Organization (`ui/src/`)

- **`app/migration_service.rs`** — Root component, the main migration wizard UI
- **`components/`** — Reusable UI: forms (login, PDS selection, migration details, PLC verification), display (progress, loading), layout (navbar)
- **`migration/`** — Migration orchestration and business logic
  - `orchestrator.rs` — Entry point: `execute_migration_client_side()` drives the full flow
  - `steps/` — Individual migration phases: `repository`, `blob`, `preferences`, `plc`
  - `types.rs` — `MigrationState`, `MigrationAction`, all form/progress structs, reducer logic
  - `progress/` — Event tracking, metrics, progress reporter
- **`services/`** — Infrastructure layer
  - `client/` — AT Protocol PDS communication: auth, session management, API calls, DNS-over-HTTPS handle resolution, identity resolution
  - `streaming/` — WASM-optimized streaming with channel-tee patterns, metrics, orchestration
  - `blob/` — Blob chunking and OPFS (Origin Private File System) storage
  - `config/` — Platform-specific configuration and storage estimation
- **`utils/`** — Console logging macros, handle suggestions, validation, serialization helpers

### Migration Flow

The migration follows four sequential steps (as `FormStep` enum):
1. **Login** → Authenticate with current PDS
2. **SelectPds** → Choose destination PDS (defaults to BlackSky)
3. **MigrationDetails** → Configure new account (handle, password, email)
4. **PlcVerification** → Complete identity transfer via email verification code

Data migration order: **Repository (CAR export/import) → Blobs → Preferences → PLC identity update**

### WASM Constraints

- All code runs in the browser — no `Send`/`Sync` bounds on async traits
- `tokio` is used with limited features (`macros`, `sync`, `rt`) for WASM compatibility
- HTTP via `reqwest` with WASM-compatible feature flags (no default features)
- Browser storage: OPFS (primary), IndexedDB via `rexie`, LocalStorage via `gloo-storage` (fallback)
- `u64` values are serialized as strings to avoid BigInt issues in WASM
- Console logging uses custom macros (`console_log!`, `console_info!`, `console_debug!`) from `utils/console_macros.rs`

### Clippy Configuration

`clippy.toml` configures `await-holding-invalid-types` to prevent holding Dioxus signal refs (`GenerationalRef`, `GenerationalRefMut`, `dioxus_signals::Write`) across await points. This is critical — holding these across awaits causes runtime panics.

## CI/CD

- GitHub Actions deploys to GitHub Pages on push to `main` (`.github/workflows/deploy.yml`)
- Custom domain: `tektite.cc` (CNAME file added during build)
- Docker build uses `cargo-chef` for layer caching, serves via nginx on port 8080

## AT Protocol Reference

- The migration process follows the manual migration flow documented in `NEWBOLD.md`
- Handle resolution uses Cloudflare DNS-over-HTTPS and HTTP `.well-known` fallback
- PDS operations use standard ATProto XRPC endpoints (`com.atproto.server.*`, `com.atproto.sync.*`, `com.atproto.identity.*`)
