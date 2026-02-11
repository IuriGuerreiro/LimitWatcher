# Project Context: LimitsWatcher

LimitsWatcher is a cross-platform desktop application designed to monitor and visualize usage quotas and limits for various AI service providers. It is based on the **CodexBar** project (located in the `/CodexBar/` directory at the repository root), leveraging its provider implementation logic and architecture for tracking AI usage. It helps users manage their AI subscriptions and stay within rate limits through real-time tracking and automated alerts.

## Core Purpose
The primary goal of LimitsWatcher is to provide a unified dashboard for AI usage across different platforms, preventing unexpected service interruptions or overage charges by tracking:
- **Session Limits:** Requests Per Minute (RPM) and Tokens Per Minute (TPM).
- **Periodic Limits:** Daily, weekly, or monthly usage quotas.
- **Credit Balances:** Remaining monetary or token credits for paid tiers.

## Architecture
The application is built using **Tauri v2**, combining a performant Rust backend with a modern React + TypeScript frontend.

### Tech Stack
- **Frontend:** React 18, TypeScript, Vite, Tauri JS API.
- **Backend:** Rust, Tauri v2.
- **Async Runtime:** Tokio.
- **Networking:** Reqwest.
- **Security:** 
    - `keyring-rs` for OS-native secret storage (Windows Credential Manager, macOS Keychain, Linux Secret Service).
    - `aes-gcm` for application-level encryption of cached sensitive data.
- **Data Persistence:** Tauri Plugin Store and custom file-based caching.

## Key Components

### 1. Provider System (`src-tauri/src/providers/`)
A trait-based system allows for easy integration of new AI services. Each provider implements:
- **Authentication:** Supports API Keys, OAuth2, Device Flow, Browser Cookies, and CLI-based auth.
- **Quota Fetching:** Specialized logic for scraping or API calls to retrieve usage data.
- **Supported Providers:**
    - **Gemini:** CLI-based OAuth integration.
    - **Claude:** (Implementation in progress).
    - **Copilot:** (Implementation in progress).
    - **Antigravity:** (Implementation in progress).

### 2. Secure Storage (`src-tauri/src/storage/`)
- **Keyring:** Manages sensitive tokens and API keys using system-level security.
- **Cache Manager:** Stores non-sensitive usage data and encrypted session info to enable quick app launches and background tracking.

### 3. Background Services
- **Scheduler (`src-tauri/src/scheduler/`):** A background task that periodically polls active providers for usage updates.
- **Notifications (`src-tauri/src/notifications/`):** Triggers system alerts when usage crosses configurable thresholds (e.g., 80% or 90% of limit).
- **System Tray (`src-tauri/src/tray/`):** Provides a persistent presence in the system menu bar/tray, showing quick status updates without opening the main window.

## Development Status
The project is currently in the early implementation phase, following a structured roadmap:
- **Phase 0:** Setup and scaffolding (Complete).
- **Phase 1:** Core infrastructure - Storage, Tray, Scheduler (In Progress).
- **Phase 2:** Provider Implementations (Gemini started).
- **Phase 3:** UI Refinement.

## Project Structure
```text
LimitsWatcher/
├── src/                # React Frontend
│   ├── components/     # UI Components (Settings, Auth Panels, Usage Cards)
│   └── App.tsx         # Main Dashboard logic
├── src-tauri/          # Rust Backend
│   ├── src/
│   │   ├── commands/   # IPC Command Handlers
│   │   ├── providers/  # AI Provider Logic & Traits
│   │   ├── storage/    # Encrypted & Secure Storage
│   │   ├── scheduler.rs # Background Polling
│   │   ├── notifications.rs # Alerting Logic
│   │   ├── tray.rs      # System Tray UI
│   │   └── lib.rs       # App Initialization
│   └── Cargo.toml      # Rust Dependencies
└── docs/               # Development Documentation & Phase Plans
```