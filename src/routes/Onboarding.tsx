import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import type { Settings } from "../lib/types";
import {
  Banner,
  Button,
  ExtLink,
  Help,
  Select,
  TextField,
} from "../components/ui";

type Props = { onDone: () => void };

const STEPS = ["Welcome", "Plaid", "Research", "Done"] as const;

function twoYearsAgo(): string {
  const d = new Date();
  d.setDate(d.getDate() - 730);
  return d.toISOString().slice(0, 10);
}

export default function Onboarding({ onDone }: Props) {
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  const [plaidId, setPlaidId] = useState("");
  const [plaidSecret, setPlaidSecret] = useState("");
  const [plaidEnv, setPlaidEnv] = useState("sandbox");
  const [historyStart, setHistoryStart] = useState(twoYearsAgo());

  const [llm, setLlm] = useState("none");
  const [anthropicKey, setAnthropicKey] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [finnhubKey, setFinnhubKey] = useState("");

  const settings = (over: Partial<Settings> = {}): Settings => ({
    plaid_env: plaidEnv,
    history_start_date: historyStart,
    llm_provider: "none",
    anthropic_model: "claude-sonnet-5",
    ollama_url: ollamaUrl,
    ollama_model: "llama3.1",
    news_provider: "finnhub",
    auto_confirm_high_confidence: false,
    research_share_gains: false,
    ...over,
  });

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
      if (!plaidId.trim() || !plaidSecret.trim())
        throw new Error("Both Plaid fields are required.");
      await api.saveCredential("plaid.client_id", plaidId.trim());
      await api.saveCredential("plaid.secret", plaidSecret.trim());
      await api.updateSettings(settings());
      next();
    });

  const testPlaid = () =>
    guard(async () => {
      setTestResult(null);
      await api.saveCredential("plaid.client_id", plaidId.trim());
      await api.saveCredential("plaid.secret", plaidSecret.trim());
      await api.updateSettings(settings());
      await api.testPlaidConnection();
      setTestResult("Credentials valid.");
    });

  const saveResearch = () =>
    guard(async () => {
      if (llm === "anthropic") await saveIf("anthropic.api_key", anthropicKey);
      await saveIf("finnhub.api_key", finnhubKey);
      await api.updateSettings(
        settings({
          llm_provider: llm as Settings["llm_provider"],
          news_provider: finnhubKey.trim() ? "finnhub" : "none",
        }),
      );
      next();
    });

  const finish = () =>
    guard(async () => {
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
            pull <strong>read-only</strong> account data through Plaid, fetch market
            news, and — if you enable it — ask an LLM about your holdings. Balances
            and transaction amounts never leave your computer.
          </p>
          <p className="text-sm text-[var(--muted)]">
            You'll need a free Plaid account. The research LLM and news feed are
            optional and can be added later in Settings. Each step has a{" "}
            <strong>"How do I get this?"</strong> section you can expand for
            click-by-click instructions.
          </p>
          <Button onClick={next}>Get started</Button>
        </div>
      )}

      {step === 1 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">Connect Plaid</h1>
          <p className="text-sm text-[var(--muted)]">
            Plaid is the read-only bridge to your banks and cards. The free plan
            is enough and no card is required to start.
          </p>

          <Help title="Step-by-step: get your Plaid keys (~3 min)">
            <ol className="list-decimal space-y-1.5 pl-4">
              <li>
                Go to{" "}
                <ExtLink href="https://dashboard.plaid.com/signup">
                  dashboard.plaid.com/signup
                </ExtLink>{" "}
                and create a free account. Pick "Personal" use if asked.
              </li>
              <li>
                Verify your email, then open{" "}
                <ExtLink href="https://dashboard.plaid.com/developers/keys">
                  Developers → Keys
                </ExtLink>
                .
              </li>
              <li>
                Copy the <strong>client_id</strong> into the first field below.
              </li>
              <li>
                Start with <strong>Sandbox</strong>: copy the{" "}
                <em>Sandbox</em> secret and leave Environment on "Sandbox". You
                can try the whole app with fake data — at the bank login screen
                use username <code>user_good</code> / password{" "}
                <code>pass_good</code>.
              </li>
              <li>
                When you're ready for real accounts, switch Environment to{" "}
                <strong>Production</strong> here and paste the{" "}
                <em>Production</em> secret instead. Production access is
                self-serve for personal use (up to 100 linked institutions).
              </li>
            </ol>
          </Help>

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
          <TextField
            label="Pull history since"
            type="date"
            value={historyStart}
            onChange={(e) => setHistoryStart(e.target.value)}
            hint="Plaid can go back at most ~2 years. Earlier dates just cap there."
          />
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
          <h1 className="text-xl font-semibold">Research (optional)</h1>
          <p className="text-sm text-[var(--muted)]">
            Adds a market-news feed and an LLM that comments on your holdings.
            Only ticker symbols and rounded allocation percentages are ever sent —
            never balances or amounts. You can skip this and add it later in
            Settings.
          </p>

          <Select label="LLM provider" value={llm} onChange={setLlm}>
            <option value="none">None</option>
            <option value="anthropic">Claude API</option>
            <option value="ollama">Ollama (local, nothing leaves machine)</option>
          </Select>

          <Help title="Which should I pick?">
            <ul className="list-disc space-y-1.5 pl-4">
              <li>
                <strong>Claude API</strong> — best answers, works on any machine.
                Needs an account with a payment method; a research run costs a few
                cents.
              </li>
              <li>
                <strong>Ollama</strong> — free and fully local (nothing leaves
                your computer), but needs a reasonably powerful machine and a
                one-time model download.
              </li>
              <li>
                <strong>None</strong> — skip the LLM. The news feed still works
                with just a Finnhub key.
              </li>
            </ul>
          </Help>

          {llm === "anthropic" && (
            <>
              <TextField
                label="Anthropic API key"
                type="password"
                value={anthropicKey}
                onChange={(e) => setAnthropicKey(e.target.value)}
              />
              <Help title="Step-by-step: get an Anthropic API key">
                <ol className="list-decimal space-y-1.5 pl-4">
                  <li>
                    Sign up at{" "}
                    <ExtLink href="https://console.anthropic.com/">
                      console.anthropic.com
                    </ExtLink>
                    .
                  </li>
                  <li>
                    Open <strong>Billing</strong> and add a small amount of
                    credit (e.g. $5).
                  </li>
                  <li>
                    Open <strong>API Keys → Create Key</strong>, copy it, and
                    paste it above. It starts with <code>sk-ant-</code>.
                  </li>
                </ol>
              </Help>
            </>
          )}

          {llm === "ollama" && (
            <>
              <TextField
                label="Ollama URL"
                value={ollamaUrl}
                onChange={(e) => setOllamaUrl(e.target.value)}
              />
              <Help title="Step-by-step: set up Ollama">
                <ol className="list-decimal space-y-1.5 pl-4">
                  <li>
                    Download and install it from{" "}
                    <ExtLink href="https://ollama.com/download">
                      ollama.com/download
                    </ExtLink>{" "}
                    and open it once so it's running.
                  </li>
                  <li>
                    In a terminal, run <code>ollama pull llama3.1</code> (about
                    5&nbsp;GB). A bigger model like <code>ollama pull qwen2.5:14b</code>{" "}
                    gives better answers if your machine can handle it.
                  </li>
                  <li>
                    Leave the URL below as{" "}
                    <code>http://localhost:11434</code> unless you changed it.
                  </li>
                  <li>
                    If you pulled a different model, set its name in{" "}
                    <strong>Settings</strong> after onboarding.
                  </li>
                </ol>
              </Help>
            </>
          )}

          <TextField
            label="Finnhub API key (market news, optional)"
            type="password"
            value={finnhubKey}
            onChange={(e) => setFinnhubKey(e.target.value)}
          />
          <Help title="Step-by-step: get a free Finnhub key (~1 min)">
            <ol className="list-decimal space-y-1.5 pl-4">
              <li>
                Register at{" "}
                <ExtLink href="https://finnhub.io/register">
                  finnhub.io/register
                </ExtLink>{" "}
                with an email address — no card needed.
              </li>
              <li>
                Your API key is shown on the dashboard right after you log in.
                Copy it above.
              </li>
              <li>
                The free tier covers the per-company news this app uses.
              </li>
            </ol>
          </Help>
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

      {step === 3 && (
        <div className="space-y-4">
          <h1 className="text-xl font-semibold">You're set</h1>
          <p className="text-sm text-[var(--muted)]">
            Credentials are stored inside the encrypted database. Next, link your
            banks and cards on the Accounts page.
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
