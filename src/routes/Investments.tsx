import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { api, errorMessage } from "../lib/api";
import { money, relativeTime } from "../lib/format";
import { Banner, Button, Card } from "../components/ui";
import HoldingsTable from "../components/HoldingsTable";
import ValueAreaChart from "../components/ValueAreaChart";

function twoYearsAgo() {
  const d = new Date();
  d.setDate(d.getDate() - 730);
  return d.toISOString().slice(0, 10);
}

function Kpi({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--card)] p-4">
      <div className="text-xs text-[var(--muted)]">{label}</div>
      <div className="mt-1 text-xl font-semibold">{value}</div>
      {sub && <div className="mt-0.5 text-xs text-[var(--muted)]">{sub}</div>}
    </div>
  );
}

export default function Investments() {
  const qc = useQueryClient();
  const port = useQuery({ queryKey: ["portfolio"], queryFn: api.portfolio });
  const hist = useQuery({
    queryKey: ["portfolio-history"],
    queryFn: () => api.portfolioHistory(twoYearsAgo(), new Date().toISOString().slice(0, 10)),
  });
  const [view, setView] = useState<"combined" | "accounts">("combined");
  const [msg, setMsg] = useState<string | null>(null);

  const sync = useMutation({
    mutationFn: api.syncAll,
    onSuccess: () => {
      setMsg("Synced.");
      qc.invalidateQueries({ queryKey: ["portfolio"] });
      qc.invalidateQueries({ queryKey: ["portfolio-history"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
    onError: (e) => setMsg(errorMessage(e)),
  });

  const p = port.data;
  const hasHoldings = p && (p.accounts.length > 0 || p.combined.length > 0);

  return (
    <>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">Investments</h1>
        <Button variant="ghost" disabled={sync.isPending} onClick={() => sync.mutate()}>
          {sync.isPending ? "Syncing…" : "Sync"}
        </Button>
      </div>

      {msg && (
        <div className="mb-4">
          <Banner tone="info">{msg}</Banner>
        </div>
      )}

      {!hasHoldings && (
        <Banner tone="info">
          No brokerage holdings yet. Link a brokerage (E*Trade, Robinhood, …) on the{" "}
          <Link className="underline" to="/accounts">
            Accounts
          </Link>{" "}
          page through Plaid — investment accounts sync their holdings automatically.
        </Banner>
      )}

      {hasHoldings && (
        <div className="space-y-4">
          <div className="grid grid-cols-3 gap-3">
            <Kpi
              label="Portfolio value"
              value={money(p!.total_value)}
              sub={p!.total_cash > 0 ? `${money(p!.total_cash)} in cash` : undefined}
            />
            <Kpi
              label="Cost basis"
              value={p!.total_cost_basis != null ? money(p!.total_cost_basis) : "—"}
            />
            <Kpi
              label="Total gain / loss"
              value={p!.total_gain != null ? money(p!.total_gain) : "—"}
              sub={
                p!.total_gain != null && p!.total_cost_basis
                  ? `${((p!.total_gain / p!.total_cost_basis) * 100).toFixed(1)}%`
                  : undefined
              }
            />
          </div>

          <Card title="Value over time">
            <ValueAreaChart data={hist.data ?? []} height={200} />
            <p className="mt-1 text-xs text-[var(--muted)]">
              Built from each sync — Plaid has no historical prices, so this
              starts when you first synced and grows over time.
            </p>
          </Card>

          <Card
            title={
              <div className="flex gap-1.5">
                {(["combined", "accounts"] as const).map((v) => (
                  <button
                    key={v}
                    onClick={() => setView(v)}
                    className={`rounded-md px-2 py-1 text-xs ${
                      view === v
                        ? "bg-[var(--accent)] text-white"
                        : "text-[var(--muted)]"
                    }`}
                  >
                    {v === "combined" ? "Combined" : "By account"}
                  </button>
                ))}
              </div>
            }
            actions={
              <span className="text-xs text-[var(--muted)]">
                synced {relativeTime(p!.last_synced_at)}
              </span>
            }
          >
            {view === "combined" ? (
              <HoldingsTable positions={p!.combined} />
            ) : (
              <div className="space-y-6">
                {p!.accounts
                  .filter((a) => a.positions.length > 0 || a.cash > 0)
                  .map((a) => (
                    <div key={a.id}>
                      <div className="mb-1 flex items-center justify-between text-sm">
                        <span className="font-medium">
                          {a.name}
                          {a.mask && (
                            <span className="text-[var(--muted)]"> ••{a.mask}</span>
                          )}
                          {a.official_name && (
                            <span className="text-[var(--muted)]">
                              {" "}· {a.official_name}
                            </span>
                          )}
                        </span>
                        <span className="font-semibold">{money(a.value)}</span>
                      </div>
                      <HoldingsTable positions={a.positions} />
                      {a.cash > 0 && (
                        <div className="mt-1 text-xs text-[var(--muted)]">
                          + {money(a.cash)} cash
                        </div>
                      )}
                    </div>
                  ))}
              </div>
            )}
          </Card>
        </div>
      )}
    </>
  );
}
