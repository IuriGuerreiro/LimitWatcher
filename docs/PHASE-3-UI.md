# Phase 3: UI & Notifications

## Overview
Implement the user interface including dashboard, system tray updates, widget windows, and notification system.

---

## 1. Main Dashboard

### App.tsx - Full Implementation

```tsx
// src/App.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsPane } from "./components/SettingsPane";
import { AuthModal } from "./components/AuthModal";
import "./App.css";

export interface ProviderStatus {
  provider: string;
  enabled: boolean;
  session_used: number;
  session_limit: number;
  weekly_used: number;
  weekly_limit: number;
  reset_time: string | null;
  error: string | null;
}

export interface ProviderInfo {
  id: string;
  name: string;
  website: string;
  auth_methods: string[];
  has_session_limits: boolean;
  has_weekly_limits: boolean;
  has_credits: boolean;
  icon: string;
}

type View = "dashboard" | "settings";

function App() {
  const [view, setView] = useState<View>("dashboard");
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [authProvider, setAuthProvider] = useState<string | null>(null);

  useEffect(() => {
    loadProviders();
    
    // Listen for refresh events from tray
    const unlisten = listen("refresh-all", () => {
      refreshAll();
    });
    
    // Listen for provider updates
    const unlistenUpdate = listen<[string, ProviderStatus]>("provider-updated", (event) => {
      const [name, status] = event.payload;
      setProviders(prev => prev.map(p => 
        p.provider === name ? { ...p, ...status } : p
      ));
    });
    
    return () => {
      unlisten.then(f => f());
      unlistenUpdate.then(f => f());
    };
  }, []);

  async function loadProviders() {
    try {
      const usage = await invoke<ProviderStatus[]>("get_all_usage");
      setProviders(usage);
    } catch (e) {
      console.error("Failed to load providers:", e);
    } finally {
      setLoading(false);
    }
  }

  async function refreshAll() {
    setRefreshing(true);
    try {
      for (const p of providers.filter(p => p.enabled)) {
        await invoke("refresh_provider", { provider: p.provider });
      }
      await loadProviders();
    } finally {
      setRefreshing(false);
    }
  }

  async function toggleProvider(provider: string, enabled: boolean) {
    await invoke("set_provider_enabled", { provider, enabled });
    setProviders(prev => prev.map(p => 
      p.provider === provider ? { ...p, enabled } : p
    ));
    
    if (enabled) {
      // Trigger auth if needed
      const status = await invoke<{ authenticated: boolean }>("get_auth_status", { provider });
      if (!status.authenticated) {
        setAuthProvider(provider);
      }
    }
  }

  const enabledProviders = providers.filter(p => p.enabled);
  const disabledProviders = providers.filter(p => !p.enabled);

  return (
    <main className="app">
      <header className="app-header">
        <h1>LimitsWatcher</h1>
        <nav>
          <button 
            className={view === "dashboard" ? "active" : ""} 
            onClick={() => setView("dashboard")}
          >
            Dashboard
          </button>
          <button 
            className={view === "settings" ? "active" : ""} 
            onClick={() => setView("settings")}
          >
            Settings
          </button>
        </nav>
        <button 
          className="refresh-btn" 
          onClick={refreshAll} 
          disabled={refreshing || loading}
        >
          {refreshing ? "⟳" : "↻"} Refresh
        </button>
      </header>

      {loading ? (
        <div className="loading">Loading providers...</div>
      ) : view === "dashboard" ? (
        <div className="dashboard">
          {enabledProviders.length === 0 ? (
            <div className="empty-state">
              <p>No providers enabled yet.</p>
              <button onClick={() => setView("settings")}>
                Go to Settings to enable providers
              </button>
            </div>
          ) : (
            <div className="providers-grid">
              {enabledProviders.map(p => (
                <ProviderCard 
                  key={p.provider} 
                  status={p}
                  onRefresh={() => invoke("refresh_provider", { provider: p.provider }).then(loadProviders)}
                  onConfigure={() => setAuthProvider(p.provider)}
                />
              ))}
            </div>
          )}
        </div>
      ) : (
        <SettingsPane 
          providers={providers}
          onToggleProvider={toggleProvider}
          onConfigureProvider={setAuthProvider}
        />
      )}

      {authProvider && (
        <AuthModal 
          provider={authProvider}
          onClose={() => {
            setAuthProvider(null);
            loadProviders();
          }}
        />
      )}
    </main>
  );
}

export default App;
```

