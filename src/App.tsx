import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Settings } from "./components/Settings";

interface ProviderUsage {
  provider: string;
  enabled: boolean;
  authenticated: boolean;
  session_used: number;
  session_limit: number;
  weekly_used: number;
  weekly_limit: number;
  reset_time: string | null;
  error: string | null;
}

function App() {
  const [view, setView] = useState<"dashboard" | "settings">("dashboard");
  const [providers, setProviders] = useState<ProviderUsage[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (view === "dashboard") {
      loadProviders();
    }
  }, [view]);

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

  if (view === "settings") {
    return <Settings onBack={() => setView("dashboard")} />;
  }

  return (
    <main className="container">
      <header>
        <div>
          <h1>LimitsWatcher</h1>
          <p style={{ color: '#888', margin: 0 }}>AI Subscription Usage Tracker</p>
        </div>
        <div className="header-actions">
          <button onClick={() => setView("settings")}>
            Settings
          </button>
          <button className="primary" onClick={refreshAll} disabled={loading}>
            {loading ? "Refreshing..." : "Refresh All"}
          </button>
        </div>
      </header>

      <div className="providers-grid">
        {providers.map((p) => (
          <ProviderCard key={p.provider} usage={p} onRefresh={loadProviders} />
        ))}
        {providers.length === 0 && !loading && (
           <div style={{ gridColumn: '1 / -1', textAlign: 'center', padding: '40px', color: '#888' }}>
             <p>No providers enabled. Go to Settings to configure them.</p>
           </div>
        )}
      </div>
    </main>
  );
}

function ProviderCard({
  usage,
  onRefresh,
}: {
  usage: ProviderUsage;
  onRefresh: () => void;
}) {
  const sessionPercent =
    usage.session_limit > 0
      ? (usage.session_used / usage.session_limit) * 100
      : 0;
  const weeklyPercent =
    usage.weekly_limit > 0
      ? (usage.weekly_used / usage.weekly_limit) * 100
      : 0;

  if (!usage.enabled) return null;

  return (
    <div className="provider-card">
      <h3>{usage.provider}</h3>

      {usage.error ? (
        <p className="error">{usage.error}</p>
      ) : (
        <div>
          <div className="usage-bar">
            <div className="usage-info">
              <label>Session</label>
              <span>{usage.session_used} / {usage.session_limit > 0 ? usage.session_limit : "∞"}</span>
            </div>
            {usage.session_limit > 0 && (
                <div className="bar">
                <div 
                    className="fill"
                    style={{ width: `${Math.min(sessionPercent, 100)}%` }} 
                />
                </div>
            )}
          </div>

          <div className="usage-bar">
             <div className="usage-info">
              <label>Weekly</label>
              <span>{usage.weekly_used} / {usage.weekly_limit > 0 ? usage.weekly_limit : "∞"}</span>
            </div>
            {usage.weekly_limit > 0 && (
                <div className="bar">
                <div 
                    className="fill"
                    style={{ width: `${Math.min(weeklyPercent, 100)}%` }} 
                />
                </div>
            )}
          </div>

          {usage.reset_time && (
            <p className="reset">
              Resets: {new Date(usage.reset_time).toLocaleString()}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
