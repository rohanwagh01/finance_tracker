import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "../lib/api";
import type { SpendingFilter } from "../lib/types";
import { money } from "../lib/format";
import { Banner, Card } from "../components/ui";
import RangePicker, { type Range } from "../components/RangePicker";
import ExpandableBreakdown from "../components/ExpandableBreakdown";
import TransactionsTable from "../components/TransactionsTable";

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--card)] p-4">
      <div className="text-xs text-[var(--muted)]">{label}</div>
      <div className="mt-1 text-xl font-semibold">{value}</div>
      {sub && <div className="mt-0.5 text-xs text-[var(--muted)]">{sub}</div>}
    </div>
  );
}

export default function Spending() {
  const qc = useQueryClient();
  const [range, setRange] = useState<Range | null>(null);
  const [accountIds, setAccountIds] = useState<string[]>([]);

  const accounts = useQuery({ queryKey: ["accounts"], queryFn: api.listAccounts });

  const filter: SpendingFilter | null = range
    ? {
        from: range.from,
        to: range.to,
        account_ids: accountIds.length ? accountIds : null,
      }
    : null;

  const summary = useQuery({
    enabled: !!filter,
    queryKey: ["spending-summary", filter?.from, filter?.to, accountIds.join(",")],
    queryFn: () => api.spendingSummary(filter!),
  });

  const stats = useMemo(() => {
    const m = summary.data?.by_month ?? [];
    const months = m.length || 1;
    const avg = (summary.data?.total ?? 0) / months;
    const nowKey = new Date().toISOString().slice(0, 7);
    const thisM = m.find((x) => x.month === nowKey)?.total ?? 0;
    const idx = m.findIndex((x) => x.month === nowKey);
    const prevM = idx > 0 ? m[idx - 1].total : null;
    const delta = prevM != null ? thisM - prevM : null;
    return { avg, thisM, delta };
  }, [summary.data]);

  const onRecategorized = () => {
    qc.invalidateQueries({ queryKey: ["spending-summary"] });
    qc.invalidateQueries({ queryKey: ["spending-children"] });
    qc.invalidateQueries({ queryKey: ["spending-txns"] });
    qc.invalidateQueries({ queryKey: ["transactions-table"] });
  };

  return (
    <>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">Spending</h1>
        <RangePicker onChange={setRange} />
      </div>

      {(accounts.data?.length ?? 0) > 1 && (
        <div className="mb-4 flex flex-wrap gap-1.5">
          {accounts.data
            ?.filter((a) => !a.is_hidden)
            .map((a) => {
              const on = accountIds.includes(a.id);
              return (
                <button
                  key={a.id}
                  onClick={() =>
                    setAccountIds((s) =>
                      on ? s.filter((x) => x !== a.id) : [...s, a.id],
                    )
                  }
                  className={`rounded-full px-2.5 py-1 text-xs transition ${
                    on
                      ? "bg-[var(--accent)] text-white"
                      : "border border-[var(--border)]"
                  }`}
                >
                  {a.name}
                </button>
              );
            })}
          {accountIds.length > 0 && (
            <button
              onClick={() => setAccountIds([])}
              className="px-2 py-1 text-xs text-[var(--muted)] underline"
            >
              all accounts
            </button>
          )}
        </div>
      )}

      {summary.isError && (
        <Banner tone="error">Could not load spending.</Banner>
      )}

      {summary.data && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            <Kpi
              label="Total spent"
              value={money(summary.data.total)}
              sub={`${summary.data.txn_count} transactions`}
            />
            <Kpi
              label="Average / month"
              value={money(stats.avg)}
              sub={`${summary.data.by_month.length} month(s)`}
            />
            <Kpi
              label="This month"
              value={money(stats.thisM)}
              sub={
                stats.delta == null
                  ? undefined
                  : `${stats.delta >= 0 ? "▲" : "▼"} ${money(Math.abs(stats.delta))} vs last`
              }
            />
          </div>

          {summary.data.by_month.length > 0 && (
            <Card title="Monthly spending">
              <div className="h-56">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={summary.data.by_month}>
                    <CartesianGrid
                      strokeDasharray="3 3"
                      stroke="var(--border)"
                      vertical={false}
                    />
                    <XAxis
                      dataKey="month"
                      tick={{ fontSize: 11, fill: "var(--muted)" }}
                      stroke="var(--border)"
                    />
                    <YAxis
                      tick={{ fontSize: 11, fill: "var(--muted)" }}
                      stroke="var(--border)"
                      width={54}
                      tickFormatter={(v) => money(Number(v))}
                    />
                    <Tooltip
                      formatter={(v) => money(Number(v))}
                      contentStyle={{
                        background: "var(--card)",
                        border: "1px solid var(--border)",
                        borderRadius: 8,
                        fontSize: 12,
                      }}
                    />
                    <Bar
                      dataKey="total"
                      fill="var(--accent)"
                      radius={[3, 3, 0, 0]}
                      maxBarSize={44}
                    />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </Card>
          )}

          <Card title="Breakdown">
            <ExpandableBreakdown filter={filter!} buckets={summary.data.by_category} />
          </Card>

          <Card title="Transactions">
            <TransactionsTable filter={filter!} onRecategorized={onRecategorized} />
          </Card>
        </div>
      )}
    </>
  );
}