---

## 2. Provider Card Component

```tsx
// src/components/ProviderCard.tsx
import { ProviderStatus } from "../App";

interface Props {
  status: ProviderStatus;
  onRefresh: () => void;
  onConfigure: () => void;
}

export function ProviderCard({ status, onRefresh, onConfigure }: Props) {
  const sessionPercent = status.session_limit > 0 
    ? (status.session_used / status.session_limit) * 100 
    : 0;
  const weeklyPercent = status.weekly_limit > 0 
    ? (status.weekly_used / status.weekly_limit) * 100 
    : 0;
  
  const getBarColor = (percent: number) => {
    if (percent >= 90) return "var(--color-danger)";
    if (percent >= 70) return "var(--color-warning)";
    return "var(--color-success)";
  };
  
  const formatResetTime = (isoString: string | null) => {
    if (!isoString) return null;
    const date = new Date(isoString);
    const now = new Date();
    const diff = date.getTime() - now.getTime();
    
    if (diff < 0) return "Soon";
    
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    
    if (hours > 24) {
      const days = Math.floor(hours / 24);
      return `${days}d ${hours % 24}h`;
    }
    return `${hours}h ${minutes}m`;
  };

  return (
    <div className={`provider-card ${status.error ? "error" : ""}`}>
      <div className="card-header">
        <div className="provider-icon">
          <img src={`/icons/${status.provider}.svg`} alt="" />
        </div>
        <h3>{status.provider}</h3>
        <div className="card-actions">
          <button onClick={onRefresh} title="Refresh">↻</button>
          <button onClick={onConfigure} title="Configure">⚙</button>
        </div>
      </div>
      
      {status.error ? (
        <div className="error-message">
          <p>{status.error}</p>
          <button onClick={onConfigure}>Fix</button>
        </div>
      ) : (
        <div className="usage-info">
          {status.session_limit > 0 && (
            <div className="usage-row">
              <div className="usage-label">
                <span>Session</span>
                <span>{status.session_used} / {status.session_limit}</span>
              </div>
              <div className="usage-bar">
                <div 
                  className="usage-fill" 
                  style={{ 
                    width: `${Math.min(sessionPercent, 100)}%`,
                    backgroundColor: getBarColor(sessionPercent)
                  }} 
                />
              </div>
            </div>
          )}
          
          {status.weekly_limit > 0 && (
            <div className="usage-row">
              <div className="usage-label">
                <span>Weekly</span>
                <span>{status.weekly_used} / {status.weekly_limit}</span>
              </div>
              <div className="usage-bar">
                <div 
                  className="usage-fill" 
                  style={{ 
                    width: `${Math.min(weeklyPercent, 100)}%`,
                    backgroundColor: getBarColor(weeklyPercent)
                  }} 
                />
              </div>
            </div>
          )}
          
          {status.reset_time && (
            <div className="reset-time">
              Resets in: {formatResetTime(status.reset_time)}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

---

## 3. Settings Pane

```tsx
// src/components/SettingsPane.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProviderStatus } from "../App";

interface Props {
  providers: ProviderStatus[];
  onToggleProvider: (provider: string, enabled: boolean) => void;
  onConfigureProvider: (provider: string) => void;
}

type RefreshInterval = "manual" | "1m" | "2m" | "5m" | "15m";

