# Phase 4: Platform Polish & Distribution

## Overview
Final polish for each platform, auto-start functionality, auto-updates, and distribution setup.

---

## 1. Platform-Specific Polish

### Windows

#### Windows Credential Manager
Ensure keyring works correctly:

```rust
// Test on Windows
#[cfg(target_os = "windows")]
#[test]
fn test_windows_credential_manager() {
    use keyring::Entry;
    
    let entry = Entry::new("com.limitswatcher.test", "test_key").unwrap();
    entry.set_password("test_value").unwrap();
    
    let retrieved = entry.get_password().unwrap();
    assert_eq!(retrieved, "test_value");
    
    entry.delete_credential().unwrap();
}
```

#### Windows Startup Registration
```rust
// src-tauri/src/autostart.rs
#[cfg(target_os = "windows")]
pub fn set_autostart(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    use winreg::enums::*;
    use winreg::RegKey;
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let key = hkcu.open_subkey_with_flags(path, KEY_WRITE)?;
    
    if enabled {
        let exe_path = std::env::current_exe()?;
        key.set_value("LimitsWatcher", &exe_path.to_string_lossy().to_string())?;
    } else {
        let _ = key.delete_value("LimitsWatcher");
    }
    
    Ok(())
}
```

#### Windows System Tray Behavior
```rust
// Ensure app minimizes to tray instead of closing
fn setup_window_behavior(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of close
                api.prevent_close();
                let _ = window_clone.hide();
            }
        });
    }
}
```

---

### Linux

#### Linux Secret Service (GNOME Keyring / KWallet)
```rust
#[cfg(target_os = "linux")]
pub fn check_secret_service() -> bool {
    // Check if secret service is available
    use keyring::Entry;
    
    match Entry::new("com.limitswatcher.test", "availability_check") {
        Ok(entry) => {
            match entry.set_password("test") {
                Ok(_) => {
                    let _ = entry.delete_credential();
                    true
                }
                Err(_) => false
            }
        }
        Err(_) => false
    }
}

// Fallback to encrypted file if secret service unavailable
pub fn get_credential_with_fallback(key: &str) -> Option<String> {
    // Try keyring first
    if let Ok(Some(value)) = keyring::get_credential(key) {
        return Some(value);
    }
    
    // Fallback to encrypted file
    let path = dirs::data_local_dir()?.join("LimitsWatcher").join("credentials.enc");
    encrypted::decrypt_from_file(&path, None).ok()
}
```

#### Linux Autostart (.desktop file)
```rust
#[cfg(target_os = "linux")]
pub fn set_autostart(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let autostart_dir = dirs::config_dir()
        .ok_or("No config dir")?
        .join("autostart");
    
    std::fs::create_dir_all(&autostart_dir)?;
    
    let desktop_file = autostart_dir.join("limitswatcher.desktop");
    
    if enabled {
        let exe_path = std::env::current_exe()?;
        let content = format!(
            r#"[Desktop Entry]
Type=Application
Name=LimitsWatcher
Exec={}
Icon=limitswatcher
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
"#,
            exe_path.display()
        );
        std::fs::write(&desktop_file, content)?;
    } else {
        let _ = std::fs::remove_file(&desktop_file);
    }
    
    Ok(())
}
```

#### Linux System Tray (AppIndicator)
```toml
# src-tauri/Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
# Already included in Tauri, but ensure libayatana-appindicator3 is installed
```

---

### macOS

#### macOS Keychain Integration
Keychain should work automatically via the `keyring` crate.

#### macOS Login Items
```rust
#[cfg(target_os = "macos")]
pub fn set_autostart(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    
    let bundle_id = "com.limitswatcher.app";
    
    if enabled {
        // Use osascript to add login item
        Command::new("osascript")
            .args([
                "-e",
                &format!(
                    r#"tell application "System Events" to make login item at end with properties {{path:"/Applications/LimitsWatcher.app", hidden:false}}"#
                )
            ])
            .output()?;
    } else {
        Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to delete login item "LimitsWatcher""#
            ])
            .output()?;
    }
    
    Ok(())
}
```

#### macOS Notarization
```bash
# In package.json or separate script
# Requires Apple Developer account

# 1. Code sign
codesign --deep --force --verify --verbose \
  --sign "Developer ID Application: Your Name (TEAMID)" \
  --options runtime \
  target/release/bundle/macos/LimitsWatcher.app

# 2. Create DMG
hdiutil create -volname "LimitsWatcher" -srcfolder target/release/bundle/macos/LimitsWatcher.app \
  -ov -format UDZO LimitsWatcher.dmg

# 3. Notarize
xcrun notarytool submit LimitsWatcher.dmg \
  --apple-id "your@email.com" \
  --password "app-specific-password" \
  --team-id "TEAMID" \
  --wait

# 4. Staple
xcrun stapler staple LimitsWatcher.dmg
```

---

## 2. Auto-Update System

### Tauri Updater Configuration

**tauri.conf.json:**
```json
{
  "plugins": {
    "updater": {
      "active": true,
      "dialog": true,
      "pubkey": "YOUR_PUBLIC_KEY_HERE",
      "endpoints": [
        "https://releases.limitswatcher.app/{{target}}/{{arch}}/{{current_version}}"
      ]
    }
  }
}
```

