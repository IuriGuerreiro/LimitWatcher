# Phase 0: Project Setup

## Overview
Initialize the Tauri v2 project with all necessary dependencies and configuration for Windows, Linux, and macOS.

---

## Prerequisites

### Install Required Tools

**All Platforms:**
```bash
# Node.js 18+ (via nvm recommended)
nvm install 20
nvm use 20

# Rust (via rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Or on Windows: download from https://rustup.rs

# Tauri CLI
cargo install tauri-cli
```

**Windows Additional:**
```powershell
# Install Visual Studio Build Tools (C++ workload)
# Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/

# WebView2 (usually pre-installed on Windows 10/11)
# If missing: https://developer.microsoft.com/en-us/microsoft-edge/webview2/
```

**Linux Additional:**
```bash
# Debian/Ubuntu
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libsecret-1-dev  # For keyring support

# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel \
  libsecret-devel

# Arch
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
  libappindicator-gtk3 librsvg libsecret
```

**macOS Additional:**
```bash
# Xcode Command Line Tools
xcode-select --install
```

---

## Step 1: Create Tauri Project

```bash
cd F:\Projects\Programmes\WebApps\LimitsWatcher

# Create new Tauri v2 project with React + TypeScript
npm create tauri-app@latest . -- --template react-ts

# Or with Svelte (alternative)
# npm create tauri-app@latest . -- --template svelte-ts
```

**When prompted:**
- Project name: `limits-watcher`
- Package manager: `npm` (or pnpm/yarn)
- UI template: `React` + `TypeScript`

---

## Step 2: Install Tauri Plugins

```bash
cd LimitsWatcher

# Core plugins
npm install @tauri-apps/plugin-notification
npm install @tauri-apps/plugin-store
npm install @tauri-apps/plugin-shell
npm install @tauri-apps/plugin-process
npm install @tauri-apps/plugin-os
npm install @tauri-apps/plugin-http

# Add Rust dependencies
cd src-tauri
cargo add tauri-plugin-notification
cargo add tauri-plugin-store
cargo add tauri-plugin-shell
cargo add tauri-plugin-process
cargo add tauri-plugin-os
cargo add tauri-plugin-http

# Security/Crypto dependencies
cargo add keyring          # OS Keychain access
cargo add aes-gcm          # AES-256-GCM encryption
cargo add argon2           # Key derivation
cargo add rand             # Secure random generation
cargo add base64           # Encoding
cargo add serde --features derive
cargo add serde_json
cargo add tokio --features full
cargo add reqwest --features json,cookies
cargo add chrono --features serde

cd ..
```

---

## Step 3: Configure Tauri

Edit `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "LimitsWatcher",
  "identifier": "com.limitswatcher.app",
  "version": "0.1.0",
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "LimitsWatcher",
        "width": 900,
        "height": 600,
        "resizable": true,
        "fullscreen": false,
        "visible": false,
        "center": true
      }
    ],
    "trayIcon": {
      "iconPath": "icons/tray.png",
      "iconAsTemplate": true,
      "menuOnLeftClick": false
    },
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "webviewInstallMode": {
        "type": "downloadBootstrapper"
      }
    }
  },
  "plugins": {
    "notification": {
      "all": true
    },
    "store": {},
    "shell": {
      "open": true
    },
    "http": {
      "enabled": true,
      "scope": [
        "https://api.github.com/**",
        "https://api.anthropic.com/**",
        "https://generativelanguage.googleapis.com/**",
        "https://*.openai.com/**",
        "https://*.claude.ai/**"
      ]
    }
  }
}
```

---

## Step 4: Initialize Rust Backend Structure

Create the following directory structure in `src-tauri/src/`:

```
src-tauri/src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── commands/            # Tauri commands (IPC)
│   └── mod.rs
├── providers/           # AI provider implementations
│   ├── mod.rs
│   ├── traits.rs        # Provider trait definition
│   ├── copilot.rs
│   ├── claude.rs
│   ├── gemini.rs
│   └── antigravity.rs
├── storage/             # Secure storage
│   ├── mod.rs
│   ├── keyring.rs       # OS Keychain
│   ├── encrypted.rs     # AES-256-GCM
│   └── cache.rs         # Usage cache
├── scheduler.rs         # Background refresh
├── notifications.rs     # Alert system
└── tray.rs              # System tray
```

**Create directories:**
```bash
cd src-tauri/src
mkdir commands providers storage
```