export function SettingsPane({ providers, onToggleProvider, onConfigureProvider }: Props) {
  const [refreshInterval, setRefreshInterval] = useState<RefreshInterval>("5m");
  const [notifications, setNotifications] = useState({
    enabled: true,
    warningThreshold: 80,
    criticalThreshold: 90,
  });

  async function updateRefreshInterval(interval: RefreshInterval) {
    setRefreshInterval(interval);
    await invoke("set_refresh_interval", { interval });
  }

  async function updateNotificationSettings(settings: typeof notifications) {
    setNotifications(settings);
    await invoke("set_notification_settings", { settings });
  }

  return (
    <div className="settings-pane">
      <section className="settings-section">
        <h2>Providers</h2>
        <p className="section-desc">Enable the AI services you want to track.</p>
        
        <div className="provider-list">
          {providers.map(p => (
            <div key={p.provider} className="provider-item">
              <div className="provider-info">
                <img src={`/icons/${p.provider}.svg`} alt="" className="provider-icon-small" />
                <span className="provider-name">{p.provider}</span>
              </div>
              <div className="provider-controls">
                <button 
                  className="configure-btn"
                  onClick={() => onConfigureProvider(p.provider)}
                >
                  Configure
                </button>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={p.enabled}
                    onChange={(e) => onToggleProvider(p.provider, e.target.checked)}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h2>Refresh Interval</h2>
        <p className="section-desc">How often to check for usage updates.</p>
        
        <div className="radio-group">
          {(["manual", "1m", "2m", "5m", "15m"] as RefreshInterval[]).map(interval => (
            <label key={interval} className="radio-item">
              <input
                type="radio"
                name="refresh"
                value={interval}
                checked={refreshInterval === interval}
                onChange={() => updateRefreshInterval(interval)}
              />
              <span>{interval === "manual" ? "Manual only" : interval}</span>
            </label>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h2>Notifications</h2>
        <p className="section-desc">Get notified when usage is high.</p>
        
        <label className="checkbox-item">
          <input
            type="checkbox"
            checked={notifications.enabled}
            onChange={(e) => updateNotificationSettings({
              ...notifications,
              enabled: e.target.checked
            })}
          />
          <span>Enable notifications</span>
        </label>
        
        {notifications.enabled && (
          <>
            <div className="slider-setting">
              <label>Warning at: {notifications.warningThreshold}%</label>
              <input
                type="range"
                min="50"
                max="95"
                step="5"
                value={notifications.warningThreshold}
                onChange={(e) => updateNotificationSettings({
                  ...notifications,
                  warningThreshold: parseInt(e.target.value)
                })}
              />
            </div>
            
            <div className="slider-setting">
              <label>Critical at: {notifications.criticalThreshold}%</label>
              <input
                type="range"
                min="60"
                max="100"
                step="5"
                value={notifications.criticalThreshold}
                onChange={(e) => updateNotificationSettings({
                  ...notifications,
                  criticalThreshold: parseInt(e.target.value)
                })}
              />
            </div>
          </>
        )}
      </section>
    </div>
  );
}
```

---

## 4. Widget Window

Create a separate window for floating widget display:

### Widget Entry Point

```tsx
// src/Widget.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./Widget.css";

interface WidgetProps {
  provider: string;
}

export function Widget({ provider }: WidgetProps) {
  const [usage, setUsage] = useState({
    session_used: 0,
    session_limit: 0,
    weekly_used: 0,
    weekly_limit: 0,
    reset_time: null as string | null,
  });
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    // Load initial data
    loadUsage();
    
    // Listen for updates
    const unlisten = listen<[string, any]>("provider-updated", (event) => {
      const [name, data] = event.payload;
      if (name === provider) {
        setUsage(data);
      }
    });
    
    return () => { unlisten.then(f => f()); };
  }, [provider]);

  async function loadUsage() {
    const status = await invoke<any>("get_provider_status", { provider });
    setUsage(status);
  }

  // Drag handling for widget positioning
  async function onMouseDown(e: React.MouseEvent) {
    if (e.button === 0) {
      setDragging(true);
      const window = getCurrentWindow();
      await window.startDragging();
    }
  }

  const sessionPercent = usage.session_limit > 0 
    ? (usage.session_used / usage.session_limit) * 100 
    : 0;

  return (
    <div 
      className="widget" 
      onMouseDown={onMouseDown}
      data-dragging={dragging}
    >
      <div className="widget-header">
        <img src={`/icons/${provider}.svg`} alt="" />
        <span>{provider}</span>
      </div>
      
      <div className="widget-bars">
        <div className="mini-bar">
          <div 
            className="mini-bar-fill"
            style={{ width: `${sessionPercent}%` }}
          />
        </div>
      </div>
      
      <div className="widget-stats">
        <span>{usage.session_used}/{usage.session_limit}</span>
      </div>
    </div>
  );
}
```

### Widget CSS

```css
/* src/Widget.css */
.widget {
  width: 120px;
  padding: 8px;
  background: rgba(30, 30, 30, 0.95);
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: white;
  font-family: system-ui, -apple-system, sans-serif;
  font-size: 12px;
  cursor: grab;
  user-select: none;
  -webkit-app-region: drag;
}

.widget[data-dragging="true"] {
  cursor: grabbing;
}

.widget-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}

.widget-header img {
  width: 16px;
  height: 16px;
}

.widget-bars {
  margin-bottom: 4px;
}

.mini-bar {
  height: 4px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
  overflow: hidden;
}

.mini-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #4ade80, #facc15, #f87171);
  background-size: 200% 100%;
  transition: width 0.3s ease;
}

.widget-stats {
  text-align: center;
  opacity: 0.8;
}
```

### Creating Widget Windows (Rust)

```rust
// src-tauri/src/widget.rs
use tauri::{Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub fn create_widget_window<R: Runtime>(
    app: &tauri::AppHandle<R>,
    provider: &str,
) -> tauri::Result<()> {
    let label = format!("widget-{}", provider);
    
    // Check if window already exists
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }
    
    let url = WebviewUrl::App(format!("/widget?provider={}", provider).into());
    
    WebviewWindowBuilder::new(app, &label, url)
        .title(&format!("{} Widget", provider))
        .inner_size(130.0, 80.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(true)
        .build()?;
    
    Ok(())
}

pub fn close_widget_window<R: Runtime>(
    app: &tauri::AppHandle<R>,
    provider: &str,
) -> tauri::Result<()> {
    let label = format!("widget-{}", provider);
    
    if let Some(window) = app.get_webview_window(&label) {
        window.close()?;
    }
    
    Ok(())
}
```

---

## 5. Dynamic Tray Icon

Generate dynamic tray icons based on usage:

```rust
// src-tauri/src/tray_icon.rs
use image::{Rgba, RgbaImage};

const ICON_SIZE: u32 = 22;

pub struct UsageBar {
    pub percent: f32,
    pub color: BarColor,
}

pub enum BarColor {
    Green,
    Yellow,
    Red,
    Gray,
}

impl BarColor {
    fn to_rgba(&self) -> Rgba<u8> {
        match self {
            BarColor::Green => Rgba([74, 222, 128, 255]),
            BarColor::Yellow => Rgba([250, 204, 21, 255]),
            BarColor::Red => Rgba([248, 113, 113, 255]),
            BarColor::Gray => Rgba([100, 100, 100, 255]),
        }
    }
}

/// Generate a tray icon with usage bars
pub fn generate_tray_icon(session: Option<UsageBar>, weekly: Option<UsageBar>) -> Vec<u8> {
    let mut img = RgbaImage::new(ICON_SIZE, ICON_SIZE);
    
    // Background (transparent)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }
    
    let bar_width = ICON_SIZE - 4;
    let bar_height = 4;
    
    // Draw session bar (top)
    if let Some(bar) = session {
        let y_start = 6;
        let fill_width = ((bar_width as f32) * bar.percent.min(1.0)) as u32;
        
        // Background
        for x in 2..(2 + bar_width) {
            for y in y_start..(y_start + bar_height) {
                img.put_pixel(x, y, Rgba([60, 60, 60, 255]));
            }
        }
        
        // Fill
        for x in 2..(2 + fill_width) {
            for y in y_start..(y_start + bar_height) {
                img.put_pixel(x, y, bar.color.to_rgba());
            }
        }
    }
    
    // Draw weekly bar (bottom)
    if let Some(bar) = weekly {
        let y_start = 12;
        let fill_width = ((bar_width as f32) * bar.percent.min(1.0)) as u32;
        
        // Background
        for x in 2..(2 + bar_width) {
            for y in y_start..(y_start + bar_height) {
                img.put_pixel(x, y, Rgba([60, 60, 60, 255]));
            }
        }
        
        // Fill
        for x in 2..(2 + fill_width) {
            for y in y_start..(y_start + bar_height) {
                img.put_pixel(x, y, bar.color.to_rgba());
            }
        }
    }
    
    // Convert to PNG bytes
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("Failed to encode icon");
    bytes
}

/// Update tray icon based on provider status
pub fn update_tray_from_usage(
    session_used: u64,
    session_limit: u64,
    weekly_used: u64,
    weekly_limit: u64,
) -> Vec<u8> {
    let session_bar = if session_limit > 0 {
        let percent = session_used as f32 / session_limit as f32;
        let color = if percent >= 0.9 {
            BarColor::Red
        } else if percent >= 0.7 {
            BarColor::Yellow
        } else {
            BarColor::Green
        };
        Some(UsageBar { percent, color })
    } else {
        None
    };
    
    let weekly_bar = if weekly_limit > 0 {
        let percent = weekly_used as f32 / weekly_limit as f32;
        let color = if percent >= 0.9 {
            BarColor::Red
        } else if percent >= 0.7 {
            BarColor::Yellow
        } else {
            BarColor::Green
        };
        Some(UsageBar { percent, color })
    } else {
        None
    };
    
    generate_tray_icon(session_bar, weekly_bar)
}
```

---

## 6. CSS Styles

```css
/* src/App.css */
:root {
  --color-bg: #1a1a1a;
  --color-surface: #2a2a2a;
  --color-border: #3a3a3a;
  --color-text: #ffffff;
  --color-text-muted: #888888;
  --color-primary: #3b82f6;
  --color-success: #4ade80;
  --color-warning: #facc15;
  --color-danger: #f87171;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  background: var(--color-bg);
  color: var(--color-text);
  font-family: system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
}

.app {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border);
  gap: 24px;
}

.app-header h1 {
  font-size: 18px;
  font-weight: 600;
}

.app-header nav {
  display: flex;
  gap: 8px;
}

.app-header nav button {
  padding: 6px 12px;
  background: transparent;
  border: none;
  color: var(--color-text-muted);
  cursor: pointer;
  border-radius: 4px;
}

.app-header nav button.active {
  background: var(--color-primary);
  color: white;
}

.refresh-btn {
  margin-left: auto;
  padding: 6px 12px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  color: var(--color-text);
  border-radius: 4px;
  cursor: pointer;
}

.refresh-btn:hover {
  background: var(--color-border);
}

/* Dashboard */
.dashboard {
  flex: 1;
  padding: 24px;
}

.providers-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.provider-card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 16px;
}

.provider-card.error {
  border-color: var(--color-danger);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.card-header h3 {
  flex: 1;
  font-size: 16px;
  text-transform: capitalize;
}

.card-actions button {
  padding: 4px 8px;
  background: transparent;
  border: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.usage-row {
  margin-bottom: 12px;
}

.usage-label {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--color-text-muted);
  margin-bottom: 4px;
}

.usage-bar {
  height: 8px;
  background: var(--color-border);
  border-radius: 4px;
  overflow: hidden;
}

.usage-fill {
  height: 100%;
  border-radius: 4px;
  transition: width 0.3s ease;
}

.reset-time {
  font-size: 12px;
  color: var(--color-text-muted);
  text-align: right;
}

.error-message {
  color: var(--color-danger);
  font-size: 14px;
}

/* Settings */
.settings-pane {
  flex: 1;
  padding: 24px;
  max-width: 600px;
}

.settings-section {
  margin-bottom: 32px;
}

.settings-section h2 {
  font-size: 16px;
  margin-bottom: 8px;
}

.section-desc {
  color: var(--color-text-muted);
  font-size: 14px;
  margin-bottom: 16px;
}

.provider-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.provider-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  background: var(--color-surface);
  border-radius: 8px;
}

.provider-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* Toggle switch */
.toggle {
  position: relative;
  width: 44px;
  height: 24px;
}

.toggle input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  inset: 0;
  background: var(--color-border);
  border-radius: 12px;
  cursor: pointer;
  transition: 0.2s;
}

.toggle-slider::before {
  content: "";
  position: absolute;
  width: 18px;
  height: 18px;
  left: 3px;
  top: 3px;
  background: white;
  border-radius: 50%;
  transition: 0.2s;
}

.toggle input:checked + .toggle-slider {
  background: var(--color-primary);
}

.toggle input:checked + .toggle-slider::before {
  transform: translateX(20px);
}
```

---

## Checklist

- [ ] Main dashboard UI
  - [ ] Provider grid layout
  - [ ] Provider cards with usage bars
  - [ ] Refresh functionality
- [ ] Settings pane
  - [ ] Provider enable/disable toggles
  - [ ] Refresh interval selector
  - [ ] Notification settings
- [ ] Widget windows
  - [ ] Floating always-on-top windows
  - [ ] Draggable positioning
  - [ ] Transparent background
- [ ] Dynamic tray icon
  - [ ] Generate usage bar icons
  - [ ] Update on usage change
- [ ] Auth modal for provider setup
- [ ] CSS styling complete

---

## Next Steps
- **Phase 4:** Platform polish → See `PHASE-4-POLISH.md`
