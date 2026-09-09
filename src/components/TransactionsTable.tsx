import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import type { SpendingFilter, TxnRow } from "../lib/types";
import { money } from "../lib/format";
import { Button } from "./ui";

const PAGE = 50;

export default function TransactionsTable({
  filter,
  onRecategorized,
}: {
  filter: SpendingFilter;
  onRecategorized: () => void;
}) {
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");
  const [page, setPage] = useState(0);

  useEffect(() => {
    const t = setTimeout(() => {
      setDebounced(search);
      setPage(0);
    }, 300);
    return () => clearTimeout(t);
  }, [search]);

  const cats = useQuery({ queryKey: ["categories"], queryFn: api.listCategories });
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });

  const q = useQuery({
    queryKey: [
      "transactions-table",
      filter.from,
      filter.to,
      filter.account_ids?.join(",") ?? "",
      filter.person_id ?? "",
      filter.excluded_only ? "excluded" : "",
      debounced,
      page,
    ],
    queryFn: () =>
      api.listTransactions(filter, {
        search: debounced || null,
        limit: PAGE,
        offset: page * PAGE,
      }),
  });

  const bump = () => {
    q.refetch();
    onRecategorized();
  };

  const recategorize = useMutation({
    mutationFn: (v: { id: string; categoryId: string | null }) =>
      api.setTransactionCategory(v.id, v.categoryId),
    onSuccess: bump,
  });

  const self = people.data?.find((p) => p.is_self);
  const others = (people.data ?? []).filter((p) => !p.is_self);

  const reassign = useMutation({
    mutationFn: (v: { id: string; value: string }) => {
      if (v.value === "__mine__")
        return api.reviewDecide({ txn_ids: [v.id], decision: "keep" });
      if (v.value === "__exclude__")
        return api.reviewDecide({ txn_ids: [v.id], decision: "exclude" });
      if (v.value === "__reset__")
        return api.reviewDecide({ txn_ids: [v.id], decision: "reset" });
      return api.reviewDecide({
        txn_ids: [v.id],
        decision: "assign",
        person_id: v.value,
      });
    },
    onSuccess: bump,
  });

  const ownerValue = (t: TxnRow) => {
    if (t.review_status === "excluded") return "__exclude__";
    if (t.review_status === "pending") return "";
    if (t.owner_person_id)
      return t.owner_person_id === self?.id ? "__mine__" : t.owner_person_id;
    return "__mine__"; // not_required
  };

  const total = q.data?.total_count ?? 0;
  const pages = Math.max(1, Math.ceil(total / PAGE));

  return (
    <div>
      <input
        placeholder="Search description or merchant…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="mb-3 w-full rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)]"
      />

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-xs text-[var(--muted)]">
              <th className="py-1.5 pr-3 font-medium">Date</th>
              <th className="py-1.5 pr-3 font-medium">Description</th>
              <th className="py-1.5 pr-3 font-medium">Account</th>
              <th className="py-1.5 pr-3 font-medium">Category</th>
              <th className="py-1.5 pr-3 font-medium">Whose</th>
              <th className="py-1.5 pl-3 text-right font-medium">Amount</th>
            </tr>
          </thead>
          <tbody>
            {q.data?.rows.map((t) => (
              <tr key={t.id} className="border-t border-[var(--border)]">
                <td className="py-1.5 pr-3 tabular-nums text-[var(--muted)]">
                  {t.posted_date}
                </td>
                <td className="max-w-[200px] truncate py-1.5 pr-3">
                  {t.merchant_name || t.description}
                  {t.pending && (
                    <span className="text-[var(--muted)]"> · pending</span>
                  )}
                </td>
                <td className="py-1.5 pr-3 text-[var(--muted)]">
                  {t.account_name}
                </td>
                <td className="py-1.5 pr-3">
                  <select
                    value={t.category_id ?? ""}
                    onChange={(e) =>
                      recategorize.mutate({
                        id: t.id,
                        categoryId: e.target.value || null,
                      })
                    }
                    className="max-w-[150px] rounded border border-[var(--border)] bg-[var(--bg)] px-1.5 py-1 text-xs"
                  >
                    <option value="">Uncategorized</option>
                    {(cats.data ?? []).map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.parent_id ? `  ${c.label}` : c.label}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="py-1.5 pr-3">
                  {!t.account_is_shared ? (
                    <span className="text-xs text-[var(--muted)]">Not shared</span>
                  ) : (
                    <select
                      value={ownerValue(t)}
                      onChange={(e) =>
                        e.target.value &&
                        reassign.mutate({ id: t.id, value: e.target.value })
                      }
                      className="rounded border border-[var(--border)] bg-[var(--bg)] px-1.5 py-1 text-xs"
                    >
                      {t.review_status === "pending" && (
                        <option value="" disabled>
                          Unreviewed
                        </option>
                      )}
                      <option value="__mine__">Mine</option>
                      {others.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                      <option value="__exclude__">Excluded</option>
                      {t.review_status !== "pending" && (
                        <option value="__reset__">-reset-</option>
                      )}
                    </select>
                  )}
                </td>
                <td className="py-1.5 pl-3 text-right font-medium tabular-nums">
                  {money(t.amount, t.currency)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {q.data && q.data.rows.length === 0 && (
        <p className="py-4 text-sm text-[var(--muted)]">No transactions.</p>
      )}
      {(recategorize.isError || reassign.isError) && (
        <p className="mt-2 text-xs text-red-500">
          {errorMessage(recategorize.error ?? reassign.error)}
        </p>
      )}

      {pages > 1 && (
        <div className="mt-3 flex items-center gap-3 text-xs text-[var(--muted)]">
          <Button
            variant="ghost"
            disabled={page === 0}
            onClick={() => setPage((p) => p - 1)}
          >
            Prev
          </Button>
          <span>
            Page {page + 1} of {pages} · {total} total
          </span>
          <Button
            variant="ghost"
            disabled={page + 1 >= pages}
            onClick={() => setPage((p) => p + 1)}
          >
            Next
          </Button>
        </div>
      )}
    </div>
  );
}
