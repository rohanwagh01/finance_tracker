import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import type { Bucket, SpendingFilter } from "../lib/types";
import { money } from "../lib/format";

function Bar({ frac }: { frac: number }) {
  return (
    <span className="hidden h-1.5 w-24 shrink-0 overflow-hidden rounded-full bg-[var(--border)] sm:block">
      <span
        className="block h-full rounded-full bg-[var(--accent)]"
        style={{ width: `${Math.max(2, Math.min(100, frac * 100))}%` }}
      />
    </span>
  );
}

function Node({
  filter,
  id,
  label,
  total,
  count,
  leaf,
  maxTotal,
  depth,
}: {
  filter: SpendingFilter;
  id: string;
  label: string;
  total: number;
  count: number;
  leaf: boolean;
  maxTotal: number;
  depth: number;
}) {
  const [open, setOpen] = useState(false);

  const children = useQuery({
    enabled: open && !leaf,
    queryKey: ["spending-children", filter.from, filter.to, id],
    queryFn: () => api.spendingChildren(filter, id),
  });
  const txns = useQuery({
    enabled: open && leaf,
    queryKey: ["spending-txns", filter.from, filter.to, id],
    queryFn: () => api.listTransactions(filter, { merchant: id, limit: 100 }),
  });

  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-black/5 dark:hover:bg-white/5"
        style={{ paddingLeft: 8 + depth * 16 }}
      >
        <span className="w-3 shrink-0 text-[var(--muted)]">
          {leaf ? "·" : open ? "▾" : "▸"}
        </span>
        <span className="flex-1 truncate">{label}</span>
        <span className="shrink-0 text-xs text-[var(--muted)]">{count}</span>
        <Bar frac={maxTotal > 0 ? total / maxTotal : 0} />
        <span className="w-20 shrink-0 text-right font-medium">{money(total)}</span>
      </button>

      {open && !leaf && (
        <div>
          {children.isLoading && (
            <div
              className="px-2 py-1 text-xs text-[var(--muted)]"
              style={{ paddingLeft: 8 + (depth + 1) * 16 }}
            >
              Loading…
            </div>
          )}
          {children.data?.map((c) => {
            const cmax = Math.max(...(children.data ?? []).map((x) => x.total), 1);
            return (
              <Node
                key={c.id}
                filter={filter}
                id={c.id}
                label={c.label}
                total={c.total}
                count={c.count}
                leaf={c.is_leaf}
                maxTotal={cmax}
                depth={depth + 1}
              />
            );
          })}
        </div>
      )}

      {open && leaf && (
        <div style={{ paddingLeft: 8 + (depth + 1) * 16 }}>
          {txns.isLoading && (
            <div className="px-2 py-1 text-xs text-[var(--muted)]">Loading…</div>
          )}
          {txns.data?.rows.map((t) => (
            <div
              key={t.id}
              className="flex items-center gap-3 px-2 py-1 text-xs text-[var(--muted)]"
            >
              <span className="w-20 shrink-0 tabular-nums">{t.posted_date}</span>
              <span className="flex-1 truncate text-[var(--fg)]">
                {t.description}
                {t.pending && " (pending)"}
              </span>
              <span className="shrink-0">{t.account_name}</span>
              <span className="w-20 shrink-0 text-right font-medium text-[var(--fg)]">
                {money(t.amount, t.currency)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ExpandableBreakdown({
  filter,
  buckets,
}: {
  filter: SpendingFilter;
  buckets: Bucket[];
}) {
  if (buckets.length === 0) {
    return (
      <p className="px-2 py-4 text-sm text-[var(--muted)]">
        No spending in this range.
      </p>
    );
  }
  const max = Math.max(...buckets.map((b) => b.total), 1);
  return (
    <div className="divide-y divide-[var(--border)]">
      {buckets.map((b) => (
        <Node
          key={b.id}
          filter={filter}
          id={b.id}
          label={b.label}
          total={b.total}
          count={b.count}
          leaf={b.id === "UNCATEGORIZED"}
          maxTotal={max}
          depth={0}
        />
      ))}
    </div>
  );
}
