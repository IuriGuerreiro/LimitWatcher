// src/components/providers/CopilotAuth.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

interface AuthFlow {
  url: string;
  user_code: string | null;
  instructions: string;
  poll_interval: number | null;
}

export function CopilotAuth({ onComplete }: { onComplete: () => void }) {
  const [authFlow, setAuthFlow] = useState<AuthFlow | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function startAuth() {
    setLoading(true);
    setError(null);
    
    try {
      const flow = await invoke<AuthFlow>("start_provider_auth", { provider: "copilot" });
      setAuthFlow(flow);
      
      // Open URL in browser
      if (flow?.url) {
        await open(flow.url);
      }
      
      // Start polling (backend handles this)
      await invoke("complete_provider_auth", { 
        provider: "copilot",
        response: { DeviceFlowComplete: null }
      });
      
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
      setAuthFlow(null);
    }
  }

  return (
    <div className="auth-panel">
      <h3 style={{ margin: '0 0 10px 0', fontSize: '1em' }}>Authentication</h3>
      
      {error && <p className="error" style={{ color: 'red' }}>{error}</p>}
      
      {authFlow ? (
        <div className="device-flow">
          <p>Enter this code at GitHub:</p>
          <code 
            className="user-code" 
            onClick={() => {if (authFlow.user_code) navigator.clipboard.writeText(authFlow.user_code)}}
            title="Click to copy"
          >
            {authFlow.user_code}
          </code>
          <p className="instructions">{authFlow.instructions}</p>
          <p className="waiting">Waiting for authorization...</p>
        </div>
      ) : (
        <button 
          onClick={startAuth} 
          disabled={loading}
          className="primary"
          style={{ width: '100%' }}
        >
          {loading ? "Connecting..." : "Connect GitHub Account"}
        </button>
      )}
    </div>
  );
}
