import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import { money } from "../lib/format";
import { Banner, Button, Card } from "../components/ui";
import RulesSection from "../components/RulesSection";

export default function Review() {
  const qc = useQueryClient();
  const inbox = useQuery({ queryKey: ["review-inbox"], queryFn: api.reviewInbox });
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [assignTo, setAssignTo] = useState("");

  const others = (people.data ?? []).filter((p) => !p.is_self);

  const refresh = () => {
    setSelected(new Set());
    qc.invalidateQueries({ queryKey: ["review-inbox"] });
    qc.invalidateQueries({ queryKey: ["review-count"] });
    qc.invalidateQueries({ queryKey: ["spending-summary"] });
    qc.invalidateQueries({ queryKey: ["spending-by-person"] });
    qc.invalidateQueries({ queryKey: ["spending-trends"] });
    qc.invalidateQueries({ queryKey: ["transactions-table"] });
    // attributing a shared-card charge to yourself changes net worth
    qc.invalidateQueries({ queryKey: ["net-worth-now"] });
    qc.invalidateQueries({ queryKey: ["net-worth-history"] });
  };

  const decide = useMutation({
    mutationFn: (v: {
      ids: string[];
      decision: "keep" | "assign" | "exclude";
      personId?: string;
    }) =>
      api.reviewDecide({
        txn_ids: v.ids,
        decision: v.decision,
        person_id: v.personId ?? null,
      }),
    onSuccess: refresh,
  });

  const applyRules = useMutation({
    mutationFn: api.applyRulesNow,
    onSuccess: refresh,
  });

  const rows = inbox.data ?? [];
  const allSelected = rows.length > 0 && selected.size === rows.length;
  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const act = (decision: "keep" | "assign" | "exclude", ids: string[]) => {
    if (decision === "assign" && !assignTo) return;
    decide.mutate({ ids, decision, personId: assignTo || undefined });
  };

  return (
    <>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-xl font-semibold">Review</h1>
        <Button
          variant="ghost"
          disabled={applyRules.isPending}
          onClick={() => applyRules.mutate()}
        >
          {applyRules.isPending ? "Applying…" : "Apply rules now"}
        </Button>
      </div>

      <p className="mb-4 text-xs text-[var(--muted)]">
        Charges on cards you marked <strong>Shared</strong> land here (plus anything
        you send back with <em>-reset-</em>). Plaid can't tell which cardholder made
        a purchase, so you decide: <strong>Keep</strong> (yours),{" "}
        <strong>Assign</strong> to someone else, or <strong>Exclude</strong> (not
        your spending). Rules below can pre-fill the suggestion.
      </p>

      {decide.isError && (
        <Banner tone="error">{errorMessage(decide.error)}</Banner>
      )}

      {others.length === 0 && (
        <div className="mb-4">
          <Banner tone="info">
            To assign charges to someone else, add them under{" "}
            <strong>Settings → People</strong> first.
          </Banner>
        </div>
      )}

      <Card
        title={`Pending (${rows.length})`}
        actions={
          rows.length > 0 && (
            <label className="flex items-center gap-1.5 text-xs text-[var(--muted)]">
              <input
                type="checkbox"
                checked={allSelected}
                onChange={(e) =>
                  setSelected(
                    e.target.checked ? new Set(rows.map((r) => r.id)) : new Set(),
                  )
                }
              />
              select all
            </label>
          )
        }
      >
        {rows.length === 0 ? (
          <p className="py-4 text-sm text-[var(--muted)]">
            Nothing to review. 🎉
          </p>
        ) : (
          <div className="divide-y divide-[var(--border)]">
            {rows.map((r) => (
              <div key={r.id} className="flex items-center gap-3 py-2 text-sm">
                <input
                  type="checkbox"
                  checked={selected.has(r.id)}
                  onChange={() => toggle(r.id)}
                />
                <span className="w-20 shrink-0 text-xs tabular-nums text-[var(--muted)]">
                  {r.posted_date}
                </span>
                <span className="min-w-0 flex-1 truncate">
                  {r.merchant_name || r.description}
                  {r.suggested_person_name && (
                    <span className="ml-2 rounded-full bg-[var(--accent)]/10 px-1.5 py-0.5 text-[10px] text-[var(--accent)]">
                      suggested: {r.suggested_person_name}
                    </span>
                  )}
                </span>
                <span className="shrink-0 text-xs text-[var(--muted)]">
                  {r.account_name}
                </span>
                <span className="w-20 shrink-0 text-right font-medium tabular-nums">
                  {money(r.amount, r.currency)}
                </span>
                <span className="flex shrink-0 items-center gap-1">
                  <Button variant="ghost" onClick={() => act("keep", [r.id])}>
                    Keep
                  </Button>
                  {others.length > 0 && (
                    <select
                      value=""
                      onChange={(e) =>
                        e.target.value &&
                        decide.mutate({
                          ids: [r.id],
                          decision: "assign",
                          personId: e.target.value,
                        })
                      }
                      className="rounded border border-[var(--border)] bg-[var(--bg)] px-1.5 py-1 text-xs"
                    >
                      <option value="">
                        {r.suggested_person_name
                          ? `Assign (→ ${r.suggested_person_name})`
                          : "Assign to…"}
                      </option>
                      {others.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                  )}
                  <Button variant="ghost" onClick={() => act("exclude", [r.id])}>
                    Exclude
                  </Button>
                </span>
              </div>
            ))}
          </div>
        )}

        {selected.size > 0 && (
          <div className="sticky bottom-0 mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--card)] p-2 text-sm">
            <span className="text-[var(--muted)]">{selected.size} selected</span>
            <Button onClick={() => act("keep", [...selected])}>Keep (mine)</Button>
            {others.length > 0 && (
              <span className="flex items-center gap-1">
                <select
                  value={assignTo}
                  onChange={(e) => setAssignTo(e.target.value)}
                  className="rounded border border-[var(--border)] bg-[var(--bg)] px-2 py-1 text-xs"
                >
                  <option value="">Assign to…</option>
                  {others.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                <Button
                  variant="ghost"
                  disabled={!assignTo}
                  onClick={() => act("assign", [...selected])}
                >
                  Assign
                </Button>
              </span>
            )}
            <Button variant="ghost" onClick={() => act("exclude", [...selected])}>
              Exclude
            </Button>
          </div>
        )}
      </Card>

      <div className="mt-5">
        <RulesSection />
      </div>
    </>
  );
}
