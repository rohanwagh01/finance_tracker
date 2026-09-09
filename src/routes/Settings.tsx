import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import { CREDENTIAL_LABELS, type Settings } from "../lib/types";
import {
  Banner,
  Button,
  Card,
  Select,
  TextField,
  Toggle,
} from "../components/ui";
import PeopleManager from "../components/PeopleManager";
import SecurityCard from "../components/SecurityCard";

function CredentialField({
  name,
  present,
  onChanged,
}: {
  name: string;
  present: boolean;
  onChanged: () => void;
}) {
  const [value, setValue] = useState("");
  const save = useMutation({
    mutationFn: () => api.saveCredential(name, value),
    onSuccess: () => {
      setValue("");
      onChanged();
    },
  });
  const remove = useMutation({
    mutationFn: () => api.deleteCredential(name),
    onSuccess: onChanged,
  });

  return (
    <div className="flex items-end gap-2">
      <TextField
        label={CREDENTIAL_LABELS[name] ?? name}
        type="password"
        placeholder={present ? "•••••••• (stored)" : "not set"}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        className="flex-1"
      />
      <Button
        variant="ghost"
        disabled={!value || save.isPending}
        onClick={() => save.mutate()}
      >
        Save
      </Button>
      {present && (
        <Button
          variant="ghost"
          disabled={remove.isPending}
          onClick={() => remove.mutate()}
        >
          Clear
        </Button>
      )}
    </div>
  );
}

const CRED_GROUPS: { title: string; names: string[] }[] = [
  {
    title: "Plaid — banks, cards & brokerages",
    names: ["plaid.client_id", "plaid.secret"],
  },
  {
    title: "Research — LLM & news (optional)",
    names: ["anthropic.api_key", "finnhub.api_key", "marketaux.api_key"],
  },
];

export default function SettingsPage() {
  const qc = useQueryClient();
  const creds = useQuery({
    queryKey: ["credentials"],
    queryFn: api.listCredentials,
  });
  const settingsQuery = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });

  const [form, setForm] = useState<Settings | null>(null);
  useEffect(() => {
    if (settingsQuery.data) setForm(settingsQuery.data);
  }, [settingsQuery.data]);

  const saveSettings = useMutation({
    mutationFn: (s: Settings) => api.updateSettings(s),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["settings"] });
      qc.invalidateQueries({ queryKey: ["setup-status"] });
    },
  });

  const testPlaid = useMutation({ mutationFn: api.testPlaidConnection });

  const refreshCreds = () => {
    qc.invalidateQueries({ queryKey: ["credentials"] });
    qc.invalidateQueries({ queryKey: ["setup-status"] });
  };

  const present = (n: string) =>
    creds.data?.find((c) => c.name === n)?.present ?? false;

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold">Settings</h1>

      <div className="space-y-5">
        <SecurityCard />

        <Card title="API credentials">
          <p className="mb-4 text-xs text-[var(--muted)]">
            Stored inside the encrypted database — never in a plaintext file, never
            sent anywhere except the matching provider.
          </p>
          <div className="space-y-5">
            {CRED_GROUPS.map((g) => (
              <div key={g.title}>
                <h3 className="mb-2 text-xs font-semibold text-[var(--muted)]">
                  {g.title}
                </h3>
                <div className="space-y-3">
                  {g.names.map((n) => (
                    <CredentialField
                      key={n}
                      name={n}
                      present={present(n)}
                      onChanged={refreshCreds}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>

          <div className="mt-5 flex flex-wrap items-center gap-3">
            <Button
              variant="ghost"
              disabled={testPlaid.isPending}
              onClick={() => testPlaid.mutate()}
            >
              {testPlaid.isPending ? "Testing…" : "Test Plaid connection"}
            </Button>
            {testPlaid.isSuccess && (
              <span className="text-sm text-green-600 dark:text-green-400">
                Credentials valid.
              </span>
            )}
            {testPlaid.isError && (
              <span className="text-sm text-red-500">
                {errorMessage(testPlaid.error)}
              </span>
            )}
          </div>
        </Card>

        {form && (
          <Card
            title="Preferences"
            actions={
              <Button
                disabled={saveSettings.isPending}
                onClick={() => saveSettings.mutate(form)}
              >
                {saveSettings.isPending ? "Saving…" : "Save"}
              </Button>
            }
          >
            <div className="grid gap-4 md:grid-cols-2">
              <Select
                label="Plaid environment"
                value={form.plaid_env}
                onChange={(v) => setForm({ ...form, plaid_env: v })}
              >
                <option value="sandbox">Sandbox (test data)</option>
                <option value="production">Production (real accounts)</option>
              </Select>

              <TextField
                label="Pull history since"
                type="date"
                value={form.history_start_date}
                onChange={(e) =>
                  setForm({ ...form, history_start_date: e.target.value })
                }
                hint="Applies to newly linked institutions. Plaid caps history at ~2 years."
              />

              <Select
                label="Research LLM"
                value={form.llm_provider}
                onChange={(v) =>
                  setForm({ ...form, llm_provider: v as Settings["llm_provider"] })
                }
              >
                <option value="none">None</option>
                <option value="anthropic">Claude API</option>
                <option value="ollama">Ollama (local)</option>
              </Select>

              {form.llm_provider === "anthropic" && (
                <TextField
                  label="Anthropic model"
                  value={form.anthropic_model}
                  onChange={(e) =>
                    setForm({ ...form, anthropic_model: e.target.value })
                  }
                />
              )}

              {form.llm_provider === "ollama" && (
                <>
                  <TextField
                    label="Ollama URL"
                    value={form.ollama_url}
                    onChange={(e) =>
                      setForm({ ...form, ollama_url: e.target.value })
                    }
                  />
                  <TextField
                    label="Ollama model"
                    value={form.ollama_model}
                    onChange={(e) =>
                      setForm({ ...form, ollama_model: e.target.value })
                    }
                  />
                </>
              )}

              <Select
                label="News provider"
                value={form.news_provider}
                onChange={(v) =>
                  setForm({
                    ...form,
                    news_provider: v as Settings["news_provider"],
                  })
                }
              >
                <option value="finnhub">Finnhub</option>
                <option value="marketaux">Marketaux</option>
                <option value="none">None</option>
              </Select>

              <div className="pt-1">
                <Toggle
                  label="Let the research LLM see gain/loss %"
                  checked={form.research_share_gains}
                  onChange={(v) =>
                    setForm({ ...form, research_share_gains: v })
                  }
                />
                <p className="mt-1 text-xs text-[var(--muted)]">
                  Adds each holding's rounded gain/loss percent vs. cost basis to
                  what's sent for analysis. Still never sends dollar amounts,
                  balances, or share counts.
                </p>
              </div>
            </div>

            <div className="mt-4">
              <Toggle
                label="Auto-confirm high-confidence attribution rules (skip the review inbox for those)"
                checked={form.auto_confirm_high_confidence}
                onChange={(v) =>
                  setForm({ ...form, auto_confirm_high_confidence: v })
                }
              />
            </div>

            {saveSettings.isError && (
              <div className="mt-3">
                <Banner tone="error">
                  {errorMessage(saveSettings.error)}
                </Banner>
              </div>
            )}
          </Card>
        )}

        <Card title="People">
          <p className="mb-4 text-xs text-[var(--muted)]">
            Used to attribute charges on shared cards. Capital One doesn't expose
            per-cardholder data, so attribution is manual via the review inbox.
          </p>
          <PeopleManager />
        </Card>
      </div>
    </>
  );
}