---

## Step 5: Basic main.rs Setup

Replace `src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod providers;
mod storage;
mod scheduler;
mod notifications;
mod tray;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            // Initialize system tray
            tray::init(app)?;
            
            // Start background scheduler
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                scheduler::start(handle).await;
            });
            
            // Hide dock icon on macOS (menu bar app style)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_provider_status,
            commands::refresh_provider,
            commands::save_credentials,
            commands::get_all_usage,
            commands::set_provider_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Step 6: Create Icon Assets

Create `src-tauri/icons/` with the required icon files:

```bash
mkdir -p src-tauri/icons
```

Required icons:
- `32x32.png` - Small icon
- `128x128.png` - Standard icon
- `128x128@2x.png` - Retina icon
- `icon.icns` - macOS app icon
- `icon.ico` - Windows app icon
- `tray.png` - System tray icon (recommend 22x22 or 32x32)

**Placeholder tray icon (create later with proper design):**
For now, create a simple colored square as placeholder.

---

## Step 7: Frontend Base Structure

Update `src/App.tsx`:

```tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface ProviderUsage {
  provider: string;
  enabled: boolean;
  sessionUsed: number;
  sessionLimit: number;
  weeklyUsed: number;
  weeklyLimit: number;
  resetTime: string | null;
  error: string | null;
}

function App() {
  const [providers, setProviders] = useState<ProviderUsage[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadProviders();
  }, []);

  async function loadProviders() {
    try {
      const usage = await invoke<ProviderUsage[]>("get_all_usage");
      setProviders(usage);
    } catch (e) {
      console.error("Failed to load providers:", e);
    } finally {
      setLoading(false);
    }
  }

  async function refreshAll() {
    setLoading(true);
    for (const p of providers) {
      if (p.enabled) {
        await invoke("refresh_provider", { provider: p.provider });
      }
    }
    await loadProviders();
  }

  return (
    <main className="container">
      <h1>LimitsWatcher</h1>
      <p>AI Subscription Usage Tracker</p>
      
      <button onClick={refreshAll} disabled={loading}>
        {loading ? "Refreshing..." : "Refresh All"}
      </button>

      <div className="providers-grid">
        {providers.map((p) => (
          <ProviderCard key={p.provider} usage={p} onRefresh={loadProviders} />
        ))}
      </div>
    </main>
  );
}

function ProviderCard({ usage, onRefresh }: { usage: ProviderUsage; onRefresh: () => void }) {
  const sessionPercent = usage.sessionLimit > 0 
    ? (usage.sessionUsed / usage.sessionLimit) * 100 
    : 0;
  const weeklyPercent = usage.weeklyLimit > 0 
    ? (usage.weeklyUsed / usage.weeklyLimit) * 100 
    : 0;

  return (
    <div className={`provider-card ${usage.enabled ? '' : 'disabled'}`}>
      <h3>{usage.provider}</h3>
      
      {usage.error ? (
        <p className="error">{usage.error}</p>
      ) : (
        <>
          <div className="usage-bar">
            <label>Session: {usage.sessionUsed}/{usage.sessionLimit}</label>
            <div className="bar">
              <div className="fill" style={{ width: `${sessionPercent}%` }} />
            </div>
          </div>
          
          <div className="usage-bar">
            <label>Weekly: {usage.weeklyUsed}/{usage.weeklyLimit}</label>
            <div className="bar">
              <div className="fill" style={{ width: `${weeklyPercent}%` }} />
            </div>
          </div>
          
          {usage.resetTime && (
            <p className="reset">Resets: {usage.resetTime}</p>
          )}
        </>
      )}
    </div>
  );
}

export default App;
```

---

## Step 8: Verify Setup

```bash
# Development mode
npm run tauri dev

# Build for production
npm run tauri build
```

---

## Checklist

- [ ] Prerequisites installed (Node.js, Rust, platform tools)
- [ ] Tauri project created
- [ ] All plugins installed (npm + cargo)
- [ ] `tauri.conf.json` configured
- [ ] Directory structure created
- [ ] Basic `main.rs` with plugin initialization
- [ ] Icon placeholders created
- [ ] Frontend base structure ready
- [ ] `npm run tauri dev` works

---

## Next Steps
- **Phase 1:** Core infrastructure (storage, tray, scheduler) → See `PHASE-1-CORE.md`
- **Providers:** Can start implementing providers in parallel → See `providers/` docs
