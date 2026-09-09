import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import { money } from "../lib/format";
import { Banner, Button, Card } from "../components/ui";
import ValueAreaChart from "../components/ValueAreaChart";
import RangePicker, { type Range } from "../components/RangePicker";
import type {
  AssetInput,
  LiabilityInput,
  ManualAsset,
  ManualLiability,
  Recurring,
  RecurringInput,
} from "../lib/types";

const ASSET_KINDS = [
  ["vehicle", "Vehicle"],
  ["property", "Property / real estate"],
  ["cash", "Cash held elsewhere"],
  ["other", "Other asset"],
] as const;

const LIABILITY_KINDS = [
  ["mortgage", "Mortgage"],
  ["auto_loan", "Auto loan"],
  ["student_loan", "Student loan"],
  ["personal_loan", "Personal loan"],
  ["other", "Other liability"],
] as const;

const CADENCES = [
  ["weekly", "Weekly"],
  ["biweekly", "Every 2 weeks"],
  ["monthly", "Monthly"],
  ["quarterly", "Quarterly"],
  ["yearly", "Yearly"],
] as const;

function today() {
  return new Date().toISOString().slice(0, 10);
}

function fieldCls(extra = "") {
  return `w-full rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)] ${extra}`;
}

function Labeled({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--muted)]">
        {label}
      </span>
      {children}
    </label>
  );
}

/* ------------------------------------------------------------------ assets */

