import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { SpendingTrends } from "../lib/types";
import { money } from "../lib/format";

// muted categorical palette (7 + Other) — readable in both themes
const COLORS = [
  "#6366f1",
  "#0ea5e9",
  "#10b981",
  "#f59e0b",
  "#ef4444",
  "#a855f7",
  "#14b8a6",
  "#94a3b8",
];

export default function CategoryTrendChart({ trends }: { trends: SpendingTrends }) {
  if (trends.months.length === 0) {
    return <p className="py-4 text-sm text-[var(--muted)]">No spending in range.</p>;
  }

  // recharts wants row-per-month objects
  const rows = trends.months.map((m, i) => {
    const row: Record<string, string | number> = { month: m };
    for (const s of trends.series) row[s.label] = s.values[i] ?? 0;
    return row;
  });

  return (
    <div className="h-64">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={rows}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
          <XAxis
            dataKey="month"
            tick={{ fontSize: 11, fill: "var(--muted)" }}
            stroke="var(--border)"
          />
          <YAxis
            tick={{ fontSize: 11, fill: "var(--muted)" }}
            stroke="var(--border)"
            width={56}
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
          <Legend wrapperStyle={{ fontSize: 11 }} />
          {trends.series.map((s, i) => (
            <Bar
              key={s.id}
              dataKey={s.label}
              stackId="a"
              fill={COLORS[i % COLORS.length]}
              maxBarSize={44}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
