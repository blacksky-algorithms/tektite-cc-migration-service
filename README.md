# Tektite CC Migration Service

A high-performance, fully client-side migration tool built with Rust and WebAssembly for the AT Protocol ecosystem. Transfer your accounts between Personal Data Servers (PDS) with enterprise-grade reliability, advanced blob management, and fault-tolerant architecture.

![Migration Service UI](https://tektite.cc)

## Overview

Tektite CC provides a sophisticated client-side solution for AT Protocol account migrations (Bluesky, other providers) with zero server dependencies. Built entirely in Rust and compiled to WebAssembly, it delivers native performance in the browser while implementing advanced features like:

- **Smart Repository Migration** with resume capabilities
- **Advanced Blob Management** (chunking, deduplication, verification)
- **Multi-Storage Backends** (OPFS + LocalStorage fallback)
- **Fault-Tolerant Architecture** with circuit breakers
- **PLC Identity Operations** with secure token handling
- **Progressive Migration** with detailed progress tracking

## Features

### Core Migration Capabilities
- **Zero Server Dependencies**: Pure client-side architecture with no backend requirements
- **Resumable Migrations**: Checkpoint-based resumption at repository, blob, preferences, and PLC levels  
- **Cross-PDS Support**: Migrate between any compliant AT Protocol servers (Bluesky, custom instances)
- **Secure DNS-over-HTTPS**: Cloudflare DoH integration for reliable handle resolution

### Advanced Streaming Architecture
- **Performance Metrics**: Real-time monitoring of transfer rates, chunk efficiency, and memory usage
- **Enhanced Error Handling**: Comprehensive error types with automatic recovery strategies
- **Platform Optimization**: Adaptive configuration for browser, mobile, and desktop environments
- **Memory Management**: Intelligent pressure monitoring with automatic cleanup and optimization
- **Streaming Infrastructure**: Pure streaming approach with channel-tee pattern for efficient data flow

### Fault Tolerance & Reliability  
- **Modular Architecture**: Clean separation of concerns with focused, specialized modules
- **Streaming-First Design**: Pure streaming approach eliminates legacy complexity and improves reliability
- **Progress Monitoring**: Real-time metrics and detailed progress reporting with performance analytics
- **Session Management**: Secure JWT handling with automatic refresh capabilities
- **Network Resilience**: Automatic retry logic with intelligent backoff and recovery strategies

### User Experience
- **Progressive UI**: Step-by-step wizard with visual progress indicators
- **Custom Domain Support**: FQDN migration with DNS update instructions
- **Video Tutorial Integration**: Built-in guidance for complex migration scenarios
- **Mobile Responsive**: Optimized for both desktop and mobile usage

## Architecture

Tektite CC is built with a sophisticated Rust + WebAssembly architecture optimized for performance and reliability:

### Frontend Stack
- **UI Framework**: Dioxus 0.6 (Rust-based reactive framework)
- **State Management**: Signal-based architecture with action dispatching
- **Compilation Target**: WebAssembly (WASM) for near-native performance
- **Routing**: Client-side routing with Dioxus Router

### Service Layer Architecture  
```
services/
├── blob/                    # Streaming-optimized blob migration
│   ├── blob_chunking.rs     # Intelligent chunk processing
│   └── blob_opfs_storage.rs # Origin Private File System integration
├── streaming/               # High-performance streaming infrastructure
│   ├── metrics.rs           # Performance monitoring and analytics
│   ├── errors.rs            # Enhanced error handling with recovery
│   ├── traits.rs            # Core streaming abstractions
│   ├── orchestrator.rs      # Stream coordination and management
│   └── implementations.rs   # Browser-optimized implementations
├── client/                  # PDS communication layer
│   ├── api/                 # API endpoint implementations
│   ├── auth/                # Authentication and session management
│   ├── pds_client.rs        # Direct AT Protocol API calls
│   ├── dns_over_https.rs    # Secure handle resolution via Cloudflare
│   └── identity_resolver.rs # Handle-to-DID resolution
└── config/                  # Unified configuration system
    ├── unified_config.rs     # Platform-specific optimizations
    └── storage_estimator.rs  # Storage quota management
```

### Migration Orchestration
- **Step-by-Step Processing**: Repository → Blobs → Preferences → PLC
- **Comprehensive Resume System**: Full checkpoint-based resumption at all migration levels
- **Progress Tracking**: Real-time metrics and event reporting with throughput analysis
- **Error Recovery**: Automatic retry with exponential backoff and circuit breaker patterns

### Storage Architecture
- **Primary**: OPFS (Origin Private File System) for optimal performance and unlimited capacity
- **Secondary**: IndexedDB for broad browser compatibility with good performance
- **Fallback**: LocalStorage for maximum compatibility (5-10MB limit)
- **Intelligent Selection**: Automatic backend selection based on browser capabilities and storage requirements

## Getting Started

### Prerequisites

- **Rust toolchain** (1.88+ recommended)
- **Dioxus CLI** for development and building
- **Modern browser** with WebAssembly and OPFS support (Chrome recommended)

### Development Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/blacksky-algorithms/tektite-cc-migration-service.git
   cd tektite-cc-migration-service
   ```

2. **Install Dioxus CLI**:
   ```bash
   cargo install dioxus-cli
   ```

3. **Install dependencies**:
   ```bash
   cargo check
   ```

4. **Run development server**:
   ```bash
   dx serve
   ```

5. **Access the application**: Open `http://localhost:8080` in your browser

### Quick Development Commands

```bash
# Hot reload development
dx serve

# Build for production  
dx build --release --features web --package web

# Run tests
cargo test --target wasm32-unknown-unknown

# Check code formatting
cargo clippy --target wasm32-unknown-unknown
```

## Usage

The migration process consists of four main steps:

1. **Login to Current PDS**: Authenticate with your current server
2. **Select New PDS**: Choose the destination server (defaults to BlackSky)
3. **Migration Details**: Configure your new account settings
4. **PLC Verification**: Complete the identity transfer with email verification

### Step-by-Step Guide

1. Enter your existing handle or DID and password
2. Select "Migrate to BlackSky" or enter a custom PDS URL
3. Configure your new handle, password, and email
4. Start the migration and wait for data transfer
5. Check your email for the PLC verification code
6. Enter the code to complete the migration

## Development

### Project Structure

```
tektite-cc-migration-service/
├── Cargo.toml                  # Workspace configuration
├── ui/                         # Core UI library
│   ├── Cargo.toml             # UI crate dependencies
│   ├── src/
│   │   ├── app/               # Main application components
│   │   │   └── migration_service.rs
│   │   ├── components/        # Reusable UI components
│   │   │   ├── display/       # Progress, loading indicators
│   │   │   ├── forms/         # Login, migration detail forms
│   │   │   └── layout/        # Navigation, layout components
│   │   ├── migration/         # Migration orchestration (refactored)
│   │   │   ├── account_operations.rs  # Account creation and status
│   │   │   ├── logic.rs       # Main migration orchestration
│   │   │   ├── resume_handlers.rs     # Checkpoint resumption logic
│   │   │   ├── session_management.rs  # Session conversion utilities
│   │   │   ├── validation.rs  # Migration validation and verification
│   │   │   ├── progress/      # Event tracking, metrics
│   │   │   ├── steps/         # Repo, blob, PLC, preferences
│   │   │   └── types.rs       # State management types
│   │   ├── services/          # Client-side service layer
│   │   │   ├── blob/          # Streaming-optimized blob migration
│   │   │   │   ├── blob_chunking.rs
│   │   │   │   └── blob_opfs_storage.rs
│   │   │   ├── streaming/     # High-performance streaming infrastructure
│   │   │   │   ├── metrics.rs      # Performance monitoring
│   │   │   │   ├── errors.rs       # Enhanced error handling
│   │   │   │   ├── traits.rs       # Core abstractions
│   │   │   │   └── orchestrator.rs # Stream coordination
│   │   │   ├── client/        # PDS communication
│   │   │   │   ├── api/       # API endpoint implementations
│   │   │   │   ├── auth/      # Authentication management
│   │   │   │   └── pds_client.rs
│   │   │   ├── config/        # Unified configuration system
│   │   │   │   ├── unified_config.rs   # Platform optimizations
│   │   │   │   └── storage_estimator.rs
│   │   │   └── errors/        # Error handling
│   │   └── utils/             # Validation, serialization
├── web/                       # Web application entry point
│   ├── Cargo.toml            # Web app dependencies
│   └── src/main.rs           # WASM application entry
└── assets/                   # Static assets (CSS, images)
```

### Building for Production

```bash
dx build --release --features web --package web
```



## Contributions

> We ❤️ thoughtful contributions! Help us keep the diff small and the community safe.

**Rules**

- We may decline or delay PRs that are too large to maintain.
- We reserve the right to lock heated threads to protect contributors’ time.

**Guidelines**

1. **Open an issue first** – give the community time to discuss scope & maintenance.
2. **Prefer small patches** – anything that touches lots of upstream code is hard to carry.
3. **Put opinionated changes behind toggles**.
4. Avoid PRs that…
  - Rename common terms (e.g., “Post” → “Skeet”)
  - Add entirely new features with no prior discussion

If your idea isn’t a fit, feel free to **fork** – that’s the beauty of open source!

---

## Forking Guidelines

- Re-brand clearly so users don’t confuse your fork with blacksky.community.
- Point analytics / error reporting to **your** endpoints.
- Update support links (feedback, email, terms, etc.) to your own.

---

## Security Disclosures

Found a vulnerability?  
Email **rudy@blacksky.app** – we will respond
promptly.

---

## License

**MIT** – see [./LICENSE](./LICENSE).

---

## P.S.

Blacksky exists because of contributors like *you*.  
Thank you for helping us build safer, community-owned social media!