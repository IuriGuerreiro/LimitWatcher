import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CopilotAuth } from "./providers/CopilotAuth";

interface ProviderStatus {
  provider: string;
  enabled: boolean;
  authenticated: boolean;
}

export function Settings({ onBack }: { onBack: () => void }) {
  const [providers, setProviders] = useState<ProviderStatus[]>([]);

  useEffect(() => {
    loadStatus();
  }, []);

  async function loadStatus() {
    try {
        const usage = await invoke<any[]>("get_all_usage");
        setProviders(usage.map(u => ({ 
            provider: u.provider, 
            enabled: u.enabled,
            authenticated: u.authenticated 
        })));
    } catch (e) {
        console.error(e);
    }
  }

  async function toggleProvider(provider: string, enabled: boolean) {
    await invoke("set_provider_enabled", { provider, enabled });
    loadStatus();
  }

  const copilot = providers.find(p => p.provider === "copilot");

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

        {/* Placeholders for others */}
        <div className="provider-config" style={{ opacity: 0.5 }}>
            <span style={{ fontSize: '1.1em', fontWeight: 500 }}>Claude (Coming Soon)</span>
        </div>
      </div>
    </div>
  );
}
