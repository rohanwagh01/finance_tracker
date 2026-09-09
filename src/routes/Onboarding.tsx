import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import { Banner, Button, Select, TextField } from "../components/ui";

type Props = { onDone: () => void };

const STEPS = ["Welcome", "Plaid", "SnapTrade", "Research", "Done"] as const;

export default function Onboarding({ onDone }: Props) {
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Plaid
  const [plaidId, setPlaidId] = useState("");
  const [plaidSecret, setPlaidSecret] = useState("");
  const [plaidEnv, setPlaidEnv] = useState("sandbox");
  // SnapTrade
  const [stId, setStId] = useState("");
  const [stKey, setStKey] = useState("");
  // Research
  const [llm, setLlm] = useState("none");
  const [anthropicKey, setAnthropicKey] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [finnhubKey, setFinnhubKey] = useState("");

  const guard = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const next = () => setStep((s) => Math.min(s + 1, STEPS.length - 1));
  const back = () => setStep((s) => Math.max(s - 1, 0));

  const saveIf = (name: string, v: string) =>
    v.trim() ? api.saveCredential(name, v.trim()) : Promise.resolve();

  const savePlaid = () =>
    guard(async () => {
      if (!plaidId.trim() || !plaidSecret.trim()) {
        throw new Error("Both Plaid fields are required.");
      }
      await api.saveCredential("plaid.client_id", plaidId.trim());
      await api.saveCredential("plaid.secret", plaidSecret.trim());
      await api.updateSettings({
        plaid_env: plaidEnv,
        llm_provider: "none",
        anthropic_model: "claude-sonnet-5",
        ollama_url: ollamaUrl,
        ollama_model: "llama3.1",
        news_provider: "finnhub",
        auto_confirm_high_confidence: false,
      });
      next();
    });

  const testPlaid = () =>
    guard(async () => {
      setTestResult(null);
      await api.saveCredential("plaid.client_id", plaidId.trim());
      await api.saveCredential("plaid.secret", plaidSecret.trim());
      await api.updateSettings({
        plaid_env: plaidEnv,
        llm_provider: "none",
        anthropic_model: "claude-sonnet-5",
        ollama_url: ollamaUrl,
        ollama_model: "llama3.1",
        news_provider: "finnhub",
        auto_confirm_high_confidence: false,
      });
      await api.testPlaidConnection();
      setTestResult("Credentials valid.");
    });

  const saveSnapTrade = () =>
    guard(async () => {
      await saveIf("snaptrade.client_id", stId);
      await saveIf("snaptrade.consumer_key", stKey);
      next();
    });

  const saveResearch = () =>
    guard(async () => {
      if (llm === "anthropic") await saveIf("anthropic.api_key", anthropicKey);
      await saveIf("finnhub.api_key", finnhubKey);
      await api.updateSettings({
        plaid_env: plaidEnv,
        llm_provider: llm as "none" | "anthropic" | "ollama",
        anthropic_model: "claude-sonnet-5",
        ollama_url: ollamaUrl,
        ollama_model: "llama3.1",
        news_provider: finnhubKey.trim() ? "finnhub" : "none",
        auto_confirm_high_confidence: false,
      });
      next();
    });

  const finish = () => guard(async () => {
    await api.completeOnboarding();
    onDone();
  });

  return (
    <div className="mx-auto flex min-h-full max-w-xl flex-col justify-center p-8">
      <div className="mb-6 flex gap-1.5">
        {STEPS.map((_, i) => (
          <div
            key={i}
            className={`h-1 flex-1 rounded-full ${
              i <= step ? "bg-[var(--accent)]" : "bg-[var(--border)]"
            }`}
          />
        ))}
      </div>

      {step === 0 && (
        <div className="space-y-4">
          <h1 className="text-2xl font-semibold">Finance Tracker</h1>
          <p className="text-sm text-[var(--muted)]">
            Everything runs on this machine. The app only reaches the internet to
            pull <strong>read-only</strong> account data (Plaid, SnapTrade), fetch
            market news, and — if you enable it — ask an LLM about your holdings.
            Balances and transaction amounts never leave your computer. The
            research feature only ever sends tickers and rounded allocation
            percentages.
          </p>
          <p className="text-sm text-[var(--muted)]">
            You'll need a Plaid account (free Trial plan works). SnapTrade and the
            research LLM are optional and can be added later in Settings.
          </p>
          <Button onClick={next}>Get started</Button>
        </div>
      )}

      {step === 1 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">Connect Plaid</h1>
          <p className="text-sm text-[var(--muted)]">
            From the Plaid Dashboard → Developers → Keys.
          </p>
          <TextField
            label="Client ID"
            value={plaidId}
            onChange={(e) => setPlaidId(e.target.value)}
          />
          <TextField
            label="Secret"
            type="password"
            value={plaidSecret}
            onChange={(e) => setPlaidSecret(e.target.value)}
          />
          <Select label="Environment" value={plaidEnv} onChange={setPlaidEnv}>
            <option value="sandbox">Sandbox (test data)</option>
            <option value="production">Production (real accounts)</option>
          </Select>
          <div className="flex items-center gap-3">
            <Button variant="ghost" disabled={busy} onClick={testPlaid}>
              Test
            </Button>
            {testResult && (
              <span className="text-sm text-green-600 dark:text-green-400">
                {testResult}
              </span>
            )}
          </div>
          {error && <Banner tone="error">{error}</Banner>}
          <div className="flex justify-between pt-2">
            <Button variant="ghost" onClick={back}>
              Back
            </Button>
            <Button disabled={busy} onClick={savePlaid}>
              Continue
            </Button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">Connect SnapTrade (optional)</h1>
          <p className="text-sm text-[var(--muted)]">
            For Robinhood and E*Trade holdings. From the SnapTrade dashboard.
            Skip if you don't track investments yet.
          </p>
          <TextField
            label="Client ID"
            value={stId}
            onChange={(e) => setStId(e.target.value)}
          />
          <TextField
            label="Consumer Key"
            type="password"
            value={stKey}
            onChange={(e) => setStKey(e.target.value)}
          />
          {error && <Banner tone="error">{error}</Banner>}
          <div className="flex justify-between pt-2">
            <Button variant="ghost" onClick={back}>
              Back
            </Button>
            <div className="flex gap-2">
              <Button variant="ghost" onClick={next}>
                Skip
              </Button>
              <Button disabled={busy} onClick={saveSnapTrade}>
                Continue
              </Button>
            </div>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">Research (optional)</h1>
          <Select label="LLM provider" value={llm} onChange={setLlm}>
            <option value="none">None</option>
            <option value="anthropic">Claude API</option>
            <option value="ollama">Ollama (local, nothing leaves machine)</option>
          </Select>
          {llm === "anthropic" && (
            <TextField
              label="Anthropic API key"
              type="password"
              value={anthropicKey}
              onChange={(e) => setAnthropicKey(e.target.value)}
            />
          )}
          {llm === "ollama" && (
            <TextField
              label="Ollama URL"
              value={ollamaUrl}
              onChange={(e) => setOllamaUrl(e.target.value)}
            />
          )}
          <TextField
            label="Finnhub API key (market news, optional)"
            type="password"
            value={finnhubKey}
            onChange={(e) => setFinnhubKey(e.target.value)}
          />
          {error && <Banner tone="error">{error}</Banner>}
          <div className="flex justify-between pt-2">
            <Button variant="ghost" onClick={back}>
              Back
            </Button>
            <div className="flex gap-2">
              <Button variant="ghost" onClick={next}>
                Skip
              </Button>
              <Button disabled={busy} onClick={saveResearch}>
                Continue
              </Button>
            </div>
          </div>
        </div>
      )}

      {step === 4 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">You're set</h1>
          <p className="text-sm text-[var(--muted)]">
            Credentials are stored in your keychain. Next milestones add the Plaid
            Link and SnapTrade connection flows so you can link accounts.
          </p>
          {error && <Banner tone="error">{error}</Banner>}
          <Button disabled={busy} onClick={finish}>
            Open the app
          </Button>
        </div>
      )}
    </div>
  );
}
