import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { HistoryPoint } from "../lib/types";
import { money } from "../lib/format";

const GID = "vac";

export default function ValueAreaChart({
  data,
  height = 200,
  color = "var(--accent)",
}: {
  data: HistoryPoint[];
  height?: number;
  color?: string;
}) {
  if (data.length < 2) {
    return (
      <p className="py-6 text-center text-xs text-[var(--muted)]">
        Not enough history yet — this fills in as you sync.
      </p>
    );
  }
  return (
    <div style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data}>
          <defs>
            <linearGradient id={GID} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity={0.35} />
              <stop offset="100%" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
          <XAxis
            dataKey="date"
            tick={{ fontSize: 11, fill: "var(--muted)" }}
            stroke="var(--border)"
            minTickGap={40}
          />
          <YAxis
            tick={{ fontSize: 11, fill: "var(--muted)" }}
            stroke="var(--border)"
            width={62}
            domain={["auto", "auto"]}
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
          <Area
            type="monotone"
            dataKey="value"
            stroke={color}
            strokeWidth={2}
            fill={`url(#${GID})`}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
