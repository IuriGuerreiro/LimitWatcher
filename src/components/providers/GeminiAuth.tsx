// src/components/providers/GeminiAuth.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

interface AuthFlow {
  url: string;
  user_code: string | null;
  instructions: string;
  poll_interval: number | null;
}

interface AuthStatus {
  user?: string;
  plan?: string;
  expires?: string;
}

export function GeminiAuth({
  onComplete,
  authStatus
}: {
  onComplete: () => void;
  authStatus?: AuthStatus;
}) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [instructions, setInstructions] = useState<AuthFlow | null>(null);

  async function startAuth() {
    setLoading(true);
    setError(null);

    try {
      const flow = await invoke<AuthFlow>("start_provider_auth", { provider: "gemini" });
      setInstructions(flow);

      // Open URL in browser
      if (flow?.url) {
        await open(flow.url);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function checkCredentials() {
    setLoading(true);
    setError(null);

    try {
      await invoke("complete_provider_auth", {
        provider: "gemini",
        response: { DeviceFlowComplete: null }
      });
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  // If already authenticated, show status
  if (authStatus?.user) {
    return (
      <div className="auth-panel">
        <div style={{
          padding: '10px',
          background: '#e8f5e9',
          color: '#2e7d32',
          borderRadius: '6px',
          marginBottom: '10px'
        }}>
          <div style={{ fontWeight: 500, marginBottom: '4px' }}>
            ✓ {authStatus.user}
          </div>
          {authStatus.plan && (
            <div style={{
              fontSize: '0.85em',
              opacity: 0.9,
              display: 'inline-block',
              background: '#c8e6c9',
              padding: '2px 8px',
              borderRadius: '4px',
              marginTop: '4px'
            }}>
              Plan: {authStatus.plan}
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="auth-panel">
      <h3 style={{ margin: '0 0 10px 0', fontSize: '1em' }}>Authentication</h3>

      {error && <p className="error" style={{ color: 'red' }}>{error}</p>}

      {instructions ? (
        <div className="cli-auth">
          <p style={{ whiteSpace: 'pre-line', marginBottom: '10px' }}>{instructions.instructions}</p>
          <button
            onClick={checkCredentials}
            disabled={loading}
            className="primary"
            style={{ width: '100%' }}
          >
            {loading ? "Checking..." : "Check for credentials"}
          </button>
        </div>
      ) : (
        <button
          onClick={startAuth}
          disabled={loading}
          className="primary"
          style={{ width: '100%' }}
        >
          {loading ? "Loading..." : "Setup Gemini CLI"}
        </button>
      )}
    </div>
  );
}
