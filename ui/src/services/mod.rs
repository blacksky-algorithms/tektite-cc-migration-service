//! Infrastructure Services
//!
//! This module provides the core infrastructure services for the migration application:
//!
//! - **client**: ATProto client with PDS operations, authentication, and identity resolution
//! - **streaming**: WASM-optimized streaming architecture with channel-tee patterns
//! - **blob**: Legacy blob management (being migrated to streaming architecture)
//! - **config**: Configuration management and global settings
//! - **errors**: Common error types and handling utilities
//!
//! The services are designed to be WASM-first, using browser APIs and async traits
//! without Send/Sync bounds for compatibility.

pub mod blob;
pub mod client;
pub mod config;
pub mod errors;
pub mod streaming;
