import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CopilotAuth } from "./providers/CopilotAuth";
import { GeminiAuth } from "./providers/GeminiAuth";

interface ProviderStatus {
  provider: string;
  enabled: boolean;
  authenticated: boolean;
  model_quotas?: ModelQuota[];
}

interface ModelQuota {
  model_id: string;
  percent_left: number;
  reset_time?: string;
}

interface AuthStatus {
  authenticated: boolean;
  user?: string;
  plan?: string;
  expires?: string;
}

export function Settings({ onBack }: { onBack: () => void }) {
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  const [geminiAuthStatus, setGeminiAuthStatus] = useState<AuthStatus | null>(null);

  useEffect(() => {
    loadStatus();
  }, []);

  async function loadStatus() {
    try {
      const usage = await invoke<any[]>("get_all_usage");
      setProviders(usage.map(u => ({
        provider: u.provider,
        enabled: u.enabled,
        authenticated: u.authenticated,
        model_quotas: u.model_quotas
      })));

      // Fetch Gemini auth status
      try {
        const geminiStatus = await invoke<AuthStatus>("get_provider_auth_status", { provider: "gemini" });
        setGeminiAuthStatus(geminiStatus);
      } catch (e) {
        console.error("Failed to fetch Gemini auth status:", e);
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function toggleProvider(provider: string, enabled: boolean) {
    await invoke("set_provider_enabled", { provider, enabled });
    loadStatus();
  }

  const copilot = providers.find(p => p.provider === "copilot");
  const gemini = providers.find(p => p.provider === "gemini");

  function formatResetTime(resetTime?: string): string {
    if (!resetTime) return "";

    const resetDate = new Date(resetTime);
    const now = new Date();
    const diffMs = resetDate.getTime() - now.getTime();

    if (diffMs <= 0) return "Now";

    const hours = Math.floor(diffMs / (1000 * 60 * 60));
    const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));

    if (hours > 24) {
      const days = Math.floor(hours / 24);
      return `in ${days}d ${hours % 24}h`;
    }
    return `in ${hours}h ${minutes}m`;
  }

  return (
    <div className="container">
      <header>
        <h2>Settings</h2>
        <button onClick={onBack}>Back</button>
      </header>

      <div className="settings-section">
        <h3>Providers</h3>

        {/* Copilot Config */}
        <div className="provider-config">
          <div className="config-header">
            <span style={{ fontSize: '1.1em', fontWeight: 500 }}>GitHub Copilot</span>
            <label className="switch">
              <input
                type="checkbox"
                checked={copilot?.enabled || false}
                onChange={(e) => toggleProvider("copilot", e.target.checked)}
              />
              <span>Enabled</span>
            </label>
          </div>

          {copilot?.authenticated ? (
            <div style={{ padding: '10px', background: '#e8f5e9', color: '#2e7d32', borderRadius: '6px', textAlign: 'center' }}>
              ✓ Connected to GitHub
              <button
                style={{ marginLeft: '10px', fontSize: '0.8em', background: 'transparent', color: '#c62828', border: '1px solid #ffcdd2', padding: '2px 8px' }}
                onClick={async () => {
                  // TODO: Add logout command backend-side if needed, for now just show connected
                }}
              >
                Disconnect (Not Implemented)
              </button>
            </div>
          ) : (
            <CopilotAuth onComplete={() => {
              toggleProvider("copilot", true);
              loadStatus();
            }} />
          )}
        </div>

        {/* Gemini Config */}
        <div className="provider-config">
          <div className="config-header">
            <span style={{ fontSize: '1.1em', fontWeight: 500 }}>Gemini</span>
            <label className="switch">
              <input
                type="checkbox"
                checked={gemini?.enabled || false}
                onChange={(e) => toggleProvider("gemini", e.target.checked)}
              />
              <span>Enabled</span>
            </label>
          </div>

          {gemini?.authenticated ? (
            <div>
              <GeminiAuth
                authStatus={geminiAuthStatus ?? undefined}
                onComplete={loadStatus}
              />

              {/* Per-model quotas display */}
              {gemini.model_quotas && gemini.model_quotas.length > 0 && (
                <div style={{
                  marginTop: '10px',
                  padding: '10px',
                  background: '#f5f5f5',
                  borderRadius: '6px',
                  fontSize: '0.9em'
                }}>
                  <div style={{ fontWeight: 500, marginBottom: '8px' }}>Model Quotas:</div>
                  {gemini.model_quotas.map((quota) => (
                    <div key={quota.model_id} style={{
                      padding: '6px 0',
                      borderBottom: '1px solid #e0e0e0',
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center'
                    }}>
                      <span style={{ fontWeight: 500 }}>{quota.model_id}</span>
                      <div style={{ textAlign: 'right' }}>
                        <span style={{
                          color: quota.percent_left > 50 ? '#2e7d32' : quota.percent_left > 20 ? '#f57c00' : '#c62828',
                          fontWeight: 600
                        }}>
                          {quota.percent_left.toFixed(1)}% remaining
                        </span>
                        {quota.reset_time && (
                          <div style={{ fontSize: '0.85em', color: '#666', marginTop: '2px' }}>
                            Resets {formatResetTime(quota.reset_time)}
                          </div>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}

              <button
                style={{
                  marginTop: '10px',
                  width: '100%',
                  fontSize: '0.9em',
                  background: 'transparent',
                  color: '#c62828',
                  border: '1px solid #ffcdd2',
                  padding: '6px 12px'
                }}
                onClick={async () => {
                  await invoke("logout_provider", { provider: "gemini" });
                  loadStatus();
                }}
              >
                Disconnect
              </button>
            </div>
          ) : (
            <GeminiAuth onComplete={() => {
              toggleProvider("gemini", true);
              loadStatus();
            }} />
          )}
        </div>

        {/* Placeholders for others */}
        <div className="provider-config" style={{ opacity: 0.5 }}>
          <span style={{ fontSize: '1.1em', fontWeight: 500 }}>Claude (Coming Soon)</span>
        </div>
      </div>
    </div>
  );
}
