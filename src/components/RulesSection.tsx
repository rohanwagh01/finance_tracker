import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import type { RuleInput } from "../lib/types";
import { Banner, Button, Card } from "./ui";

const FIELDS = [
  { v: "description", label: "Description" },
  { v: "merchant_name", label: "Merchant" },
  { v: "amount", label: "Amount" },
];
const TEXT_OPS = [
  { v: "contains", label: "contains" },
  { v: "equals", label: "equals" },
  { v: "regex", label: "matches regex" },
];
const AMOUNT_OPS = [
  { v: "gt", label: ">" },
  { v: "lt", label: "<" },
  { v: "equals", label: "=" },
  { v: "between", label: "between" },
];

const EMPTY: RuleInput = {
  match_field: "description",
  match_op: "contains",
  match_value: "",
  match_value2: null,
  account_id: null,
  set_person_id: null,
  set_category_id: null,
  high_confidence: false,
};

export default function RulesSection() {
  const qc = useQueryClient();
  const rules = useQuery({ queryKey: ["rules"], queryFn: api.listRules });
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });
  const cats = useQuery({ queryKey: ["categories"], queryFn: api.listCategories });
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: api.listAccounts });

  const [form, setForm] = useState<RuleInput>(EMPTY);
  const [open, setOpen] = useState(false);

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["rules"] });
    qc.invalidateQueries({ queryKey: ["review-inbox"] });
  };

  const create = useMutation({
    mutationFn: () => api.createRule(form),
    onSuccess: () => {
      setForm(EMPTY);
      setOpen(false);
      refresh();
      api.applyRulesNow().then(refresh);
    },
  });
  const del = useMutation({
    mutationFn: (id: string) => api.deleteRule(id),
    onSuccess: refresh,
  });
  const toggle = useMutation({
    mutationFn: (v: { id: string; input: RuleInput }) =>
      api.updateRule(v.id, v.input),
    onSuccess: refresh,
  });

  const isAmount = form.match_field === "amount";
  const ops = isAmount ? AMOUNT_OPS : TEXT_OPS;

  return (
    <Card
      title="Attribution rules"
      actions={
        <Button variant="ghost" onClick={() => setOpen((v) => !v)}>
          {open ? "Cancel" : "Add rule"}
        </Button>
      }
    >
      <p className="mb-3 text-xs text-[var(--muted)]">
        A rule suggests a person for matching charges. Mark it{" "}
        <strong>high-confidence</strong> and enable auto-confirm in Settings to skip
        the inbox for those.
      </p>

      {open && (
        <div className="mb-4 space-y-2 rounded-lg border border-[var(--border)] p-3 text-sm">
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={form.match_field}
              onChange={(e) =>
                setForm({
                  ...form,
                  match_field: e.target.value,
                  match_op: e.target.value === "amount" ? "gt" : "contains",
                })
              }
              className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
            >
              {FIELDS.map((f) => (
                <option key={f.v} value={f.v}>
                  {f.label}
                </option>
              ))}
            </select>
            <select
              value={form.match_op}
              onChange={(e) => setForm({ ...form, match_op: e.target.value })}
              className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
            >
              {ops.map((o) => (
                <option key={o.v} value={o.v}>
                  {o.label}
                </option>
              ))}
            </select>
            <input
              value={form.match_value}
              onChange={(e) => setForm({ ...form, match_value: e.target.value })}
              placeholder={isAmount ? "amount" : "text"}
              className="w-40 rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
            />
            {form.match_op === "between" && (
              <input
                value={form.match_value2 ?? ""}
                onChange={(e) =>
                  setForm({ ...form, match_value2: e.target.value })
                }
                placeholder="and"
                className="w-24 rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
              />
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-[var(--muted)]">→ assign to</span>
            <select
              value={form.set_person_id ?? ""}
              onChange={(e) =>
                setForm({ ...form, set_person_id: e.target.value || null })
              }
              className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
            >
              <option value="">(no person)</option>
              {people.data?.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <span className="text-xs text-[var(--muted)]">category</span>
            <select
              value={form.set_category_id ?? ""}
              onChange={(e) =>
                setForm({ ...form, set_category_id: e.target.value || null })
              }
              className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1"
            >
              <option value="">(keep)</option>
              {cats.data?.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.parent_id ? `  ${c.label}` : c.label}
                </option>
              ))}
            </select>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <select
              value={form.account_id ?? ""}
              onChange={(e) =>
                setForm({ ...form, account_id: e.target.value || null })
              }
              className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1 text-xs"
            >
              <option value="">any account</option>
              {accounts.data?.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-1.5 text-xs">
              <input
                type="checkbox"
                checked={form.high_confidence}
                onChange={(e) =>
                  setForm({ ...form, high_confidence: e.target.checked })
                }
              />
              high-confidence
            </label>
            <Button
              disabled={!form.match_value.trim() || create.isPending}
              onClick={() => create.mutate()}
            >
              Save rule
            </Button>
          </div>
          {create.isError && (
            <Banner tone="error">{errorMessage(create.error)}</Banner>
          )}
        </div>
      )}

      {rules.data?.length === 0 && !open && (
        <p className="text-sm text-[var(--muted)]">No rules yet.</p>
      )}

      <ul className="space-y-1.5 text-sm">
        {rules.data?.map((r) => (
          <li key={r.id} className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={r.enabled}
              onChange={() =>
                toggle.mutate({
                  id: r.id,
                  input: {
                    match_field: r.match_field,
                    match_op: r.match_op,
                    match_value: r.match_value,
                    match_value2: r.match_value2,
                    account_id: r.account_id,
                    set_person_id: r.set_person_id,
                    set_category_id: r.set_category_id,
                    high_confidence: r.high_confidence,
                    priority: r.priority,
                    enabled: !r.enabled,
                  },
                })
              }
            />
            <span className={r.enabled ? "" : "text-[var(--muted)] line-through"}>
              {FIELDS.find((f) => f.v === r.match_field)?.label} {r.match_op}{" "}
              <strong>{r.match_value}</strong>
              {r.match_value2 && ` / ${r.match_value2}`}
              {" → "}
              {r.set_person_name ?? "—"}
              {r.high_confidence && " ⚡"}
              {r.account_name && ` (${r.account_name})`}
            </span>
            <button
              className="ml-auto text-xs text-red-500 underline"
              onClick={() => del.mutate(r.id)}
            >
              delete
            </button>
          </li>
        ))}
      </ul>
    </Card>
  );
}