### Update Server Response Format
```json
{
  "version": "1.0.1",
  "notes": "Bug fixes and performance improvements",
  "pub_date": "2026-01-31T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "SIGNATURE_HERE",
      "url": "https://releases.limitswatcher.app/download/windows/LimitsWatcher_1.0.1_x64-setup.nsis.zip"
    },
    "linux-x86_64": {
      "signature": "SIGNATURE_HERE",
      "url": "https://releases.limitswatcher.app/download/linux/LimitsWatcher_1.0.1_amd64.AppImage.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "SIGNATURE_HERE",
      "url": "https://releases.limitswatcher.app/download/macos/LimitsWatcher_1.0.1_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "SIGNATURE_HERE",
      "url": "https://releases.limitswatcher.app/download/macos/LimitsWatcher_1.0.1_aarch64.app.tar.gz"
    }
  }
}
```

### Generate Update Keys
```bash
# Generate signing keys for updates
cargo tauri signer generate -w ~/.tauri/limitswatcher.key

# This outputs:
# - Private key: ~/.tauri/limitswatcher.key
# - Public key: (displayed, add to tauri.conf.json)
```

### Check for Updates (Frontend)
```tsx
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

async function checkForUpdates() {
  const update = await check();
  
  if (update?.available) {
    const confirmed = await confirm(
      `Version ${update.version} is available. Update now?`
    );
    
    if (confirmed) {
      await update.downloadAndInstall();
      await relaunch();
    }
  }
}
```

---

## 3. Build & Distribution

### Build Commands

```bash
# Development
npm run tauri dev

# Production builds
npm run tauri build

# Platform-specific
npm run tauri build -- --target x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-unknown-linux-gnu
npm run tauri build -- --target x86_64-apple-darwin
npm run tauri build -- --target aarch64-apple-darwin

# Universal macOS binary
npm run tauri build -- --target universal-apple-darwin
```

### GitHub Actions CI/CD

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - platform: macos-latest
            target: x86_64-apple-darwin
          - platform: macos-latest
            target: aarch64-apple-darwin

    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libsecret-1-dev
      
      - name: Install dependencies
        run: npm ci
      
      - name: Build
        run: npm run tauri build -- --target ${{ matrix.target }}
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: binaries-${{ matrix.target }}
          path: |
            target/${{ matrix.target }}/release/bundle/
  
  release:
    needs: build
    runs-on: ubuntu-latest
    
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            binaries-*/nsis/*.exe
            binaries-*/appimage/*.AppImage
            binaries-*/dmg/*.dmg
            binaries-*/macos/*.app.tar.gz
          draft: true
```

---

## 4. Distribution Checklist

### Windows
- [ ] NSIS installer works
- [ ] Portable version available
- [ ] Windows Defender doesn't flag (code signing helps)
- [ ] Startup registration works
- [ ] System tray icon visible

### Linux
- [ ] AppImage works on major distros
- [ ] .deb package for Debian/Ubuntu
- [ ] System tray visible (AppIndicator)
- [ ] Secret Service fallback works
- [ ] Autostart .desktop file works

### macOS
- [ ] App bundle signed
- [ ] App notarized
- [ ] DMG created
- [ ] Gatekeeper passes
- [ ] Login item registration works
- [ ] Menu bar icon visible

### All Platforms
- [ ] Auto-update works
- [ ] Crash reporting (optional)
- [ ] Analytics (optional, privacy-respecting)

---

## 5. Testing Matrix

| Feature | Windows | Linux | macOS |
|---------|---------|-------|-------|
| Install | ✓ | ✓ | ✓ |
| System tray | ✓ | ✓ | ✓ |
| Notifications | ✓ | ✓ | ✓ |
| Keychain | ✓ | ✓ | ✓ |
| Autostart | ✓ | ✓ | ✓ |
| Widget windows | ✓ | ✓ | ✓ |
| Auto-update | ✓ | ✓ | ✓ |
| Copilot auth | ✓ | ✓ | ✓ |
| Claude auth | ✓ | ✓ | ✓ |
| Gemini auth | ✓ | ✓ | ✓ |
| Antigravity | ✓ | ✓ | ✓ |

---

## Checklist

- [ ] Windows polish
  - [ ] Credential Manager working
  - [ ] Startup registration
  - [ ] Minimize to tray
- [ ] Linux polish
  - [ ] Secret Service working (+ fallback)
  - [ ] Autostart .desktop file
  - [ ] AppIndicator tray
- [ ] macOS polish
  - [ ] Keychain working
  - [ ] Login item registration
  - [ ] Code signing
  - [ ] Notarization
- [ ] Auto-update system
  - [ ] Signing keys generated
  - [ ] Update endpoints configured
  - [ ] Update UI implemented
- [ ] CI/CD pipeline
  - [ ] GitHub Actions workflow
  - [ ] All platforms building
  - [ ] Artifacts uploaded
- [ ] Distribution
  - [ ] GitHub Releases
  - [ ] Update manifest file

---

## Next Steps
- **Phase 5:** Extended providers → See `PHASE-5-EXTENDED.md` (future)
