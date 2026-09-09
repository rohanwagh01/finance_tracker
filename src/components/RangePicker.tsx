import { useEffect, useState } from "react";

export type Range = { from: string; to: string };

const PRESETS = ["1M", "3M", "6M", "12M", "YTD"] as const;
type Preset = (typeof PRESETS)[number] | "Custom";

function iso(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function presetRange(p: Preset): Range {
  const now = new Date();
  const to = iso(now);
  if (p === "YTD") return { from: `${now.getFullYear()}-01-01`, to };
  const months =
    ({ "1M": 1, "3M": 3, "6M": 6, "12M": 12 } as Record<string, number>)[p] ?? 3;
  const from = new Date(now);
  from.setMonth(from.getMonth() - months);
  return { from: iso(from), to };
}

const KEY = "range-picker";

export default function RangePicker({
  onChange,
}: {
  onChange: (r: Range) => void;
}) {
  const [preset, setPreset] = useState<Preset>(() => {
    try {
      return (localStorage.getItem(KEY) as Preset) || "3M";
    } catch {
      return "3M";
    }
  });
  const [custom, setCustom] = useState<Range>(() => presetRange("3M"));

  useEffect(() => {
    try {
      localStorage.setItem(KEY, preset);
    } catch {
      /* ignore */
    }
    onChange(preset === "Custom" ? custom : presetRange(preset));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preset, custom]);

  return (
    <div className="flex flex-wrap items-center gap-2">
      {PRESETS.map((p) => (
        <button
          key={p}
          onClick={() => setPreset(p)}
          className={`rounded-lg px-2.5 py-1 text-xs transition ${
            preset === p
              ? "bg-[var(--accent)] text-white"
              : "border border-[var(--border)] hover:bg-black/5 dark:hover:bg-white/5"
          }`}
        >
          {p}
        </button>
      ))}
      <button
        onClick={() => setPreset("Custom")}
        className={`rounded-lg px-2.5 py-1 text-xs transition ${
          preset === "Custom"
            ? "bg-[var(--accent)] text-white"
            : "border border-[var(--border)] hover:bg-black/5 dark:hover:bg-white/5"
        }`}
      >
        Custom
      </button>
      {preset === "Custom" && (
        <span className="flex items-center gap-1.5 text-xs">
          <input
            type="date"
            value={custom.from}
            onChange={(e) => setCustom({ ...custom, from: e.target.value })}
            className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
          />
          <span className="text-[var(--muted)]">→</span>
          <input
            type="date"
            value={custom.to}
            onChange={(e) => setCustom({ ...custom, to: e.target.value })}
            className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
          />
        </span>
      )}
    </div>
  );
}
