import type { PortfolioPosition } from "../lib/types";
import { money } from "../lib/format";

function Gain({ v, pct }: { v: number | null; pct: number | null }) {
  if (v == null) return <span className="text-[var(--muted)]">—</span>;
  const up = v >= 0;
  return (
    <span className={up ? "text-green-600 dark:text-green-400" : "text-red-500"}>
      {up ? "+" : "−"}
      {money(Math.abs(v))}
      {pct != null && (
        <span className="text-xs"> ({up ? "+" : "−"}{Math.abs(pct).toFixed(1)}%)</span>
      )}
    </span>
  );
}

export default function HoldingsTable({
  positions,
}: {
  positions: PortfolioPosition[];
}) {
  if (positions.length === 0) {
    return (
      <p className="py-4 text-sm text-[var(--muted)]">No positions.</p>
    );
  }
  const max = Math.max(...positions.map((p) => p.allocation_pct), 1);

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-xs text-[var(--muted)]">
            <th className="py-1.5 pr-3 font-medium">Symbol</th>
            <th className="py-1.5 pr-3 font-medium">Qty</th>
            <th className="py-1.5 pr-3 font-medium">Price</th>
            <th className="py-1.5 pr-3 font-medium">Value</th>
            <th className="py-1.5 pr-3 font-medium">Cost basis</th>
            <th className="py-1.5 pr-3 font-medium">Gain / loss</th>
            <th className="py-1.5 pl-3 text-right font-medium">Allocation</th>
          </tr>
        </thead>
        <tbody>
          {positions.map((p, i) => (
            <tr key={(p.ticker ?? p.name ?? "") + i} className="border-t border-[var(--border)]">
              <td className="py-1.5 pr-3">
                <span className="font-medium">{p.ticker ?? "—"}</span>
                {p.name && (
                  <span className="ml-2 hidden text-xs text-[var(--muted)] md:inline">
                    {p.name.length > 32 ? p.name.slice(0, 32) + "…" : p.name}
                  </span>
                )}
              </td>
              <td className="py-1.5 pr-3 tabular-nums text-[var(--muted)]">
                {p.quantity.toLocaleString(undefined, { maximumFractionDigits: 4 })}
              </td>
              <td className="py-1.5 pr-3 tabular-nums text-[var(--muted)]">
                {money(p.price)}
              </td>
              <td className="py-1.5 pr-3 font-medium tabular-nums">
                {money(p.value)}
              </td>
              <td className="py-1.5 pr-3 tabular-nums text-[var(--muted)]">
                {money(p.cost_basis)}
              </td>
              <td className="py-1.5 pr-3 tabular-nums">
                <Gain v={p.gain} pct={p.gain_pct} />
              </td>
              <td className="py-1.5 pl-3">
                <div className="flex items-center justify-end gap-2">
                  <span className="hidden h-1.5 w-16 overflow-hidden rounded-full bg-[var(--border)] sm:block">
                    <span
                      className="block h-full rounded-full bg-[var(--accent)]"
                      style={{ width: `${(p.allocation_pct / max) * 100}%` }}
                    />
                  </span>
                  <span className="w-12 text-right tabular-nums">
                    {p.allocation_pct.toFixed(1)}%
                  </span>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
