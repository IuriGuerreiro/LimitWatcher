import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface ProviderUsage {
  provider: string;
  enabled: boolean;
  session_used: number;
  session_limit: number;
  weekly_used: number;
  weekly_limit: number;
  reset_time: string | null;
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

  return (
    <div className={`provider-card ${usage.enabled ? "" : "disabled"}`}>
      <h3>{usage.provider}</h3>

      {usage.error ? (
        <p className="error">{usage.error}</p>
      ) : (
        <>
          <div className="usage-bar">
            <label>
              Session: {usage.session_used}/{usage.session_limit}
            </label>
            <div className="bar">
              <div className="fill" style={{ width: `${sessionPercent}%` }} />
            </div>
          </div>

          <div className="usage-bar">
            <label>
              Weekly: {usage.weekly_used}/{usage.weekly_limit}
            </label>
            <div className="bar">
              <div className="fill" style={{ width: `${weeklyPercent}%` }} />
            </div>
          </div>

          {usage.reset_time && <p className="reset">Resets: {usage.reset_time}</p>}
        </>
      )}
    </div>
  );
}

export default App;
