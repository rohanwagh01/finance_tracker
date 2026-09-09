import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import type { AccountView, TxnRow } from "../lib/types";
import { money } from "../lib/format";
import { Banner, Button, Toggle } from "./ui";
import ValueAreaChart from "./ValueAreaChart";

function twoYearsAgo() {
  const d = new Date();
  d.setDate(d.getDate() - 730);
  return d.toISOString().slice(0, 10);
}

function attribution(t: TxnRow): string {
  switch (t.review_status) {
    case "pending":
      return "Pending review";
    case "excluded":
      return "Excluded";
    case "kept":
    case "assigned":
      return t.owner_person_name ?? "—";
    default:
      return t.account_is_shared ? "—" : "Not shared";
  }
}

export default function AccountDetailModal({
  account,
  onClose,
}: {
  account: AccountView;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [confirmReset, setConfirmReset] = useState(false);

  const txns = useQuery({
    queryKey: ["account-transactions", account.id],
    queryFn: () => api.accountTransactions(account.id, 200),
  });
  const hist = useQuery({
    queryKey: ["account-value-history", account.id],
    queryFn: () =>
      api.accountValueHistory(
        account.id,
        twoYearsAgo(),
        new Date().toISOString().slice(0, 10),
      ),
  });

  const invalidateAll = () => {
    for (const k of [
      "account-transactions",
      "accounts",
      "items",
      "review-count",
      "review-inbox",
      "spending-summary",
      "spending-by-person",
      "transactions-table",
    ]) {
      qc.invalidateQueries({ queryKey: [k] });
    }
  };

  const shared = useMutation({
    mutationFn: (v: boolean) => api.setAccountShared(account.id, v),
    onSuccess: invalidateAll,
  });
  const hidden = useMutation({
    mutationFn: (v: boolean) => api.setAccountHidden(account.id, v),
    onSuccess: invalidateAll,
  });
  const reset = useMutation({
    mutationFn: () => api.resetAccountAttributions(account.id),
    onSuccess: () => {
      setConfirmReset(false);
      invalidateAll();
    },
  });

  const isDebt = ["credit", "loan"].includes(account.account_type);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onClick={onClose}
    >
      <div
        className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-[var(--border)] bg-[var(--card)]"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-start justify-between gap-4 border-b border-[var(--border)] p-5">
          <div>
            <div className="text-xs text-[var(--muted)]">
              {account.institution_name}
            </div>
            <h2 className="text-base font-semibold">
              {account.name}
              {account.mask && (
                <span className="text-[var(--muted)]"> ••{account.mask}</span>
              )}
            </h2>
            <div className="text-xs capitalize text-[var(--muted)]">
              {account.subtype ?? account.account_type}
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-[var(--muted)] hover:text-[var(--fg)]"
          >
            ✕
          </button>
        </header>

        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-[var(--border)] p-5">
          <div className="flex gap-6 text-sm">
            <div>
              <div className="text-xs text-[var(--muted)]">Balance</div>
              <div className={`font-semibold ${isDebt ? "text-red-500" : ""}`}>
                {money(account.current_balance, account.currency)}
              </div>
            </div>
            {account.available_balance != null && (
              <div>
                <div className="text-xs text-[var(--muted)]">Available</div>
                <div className="font-semibold">
                  {money(account.available_balance, account.currency)}
                </div>
              </div>
            )}
            {account.credit_limit != null && (
              <div>
                <div className="text-xs text-[var(--muted)]">Limit</div>
                <div className="font-semibold">
                  {money(account.credit_limit, account.currency)}
                </div>
              </div>
            )}
          </div>
          <div className="flex flex-col gap-1">
            <Toggle
              label="Shared"
              checked={account.is_shared}
              onChange={(v) => shared.mutate(v)}
            />
            <Toggle
              label="Hidden"
              checked={account.is_hidden}
              onChange={(v) => hidden.mutate(v)}
            />
          </div>
        </div>

        {(hist.data?.length ?? 0) >= 2 && (
          <div className="border-b border-[var(--border)] px-5 py-3">
            <ValueAreaChart data={hist.data ?? []} height={140} />
          </div>
        )}

        <div className="border-b border-[var(--border)] p-5">
          {!confirmReset ? (
            <Button variant="ghost" onClick={() => setConfirmReset(true)}>
              Reset attributions
            </Button>
          ) : (
            <div className="space-y-2">
              <Banner tone="info">
                Clears every Keep / Assign / Exclude decision on this account.
                {account.is_shared
                  ? " All spending charges go back to the review inbox."
                  : " Everything reverts to yours."}
              </Banner>
              <div className="flex gap-2">
                <Button
                  variant="danger"
                  disabled={reset.isPending}
                  onClick={() => reset.mutate()}
                >
                  Reset
                </Button>
                <Button variant="ghost" onClick={() => setConfirmReset(false)}>
                  Cancel
                </Button>
              </div>
            </div>
          )}
          {reset.isError && (
            <p className="mt-2 text-xs text-red-500">
              {errorMessage(reset.error)}
            </p>
          )}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          <h3 className="mb-2 text-xs font-semibold text-[var(--muted)]">
            Transactions {txns.data ? `(${txns.data.total_count})` : ""}
          </h3>
          <table className="w-full text-sm">
            <tbody>
              {txns.data?.rows.map((t) => (
                <tr key={t.id} className="border-t border-[var(--border)]">
                  <td className="py-1.5 pr-3 tabular-nums text-[var(--muted)]">
                    {t.posted_date}
                  </td>
                  <td className="max-w-[200px] truncate py-1.5 pr-3">
                    {t.merchant_name || t.description}
                  </td>
                  <td className="py-1.5 pr-3 text-xs text-[var(--muted)]">
                    {attribution(t)}
                  </td>
                  <td className="py-1.5 pl-3 text-right font-medium tabular-nums">
                    {money(t.amount, t.currency)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {txns.data?.rows.length === 0 && (
            <p className="py-4 text-sm text-[var(--muted)]">
              No transactions synced yet.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
