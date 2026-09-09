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
import CategoryTrendChart from "../components/CategoryTrendChart";

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
  const [personId, setPersonId] = useState<string>("");
  const [chart, setChart] = useState<"total" | "category">("total");

  const accounts = useQuery({ queryKey: ["accounts"], queryFn: api.listAccounts });
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });

  const isExcluded = personId === "__excluded__";
  const filter: SpendingFilter | null = range
    ? {
        from: range.from,
        to: range.to,
        account_ids: accountIds.length ? accountIds : null,
        person_id: isExcluded ? null : personId || null,
        excluded_only: isExcluded ? true : null,
      }
    : null;

  const byPerson = useQuery({
    enabled: !!filter && !personId && !isExcluded && (people.data?.length ?? 0) > 1,
    queryKey: ["spending-by-person", filter?.from, filter?.to, accountIds.join(",")],
    queryFn: () => api.spendingByPerson(filter!),
  });

  const trends = useQuery({
    enabled: !!filter && chart === "category",
    queryKey: [
      "spending-trends",
      filter?.from,
      filter?.to,
      accountIds.join(","),
      personId,
    ],
    queryFn: () => api.spendingTrends(filter!),
  });

  const summary = useQuery({
    enabled: !!filter,
    queryKey: [
      "spending-summary",
      filter?.from,
      filter?.to,
      accountIds.join(","),
      personId,
    ],
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
    for (const k of [
      "spending-summary",
      "spending-children",
      "spending-txns",
      "spending-by-person",
      "transactions-table",
      "spending-trends",
      "review-count",
      "review-inbox",
      // reassigning a shared-card charge to/from yourself changes net worth
      "net-worth-now",
      "net-worth-history",
    ]) {
      qc.invalidateQueries({ queryKey: [k] });
    }
  };

  return (
    <>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">Spending</h1>
        <div className="flex flex-wrap items-center gap-3">
          <select
            value={personId}
            onChange={(e) => setPersonId(e.target.value)}
            className="rounded-lg border border-[var(--border)] bg-[var(--bg)] px-2.5 py-1 text-xs"
          >
            <option value="">All</option>
            {people.data?.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
                {p.is_self ? " (you)" : ""}
              </option>
            ))}
            <option value="__excluded__">Excluded</option>
          </select>
          <RangePicker onChange={setRange} />
        </div>
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

      {summary.data && summary.data.txn_count === 0 && (
        <div className="rounded-xl border border-dashed border-[var(--border)] bg-[var(--card)] p-10 text-center">
          <h2 className="text-base font-semibold">No spending in this range yet</h2>
          <p className="mx-auto mt-2 max-w-md text-sm text-[var(--muted)]">
            Link your bank and cards on the <strong>Accounts</strong> page and run
            a sync. Transactions will show up here once Plaid has them — widen the
            date range if you just linked.
          </p>
        </div>
      )}

      {summary.data && summary.data.txn_count > 0 && (
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

          {!personId && !isExcluded && (byPerson.data?.length ?? 0) > 1 && (
            <Card title="By person">
              <p className="mb-2 text-xs text-[var(--muted)]">
                Best-effort — from the review inbox. Everything on non-shared
                accounts counts as yours.
              </p>
              <div className="space-y-1.5">
                {byPerson.data?.map((p) => {
                  const max = Math.max(
                    ...(byPerson.data ?? []).map((x) => x.total),
                    1,
                  );
                  return (
                    <button
                      key={p.person_id}
                      onClick={() => setPersonId(p.person_id)}
                      className="flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-sm hover:bg-black/5 dark:hover:bg-white/5"
                    >
                      <span className="w-28 shrink-0 truncate text-left">
                        {p.person_name}
                        {p.is_self ? " (you)" : ""}
                      </span>
                      <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-[var(--border)]">
                        <span
                          className="block h-full rounded-full bg-[var(--accent)]"
                          style={{ width: `${(p.total / max) * 100}%` }}
                        />
                      </span>
                      <span className="w-20 shrink-0 text-right font-medium">
                        {money(p.total)}
                      </span>
                    </button>
                  );
                })}
              </div>
            </Card>
          )}

          {summary.data.by_month.length > 0 && (
            <Card
              title={
                <div className="flex gap-1.5">
                  {(["total", "category"] as const).map((v) => (
                    <button
                      key={v}
                      onClick={() => setChart(v)}
                      className={`rounded-md px-2 py-1 text-xs ${
                        chart === v
                          ? "bg-[var(--accent)] text-white"
                          : "text-[var(--muted)]"
                      }`}
                    >
                      {v === "total" ? "Monthly total" : "By category"}
                    </button>
                  ))}
                </div>
              }
            >
              {chart === "total" ? (
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
              ) : trends.data ? (
                <CategoryTrendChart trends={trends.data} />
              ) : (
                <p className="py-8 text-center text-xs text-[var(--muted)]">Loading…</p>
              )}
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
