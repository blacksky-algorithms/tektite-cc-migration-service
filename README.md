# ATProto PDS Migration Service

A client-side migration tool for the AT Protocol ecosystem, allowing users to transfer their accounts between different Personal Data Servers (PDS) while preserving all data and identity information.

![Migration Service UI](https://via.placeholder.com/800x400?text=ATProto+Migration+Service)

## Overview

This migration service provides a fully client-side solution for transferring AT Protocol accounts (like Bluesky accounts) between different providers. It handles all aspects of the migration process:

- Repository migration (posts, follows, etc.)
- Blob migration (images and other attachments)
- User preferences migration
- PLC identity operations
- Account activation/deactivation

## Features

- **Fully Client-Side**: No server component needed - all operations happen directly in the browser
- **Resumable Migrations**: Can resume interrupted migrations from various checkpoints
- **Cross-PDS Support**: Migrate between any compliant AT Protocol servers
- **DNS-over-HTTPS**: Uses secure DNS-over-HTTPS for handle resolution
- **Progress Tracking**: Detailed progress monitoring for long-running migrations
- **Secure Credential Handling**: Proper handling of authentication tokens
- **Custom Domain Support**: Handles FQDN migration with instructions for DNS updates

## Architecture

The application is built with a modern Rust + WebAssembly stack:

- **UI Layer**: Dioxus (Rust-based React-like framework)
- **State Management**: Signal-based state with action dispatching
- **Storage**: Multiple storage backends (LocalStorage, OPFS) with fallback strategy
- **Networking**: Direct PDS API calls with client-side HTTP

## Getting Started

### Prerequisites

- Rust toolchain (1.70+ recommended)
- Dioxus CLI

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/username/atproto-migration-service.git
   cd atproto-migration-service
   ```

2. Install the Dioxus CLI:
   ```bash
   cargo install dioxus-cli
   ```

3. Build and run:
   ```bash
   dx serve
   ```

4. Access the application at `http://localhost:8080`

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
.
├── Cargo.toml          # Workspace configuration
├── ui/                 # Shared UI components
│   ├── src/
│   │   ├── app/        # Main application components
│   │   ├── components/ # Reusable UI components
│   │   ├── features/   # Feature modules (migration, etc.)
│   │   ├── services/   # Client-side services (DNS, PDS, etc.)
│   │   └── utils/      # Utility functions
├── web/                # Web application entry point
```

### Building for Production

```bash
dx build --release
```

Production files will be output to the `dist` directory.

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
  - Replace core libraries without strong need (e.g., MobX → Redux)
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