function AssetForm({
  initial,
  onSubmit,
  onCancel,
  pending,
}: {
  initial?: ManualAsset;
  onSubmit: (v: AssetInput) => void;
  onCancel: () => void;
  pending: boolean;
}) {
  const [name, setName] = useState(initial?.name ?? "");
  const [kind, setKind] = useState(initial?.kind ?? "vehicle");
  const [value, setValue] = useState(initial ? String(initial.value) : "");
  const [asOf, setAsOf] = useState(initial?.as_of ?? today());
  const [dep, setDep] = useState(
    initial?.depreciation_annual_pct != null
      ? String(initial.depreciation_annual_pct)
      : "",
  );
  const [note, setNote] = useState(initial?.note ?? "");

  const valid = name.trim() !== "" && value !== "" && !Number.isNaN(Number(value));

  return (
    <div className="space-y-3 rounded-lg border border-[var(--border)] bg-[var(--bg)] p-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <Labeled label="Name">
          <input
            className={fieldCls()}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="2021 Honda Civic"
          />
        </Labeled>
        <Labeled label="Type">
          <select
            className={fieldCls()}
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            {ASSET_KINDS.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </Labeled>
        <Labeled label="Value">
          <input
            className={fieldCls()}
            type="number"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            placeholder="24000"
          />
        </Labeled>
        <Labeled label="As of">
          <input
            className={fieldCls()}
            type="date"
            value={asOf}
            onChange={(e) => setAsOf(e.target.value)}
          />
        </Labeled>
        <Labeled label="Depreciation % / year (optional)">
          <input
            className={fieldCls()}
            type="number"
            value={dep}
            onChange={(e) => setDep(e.target.value)}
            placeholder="e.g. 10 for a car"
          />
        </Labeled>
        <Labeled label="Note (optional)">
          <input
            className={fieldCls()}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
        </Labeled>
      </div>
      <div className="flex gap-2">
        <Button
          disabled={!valid || pending}
          onClick={() =>
            onSubmit({
              name: name.trim(),
              kind,
              value: Number(value),
              as_of: asOf,
              depreciation_annual_pct: dep === "" ? null : Number(dep),
              note: note.trim() === "" ? null : note.trim(),
            })
          }
        >
          {initial ? "Save" : "Add asset"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

function AssetsCard() {
  const qc = useQueryClient();
  const assets = useQuery({
    queryKey: ["manual-assets"],
    queryFn: api.listManualAssets,
  });
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["manual-assets"] });
    qc.invalidateQueries({ queryKey: ["net-worth-now"] });
    qc.invalidateQueries({ queryKey: ["net-worth-history"] });
    qc.invalidateQueries({ queryKey: ["asset-allocation"] });
  };

  const create = useMutation({
    mutationFn: (v: AssetInput) => api.createManualAsset(v),
    onSuccess: () => {
      setAdding(false);
      invalidate();
    },
  });
  const update = useMutation({
    mutationFn: (v: { id: string; input: AssetInput }) =>
      api.updateManualAsset(v.id, v.input),
    onSuccess: () => {
      setEditing(null);
      invalidate();
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteManualAsset(id),
    onSuccess: invalidate,
  });

  const total = (assets.data ?? []).reduce((s, a) => s + a.current_value, 0);

  return (
    <Card
      title="Manual assets"
      actions={
        !adding && (
          <Button variant="ghost" onClick={() => setAdding(true)}>
            Add
          </Button>
        )
      }
    >
      {adding && (
        <div className="mb-3">
          <AssetForm
            onSubmit={(v) => create.mutate(v)}
            onCancel={() => setAdding(false)}
            pending={create.isPending}
          />
        </div>
      )}

      {assets.data && assets.data.length === 0 && !adding && (
        <p className="text-sm text-[var(--muted)]">
          Add a car, home, or cash held outside your linked accounts.
        </p>
      )}

      <ul className="divide-y divide-[var(--border)]">
        {assets.data?.map((a) =>
          editing === a.id ? (
            <li key={a.id} className="py-3">
              <AssetForm
                initial={a}
                onSubmit={(input) => update.mutate({ id: a.id, input })}
                onCancel={() => setEditing(null)}
                pending={update.isPending}
              />
            </li>
          ) : (
            <li
              key={a.id}
              className="flex items-center justify-between gap-3 py-2.5 text-sm"
            >
              <div className="min-w-0">
                <div className="truncate font-medium">{a.name}</div>
                <div className="text-xs text-[var(--muted)]">
                  {ASSET_KINDS.find(([v]) => v === a.kind)?.[1] ?? a.kind}
                  {a.depreciation_annual_pct
                    ? ` · −${a.depreciation_annual_pct}%/yr from ${a.as_of}`
                    : ` · as of ${a.as_of}`}
                </div>
              </div>
              <div className="flex items-center gap-3">
                <div className="text-right">
                  <div className="font-semibold">{money(a.current_value)}</div>
                  {Math.abs(a.current_value - a.value) >= 1 && (
                    <div className="text-xs text-[var(--muted)]">
                      entered {money(a.value)}
                    </div>
                  )}
                </div>
                <button
                  className="text-xs text-[var(--muted)] underline"
                  onClick={() => setEditing(a.id)}
                >
                  Edit
                </button>
                <button
                  className="text-xs text-red-500 underline"
                  onClick={() => remove.mutate(a.id)}
                >
                  Delete
                </button>
              </div>
            </li>
          ),
        )}
      </ul>

      {(assets.data?.length ?? 0) > 0 && (
        <div className="mt-2 flex justify-between border-t border-[var(--border)] pt-2 text-sm font-semibold">
          <span>Total</span>
          <span>{money(total)}</span>
        </div>
      )}

      {(create.isError || update.isError || remove.isError) && (
        <p className="mt-2 text-sm text-red-500">
          {errorMessage(create.error ?? update.error ?? remove.error)}
        </p>
      )}
    </Card>
  );
}

/* ------------------------------------------------------------- liabilities */

function LiabilityForm({
  initial,
  onSubmit,
  onCancel,
  pending,
}: {
  initial?: ManualLiability;
  onSubmit: (v: LiabilityInput) => void;
  onCancel: () => void;
  pending: boolean;
}) {
  const [name, setName] = useState(initial?.name ?? "");
  const [kind, setKind] = useState(initial?.kind ?? "mortgage");
  const [balance, setBalance] = useState(
    initial ? String(initial.balance) : "",
  );
  const [asOf, setAsOf] = useState(initial?.as_of ?? today());
  const [apr, setApr] = useState(
    initial?.apr != null ? String(initial.apr) : "",
  );
  const [minPay, setMinPay] = useState(
    initial?.minimum_payment != null ? String(initial.minimum_payment) : "",
  );
  const [note, setNote] = useState(initial?.note ?? "");

  const valid =
    name.trim() !== "" && balance !== "" && !Number.isNaN(Number(balance));

  return (
    <div className="space-y-3 rounded-lg border border-[var(--border)] bg-[var(--bg)] p-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <Labeled label="Name">
          <input
            className={fieldCls()}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Home mortgage"
          />
        </Labeled>
        <Labeled label="Type">
          <select
            className={fieldCls()}
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            {LIABILITY_KINDS.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </Labeled>
        <Labeled label="Balance owed">
          <input
            className={fieldCls()}
            type="number"
            value={balance}
            onChange={(e) => setBalance(e.target.value)}
            placeholder="320000"
          />
        </Labeled>
        <Labeled label="As of">
          <input
            className={fieldCls()}
            type="date"
            value={asOf}
            onChange={(e) => setAsOf(e.target.value)}
          />
        </Labeled>
        <Labeled label="APR % (optional)">
          <input
            className={fieldCls()}
            type="number"
            value={apr}
            onChange={(e) => setApr(e.target.value)}
          />
        </Labeled>
        <Labeled label="Minimum payment / mo (optional)">
          <input
            className={fieldCls()}
            type="number"
            value={minPay}
            onChange={(e) => setMinPay(e.target.value)}
          />
        </Labeled>
        <Labeled label="Note (optional)">
          <input
            className={fieldCls()}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
        </Labeled>
      </div>
      <div className="flex gap-2">
        <Button
          disabled={!valid || pending}
          onClick={() =>
            onSubmit({
              name: name.trim(),
              kind,
              balance: Number(balance),
              as_of: asOf,
              apr: apr === "" ? null : Number(apr),
              minimum_payment: minPay === "" ? null : Number(minPay),
              note: note.trim() === "" ? null : note.trim(),
            })
          }
        >
          {initial ? "Save" : "Add liability"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

function LiabilitiesCard() {
  const qc = useQueryClient();
  const items = useQuery({
    queryKey: ["manual-liabilities"],
    queryFn: api.listManualLiabilities,
  });
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["manual-liabilities"] });
    qc.invalidateQueries({ queryKey: ["net-worth-now"] });
    qc.invalidateQueries({ queryKey: ["net-worth-history"] });
  };

  const create = useMutation({
    mutationFn: (v: LiabilityInput) => api.createManualLiability(v),
    onSuccess: () => {
      setAdding(false);
      invalidate();
    },
  });
  const update = useMutation({
    mutationFn: (v: { id: string; input: LiabilityInput }) =>
      api.updateManualLiability(v.id, v.input),
    onSuccess: () => {
      setEditing(null);
      invalidate();
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteManualLiability(id),
    onSuccess: invalidate,
  });

  const total = (items.data ?? []).reduce((s, l) => s + l.balance, 0);

  return (
    <Card
      title="Manual liabilities"
      actions={
        !adding && (
          <Button variant="ghost" onClick={() => setAdding(true)}>
            Add
          </Button>
        )
      }
    >
      {adding && (
        <div className="mb-3">
          <LiabilityForm
            onSubmit={(v) => create.mutate(v)}
            onCancel={() => setAdding(false)}
            pending={create.isPending}
          />
        </div>
      )}

      {items.data && items.data.length === 0 && !adding && (
        <p className="text-sm text-[var(--muted)]">
          Add a mortgage, car loan, or student loan that isn't a linked account.
        </p>
      )}

      <ul className="divide-y divide-[var(--border)]">
        {items.data?.map((l) =>
          editing === l.id ? (
            <li key={l.id} className="py-3">
              <LiabilityForm
                initial={l}
                onSubmit={(input) => update.mutate({ id: l.id, input })}
                onCancel={() => setEditing(null)}
                pending={update.isPending}
              />
            </li>
          ) : (
            <li
              key={l.id}
              className="flex items-center justify-between gap-3 py-2.5 text-sm"
            >
              <div className="min-w-0">
                <div className="truncate font-medium">{l.name}</div>
                <div className="text-xs text-[var(--muted)]">
                  {LIABILITY_KINDS.find(([v]) => v === l.kind)?.[1] ?? l.kind}
                  {l.apr ? ` · ${l.apr}% APR` : ""}
                  {l.minimum_payment
                    ? ` · ${money(l.minimum_payment)}/mo min`
                    : ""}
                </div>
              </div>
              <div className="flex items-center gap-3">
                <span className="font-semibold text-red-500">
                  {money(l.balance)}
                </span>
                <button
                  className="text-xs text-[var(--muted)] underline"
                  onClick={() => setEditing(l.id)}
                >
                  Edit
                </button>
                <button
                  className="text-xs text-red-500 underline"
                  onClick={() => remove.mutate(l.id)}
                >
                  Delete
                </button>
              </div>
            </li>
          ),
        )}
      </ul>

      {(items.data?.length ?? 0) > 0 && (
        <div className="mt-2 flex justify-between border-t border-[var(--border)] pt-2 text-sm font-semibold">
          <span>Total</span>
          <span className="text-red-500">{money(total)}</span>
        </div>
      )}

      {(create.isError || update.isError || remove.isError) && (
        <p className="mt-2 text-sm text-red-500">
          {errorMessage(create.error ?? update.error ?? remove.error)}
        </p>
      )}
    </Card>
  );
}

/* --------------------------------------------------------------- recurring */

function RecurringForm({
  kind,
  initial,
  onSubmit,
  onCancel,
  pending,
}: {
  kind: "expense" | "income";
  initial?: Recurring;
  onSubmit: (v: RecurringInput) => void;
  onCancel: () => void;
  pending: boolean;
}) {
  const [label, setLabel] = useState(initial?.label ?? "");
  const [amount, setAmount] = useState(initial ? String(initial.amount) : "");
  const [cadence, setCadence] = useState(initial?.cadence ?? "monthly");
  const [nextDue, setNextDue] = useState(initial?.next_due ?? "");

  const valid =
    label.trim() !== "" && amount !== "" && !Number.isNaN(Number(amount));

  return (
    <div className="space-y-3 rounded-lg border border-[var(--border)] bg-[var(--bg)] p-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <Labeled label="Label">
          <input
            className={fieldCls()}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={kind === "income" ? "Paycheck" : "Rent"}
          />
        </Labeled>
        <Labeled label="Amount">
          <input
            className={fieldCls()}
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
          />
        </Labeled>
        <Labeled label="Cadence">
          <select
            className={fieldCls()}
            value={cadence}
            onChange={(e) => setCadence(e.target.value)}
          >
            {CADENCES.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </Labeled>
        <Labeled label="Next due (optional)">
          <input
            className={fieldCls()}
            type="date"
            value={nextDue}
            onChange={(e) => setNextDue(e.target.value)}
          />
        </Labeled>
      </div>
      <div className="flex gap-2">
        <Button
          disabled={!valid || pending}
          onClick={() =>
            onSubmit({
              label: label.trim(),
              amount: Number(amount),
              cadence,
              kind,
              next_due: nextDue === "" ? null : nextDue,
              active: true,
            })
          }
        >
          {initial ? "Save" : kind === "income" ? "Add income" : "Add payment"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

function RecurringRow({
  r,
  onEdit,
  onToggle,
  onDelete,
}: {
  r: Recurring;
  onEdit: () => void;
  onToggle: () => void;
  onDelete: () => void;
}) {
  return (
    <li
      className={`flex items-center justify-between gap-3 py-2.5 text-sm ${
        r.active ? "" : "opacity-50"
      }`}
    >
      <div className="min-w-0">
        <div className="truncate font-medium">
          {r.label}
          {r.source === "detected" && (
            <span className="ml-2 rounded bg-[var(--border)] px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-[var(--muted)]">
              detected
            </span>
          )}
        </div>
        <div className="text-xs text-[var(--muted)]">
          {money(r.amount)} · {r.cadence}
          {r.category_label ? ` · ${r.category_label}` : ""}
          {r.next_due ? ` · next ${r.next_due}` : ""}
        </div>
      </div>
      <div className="flex items-center gap-3">
        <span className="text-right font-semibold">
          {money(r.monthly_equiv)}
          <span className="block text-[10px] font-normal text-[var(--muted)]">
            / month
          </span>
        </span>
        <button
          className="text-xs text-[var(--muted)] underline"
          onClick={onToggle}
        >
          {r.active ? "Pause" : "Resume"}
        </button>
        <button
          className="text-xs text-[var(--muted)] underline"
          onClick={onEdit}
        >
          Edit
        </button>
        <button
          className="text-xs text-red-500 underline"
          onClick={onDelete}
        >
          Delete
        </button>
      </div>
    </li>
  );
}

function RecurringCard() {
  const qc = useQueryClient();
  const list = useQuery({ queryKey: ["recurring"], queryFn: api.listRecurring });
  const [addKind, setAddKind] = useState<"expense" | "income" | null>(null);
  const [editing, setEditing] = useState<string | null>(null);

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["recurring"] });
    qc.invalidateQueries({ queryKey: ["recurring-summary"] });
  };

  const create = useMutation({
    mutationFn: (v: RecurringInput) => api.createRecurring(v),
    onSuccess: () => {
      setAddKind(null);
      invalidate();
    },
  });
  const update = useMutation({
    mutationFn: (v: { id: string; input: RecurringInput }) =>
      api.updateRecurring(v.id, v.input),
    onSuccess: () => {
      setEditing(null);
      invalidate();
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteRecurring(id),
    onSuccess: invalidate,
  });
  const detect = useMutation({
    mutationFn: () => api.detectRecurring(),
    onSuccess: invalidate,
  });

  const toRow = (r: Recurring): { id: string; input: RecurringInput } => ({
    id: r.id,
    input: {
      label: r.label,
      amount: r.amount,
      cadence: r.cadence,
      kind: r.kind,
      category_id: r.category_id,
      next_due: r.next_due,
      active: r.active,
    },
  });

  const expenses = (list.data ?? []).filter((r) => r.kind === "expense");
  const income = (list.data ?? []).filter((r) => r.kind === "income");

  const section = (
    title: string,
    kind: "expense" | "income",
    rows: Recurring[],
  ) => (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
          {title}
        </h3>
        {addKind !== kind && (
          <button
            className="text-xs text-[var(--accent)] underline"
            onClick={() => {
              setAddKind(kind);
              setEditing(null);
            }}
          >
            Add
          </button>
        )}
      </div>
      {addKind === kind && (
        <div className="mb-2">
          <RecurringForm
            kind={kind}
            onSubmit={(v) => create.mutate(v)}
            onCancel={() => setAddKind(null)}
            pending={create.isPending}
          />
        </div>
      )}
      {rows.length === 0 && addKind !== kind ? (
        <p className="py-1 text-sm text-[var(--muted)]">None yet.</p>
      ) : (
        <ul className="divide-y divide-[var(--border)]">
          {rows.map((r) =>
            editing === r.id ? (
              <li key={r.id} className="py-3">
                <RecurringForm
                  kind={kind}
                  initial={r}
                  onSubmit={(input) => update.mutate({ id: r.id, input })}
                  onCancel={() => setEditing(null)}
                  pending={update.isPending}
                />
              </li>
            ) : (
              <RecurringRow
                key={r.id}
                r={r}
                onEdit={() => {
                  setEditing(r.id);
                  setAddKind(null);
                }}
                onToggle={() =>
                  update.mutate({
                    id: r.id,
                    input: { ...toRow(r).input, active: !r.active },
                  })
                }
                onDelete={() => remove.mutate(r.id)}
              />
            ),
          )}
        </ul>
      )}
    </div>
  );

  return (
    <Card
      title="Recurring payments & income"
      actions={
        <Button
          variant="ghost"
          disabled={detect.isPending}
          onClick={() => detect.mutate()}
        >
          {detect.isPending ? "Scanning…" : "Detect from transactions"}
        </Button>
      }
    >
      {detect.data && (
        <div className="mb-3">
          <Banner tone={detect.data.added > 0 ? "success" : "info"}>
            {detect.data.added > 0
              ? `Found ${detect.data.added} new recurring pattern${
                  detect.data.added === 1 ? "" : "s"
                }.`
              : "No new recurring patterns found."}
          </Banner>
        </div>
      )}

      <div className="space-y-4">
        {section("Payments", "expense", expenses)}
        {section("Income", "income", income)}
      </div>

      {(create.isError ||
        update.isError ||
        remove.isError ||
        detect.isError) && (
        <p className="mt-2 text-sm text-red-500">
          {errorMessage(
            create.error ?? update.error ?? remove.error ?? detect.error,
          )}
        </p>
      )}
    </Card>
  );
}

/* -------------------------------------------------------------------- page */

export default function NetWorth() {
  const [range, setRange] = useState<Range>({ from: "", to: "" });

  const nw = useQuery({ queryKey: ["net-worth-now"], queryFn: api.netWorthNow });
  const hist = useQuery({
    queryKey: ["net-worth-history", range],
    queryFn: () => api.netWorthHistory(range.from, range.to),
    enabled: !!range.from && !!range.to,
  });
  const alloc = useQuery({
    queryKey: ["asset-allocation"],
    queryFn: api.assetAllocation,
  });
  const summary = useQuery({
    queryKey: ["recurring-summary"],
    queryFn: api.recurringSummary,
  });

  const allocMax = useMemo(
    () => Math.max(1, ...(alloc.data ?? []).map((s) => s.value)),
    [alloc.data],
  );

  const n = nw.data;
  const totalAssets = n ? n.cash + n.investments + n.manual_assets : 0;
  const totalDebt = n ? n.account_debt + n.manual_liabilities : 0;
  const monthlyIncome = summary.data?.monthly_income ?? 0;
  const monthlyExpense = summary.data?.monthly_expense ?? 0;
  const monthlyNet = monthlyIncome - monthlyExpense;
  const savingsRate =
    monthlyIncome > 0 ? (monthlyNet / monthlyIncome) * 100 : null;

  return (
    <>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">Net Worth</h1>
        <RangePicker onChange={setRange} />
      </div>

      <Card
        title="Net worth over time"
        actions={
          n && <span className="text-lg font-semibold">{money(n.net)}</span>
        }
      >
        <ValueAreaChart data={hist.data ?? []} height={240} />
        {n && (
          <div className="mt-3 grid gap-x-6 gap-y-1 text-xs text-[var(--muted)] sm:grid-cols-2 md:grid-cols-3">
            <span>Cash {money(n.cash)}</span>
            <span>Investments {money(n.investments)}</span>
            <span>Manual assets {money(n.manual_assets)}</span>
            <span className="text-red-500">
              Account debt −{money(n.account_debt)}
            </span>
            <span className="text-red-500">
              Manual liabilities −{money(n.manual_liabilities)}
            </span>
          </div>
        )}
        <p className="mt-2 text-[11px] text-[var(--muted)]">
          Shared cards count only the charges attributed to you.
        </p>
      </Card>

      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <Card title="Asset allocation">
          {alloc.data && alloc.data.length === 0 ? (
            <p className="text-sm text-[var(--muted)]">
              No assets yet. Link accounts or add manual assets below.
            </p>
          ) : (
            <ul className="space-y-2.5">
              {alloc.data?.map((s) => (
                <li key={s.label}>
                  <div className="mb-1 flex justify-between text-sm">
                    <span>{s.label}</span>
                    <span className="font-medium">
                      {money(s.value)}
                      <span className="ml-1.5 text-xs text-[var(--muted)]">
                        {totalAssets > 0
                          ? `${Math.round((s.value / totalAssets) * 100)}%`
                          : ""}
                      </span>
                    </span>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-[var(--border)]">
                    <div
                      className="h-full rounded-full bg-[var(--accent)]"
                      style={{ width: `${(s.value / allocMax) * 100}%` }}
                    />
                  </div>
                </li>
              ))}
            </ul>
          )}
          <div className="mt-3 space-y-1 border-t border-[var(--border)] pt-3 text-sm">
            <div className="flex justify-between">
              <span>Total assets</span>
              <span className="font-semibold">{money(totalAssets)}</span>
            </div>
            <div className="flex justify-between">
              <span>Total debt</span>
              <span className="font-semibold text-red-500">
                {money(totalDebt)}
              </span>
            </div>
          </div>
        </Card>

        <Card title="Monthly cash flow">
          <div className="space-y-1.5 text-sm">
            <div className="flex justify-between">
              <span>Recurring income</span>
              <span className="font-semibold text-green-600 dark:text-green-400">
                {money(monthlyIncome)}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Recurring payments</span>
              <span className="font-semibold text-red-500">
                −{money(monthlyExpense)}
              </span>
            </div>
            <div className="flex justify-between border-t border-[var(--border)] pt-1.5">
              <span>Net per month</span>
              <span
                className={`font-semibold ${
                  monthlyNet < 0 ? "text-red-500" : ""
                }`}
              >
                {money(monthlyNet)}
              </span>
            </div>
            {savingsRate != null && (
              <div className="flex justify-between">
                <span>Savings rate</span>
                <span className="font-semibold">
                  {Math.round(savingsRate)}%
                </span>
              </div>
            )}
          </div>
          <p className="mt-3 text-[11px] text-[var(--muted)]">
            Based on active recurring items only. Use “Detect from transactions”
            below to populate them from your history.
          </p>
        </Card>
      </div>

      <div className="mt-5 space-y-5">
        <AssetsCard />
        <LiabilitiesCard />
        <RecurringCard />
      </div>
    </>
  );
}
