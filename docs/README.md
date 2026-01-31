# LimitsWatcher Documentation Index

## Overview
LimitsWatcher is a cross-platform (Windows, Linux, macOS) application for tracking AI subscription usage and limits.

## Quick Links

### Phase Documentation
| Phase | Document | Description |
|-------|----------|-------------|
| 0 | [PHASE-0-SETUP.md](PHASE-0-SETUP.md) | Project initialization, Tauri setup, dependencies |
| 1 | [PHASE-1-CORE.md](PHASE-1-CORE.md) | Storage layer, system tray, scheduler, notifications |
| 2 | [PHASE-2-PROVIDERS.md](PHASE-2-PROVIDERS.md) | Provider framework and registry |
| 3 | [PHASE-3-UI.md](PHASE-3-UI.md) | Dashboard, settings, widgets, styling |
| 4 | [PHASE-4-POLISH.md](PHASE-4-POLISH.md) | Platform polish, auto-update, distribution |

### Provider Documentation
| Provider | Document | Auth Method | Priority |
|----------|----------|-------------|----------|
| GitHub Copilot | [providers/COPILOT.md](providers/COPILOT.md) | Device Flow OAuth | Phase 1 |
| Claude | [providers/CLAUDE.md](providers/CLAUDE.md) | OAuth / Cookies | Phase 1 |
| Gemini | [providers/GEMINI.md](providers/GEMINI.md) | CLI OAuth | Phase 1 |
| Antigravity | [providers/ANTIGRAVITY.md](providers/ANTIGRAVITY.md) | Local Probe | Phase 1 |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Tauri v2 Application                      │
├─────────────────────────────────────────────────────────────────┤
│  Frontend (TypeScript/React)                                    │
│  ├── Main Window (Dashboard, Settings)                          │
│  ├── Tray Menu (Quick status, actions)                          │
│  └── Widget Windows (Floating overlays)                         │
├─────────────────────────────────────────────────────────────────┤
│  Rust Backend                                                   │
│  ├── providers/          # AI provider implementations          │
│  ├── storage/            # Keychain + encrypted storage         │
│  ├── scheduler.rs        # Background refresh                   │
│  ├── notifications.rs    # Usage alerts                         │
│  └── tray.rs             # System tray                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Order

### Recommended Approach
You can work on multiple components in parallel:

1. **Core Team (1 person):** Phase 0 → Phase 1 → Phase 3
2. **Provider Team (1+ people):** Phase 2 providers (independent work)

### Parallel Workstreams

```
Week 1:
├── Person A: Phase 0 (Setup) → Phase 1 (Core)
├── Person B: Copilot provider implementation
└── Person C: Claude provider implementation

Week 2:
├── Person A: Phase 3 (UI)
├── Person B: Gemini provider implementation
└── Person C: Antigravity provider implementation

Week 3:
├── All: Integration and testing
└── All: Phase 4 (Polish and distribution)
```

---

## Key Technologies

| Component | Technology | Why |
|-----------|------------|-----|
| Framework | Tauri v2 | Small bundle, native features, Rust backend |
| Frontend | React + TypeScript | Familiar, good ecosystem |
| Credentials | OS Keychain | Most secure, platform-native |
| Encryption | AES-256-GCM | Industry standard for at-rest encryption |
| Key Derivation | Argon2id | Memory-hard, resistant to GPU attacks |

---

## Security Model

### Credential Storage
1. **API Keys/Tokens** → OS Keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service)
2. **Cookies** → AES-256-GCM encrypted files (machine-bound)
3. **Usage Data** → Plain JSON cache (non-sensitive)

### Key Derivation
- Keys derived using Argon2id
- Machine ID binding prevents credential theft via file copy

---

## Native Features

| Feature | Windows | Linux | macOS |
|---------|---------|-------|-------|
| System Tray | ✓ | ✓ (AppIndicator) | ✓ (Menu Bar) |
| Notifications | ✓ | ✓ | ✓ |
| Floating Widgets | ✓ | ✓ | ✓ |
| Auto-start | Registry | .desktop file | Login Items |
| Keychain | Credential Manager | Secret Service | Keychain |

---

## Reference

This project is inspired by [CodexBar](https://github.com/steipete/CodexBar), a macOS-only Swift app. The implementation documents reference CodexBar's architecture and provider implementations.

Key differences from CodexBar:
- Cross-platform (not just macOS)
- Tauri/Rust instead of Swift
- Simplified widget approach (floating windows vs WidgetKit)

---

## Getting Started

1. **Read Phase 0:** [PHASE-0-SETUP.md](PHASE-0-SETUP.md)
2. **Install prerequisites**
3. **Create Tauri project**
4. **Pick a provider to implement**
5. **Follow the phase documents in order**

---

## Contributing

Each provider can be implemented independently. Pick a provider doc from `providers/` and follow the implementation guide. The provider trait ensures consistency across all implementations.
