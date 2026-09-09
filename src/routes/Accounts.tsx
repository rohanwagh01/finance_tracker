import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import type { AccountView, ItemView, SyncSummary } from "../lib/types";
import { money, relativeTime } from "../lib/format";
import { Banner, Button, Card, Toggle } from "../components/ui";
import AccountDetailModal from "../components/AccountDetailModal";

function StatusBadge({ status }: { status: string }) {
  const map: Record<string, string> = {
    active: "text-green-600 dark:text-green-400",
    error: "text-red-500",
    disconnected: "text-[var(--muted)]",
  };
  return <span className={`text-xs ${map[status] ?? ""}`}>{status}</span>;
}

function summaryLine(s: SyncSummary): string {
  if (s.transactions_pending && s.transactions_added === 0) {
    return "balances updated · transactions still preparing at Plaid";
  }
  const parts = [`${s.transactions_added} new`];
  if (s.transactions_modified) parts.push(`${s.transactions_modified} updated`);
  if (s.transactions_removed) parts.push(`${s.transactions_removed} removed`);
  return `${parts.join(" · ")} transaction${s.transactions_added === 1 ? "" : "s"}`;
}

function AccountRow({
  account,
  onChanged,
  onOpen,
}: {
  account: AccountView;
  onChanged: () => void;
  onOpen: () => void;
}) {
  const shared = useMutation({
    mutationFn: (v: boolean) => api.setAccountShared(account.id, v),
    onSuccess: onChanged,
  });
  const hidden = useMutation({
    mutationFn: (v: boolean) => api.setAccountHidden(account.id, v),
    onSuccess: onChanged,
  });
  const isDebt = ["credit", "loan"].includes(account.account_type);

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] py-3 first:border-t-0">
      <button className="min-w-0 text-left" onClick={onOpen}>
        <div className="text-sm font-medium hover:underline">
          {account.name}
          {account.mask && (
            <span className="text-[var(--muted)]"> ••{account.mask}</span>
          )}
        </div>
        <div className="text-xs text-[var(--muted)] capitalize">
          {account.subtype ?? account.account_type}
          {account.is_shared && " · shared"}
        </div>
      </button>

      <div className="flex items-center gap-5">
        <div className="text-right">
          <div
            className={`text-sm font-semibold ${
              isDebt ? "text-red-500" : ""
            }`}
          >
            {money(account.current_balance, account.currency)}
          </div>
          {account.credit_limit != null && (
            <div className="text-xs text-[var(--muted)]">
              limit {money(account.credit_limit, account.currency)}
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
    </div>
  );
}

function ItemCard({
  item,
  accounts,
  onChanged,
  onOpenAccount,
}: {
  item: ItemView;
  accounts: AccountView[];
  onChanged: () => void;
  onOpenAccount: (a: AccountView) => void;
}) {
  const [result, setResult] = useState<string | null>(null);
  const [confirmUnlink, setConfirmUnlink] = useState(false);
  const sync = useMutation({
    mutationFn: () => api.syncItem(item.id),
    onSuccess: (s) => {
      setResult(summaryLine(s));
      onChanged();
    },
    onError: (e) => setResult(errorMessage(e)),
  });
  const unlink = useMutation({
    mutationFn: () => api.unlinkItem(item.id),
    onSuccess: onChanged,
    onError: (e) => {
      setConfirmUnlink(false);
      setResult(`Unlink failed: ${errorMessage(e)}`);
    },
  });

  return (
    <Card
      title={
        <span className="flex items-center gap-2">
          {item.institution_name ?? "Institution"} <StatusBadge status={item.status} />
        </span>
      }
      actions={
        <div className="flex items-center gap-2">
          <Button variant="ghost" disabled={sync.isPending} onClick={() => sync.mutate()}>
            {sync.isPending ? "Syncing…" : "Sync"}
          </Button>
          {confirmUnlink ? (
            <>
              <Button
                variant="danger"
                disabled={unlink.isPending}
                onClick={() => unlink.mutate()}
              >
                {unlink.isPending ? "Removing…" : "Confirm remove"}
              </Button>
              <Button variant="ghost" onClick={() => setConfirmUnlink(false)}>
                Cancel
              </Button>
            </>
          ) : (
            <Button variant="ghost" onClick={() => setConfirmUnlink(true)}>
              Unlink
            </Button>
          )}
        </div>
      }
    >
      <div className="mb-2 text-xs text-[var(--muted)]">
        Last synced {relativeTime(item.last_synced_at)}
      </div>
      {item.error_message && <Banner tone="error">{item.error_message}</Banner>}
      {result && (
        <div className="mb-2 text-xs text-[var(--muted)]">{result}</div>
      )}
      <div>
        {accounts.map((a) => (
          <AccountRow
            key={a.id}
            account={a}
            onChanged={onChanged}
            onOpen={() => onOpenAccount(a)}
          />
        ))}
      </div>
    </Card>
  );
}

export default function Accounts() {
  const qc = useQueryClient();
  const setup = useQuery({ queryKey: ["setup-status"], queryFn: api.getSetupStatus });
  const items = useQuery({ queryKey: ["items"], queryFn: api.listItems });
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: api.listAccounts });

  const refresh = () => {
    for (const k of [
      "items",
      "accounts",
      "account-transactions",
      "account-value-history",
      "review-count",
      "review-inbox",
      "spending-summary",
      "spending-by-person",
      "spending-trends",
      "transactions-table",
      // hiding/sharing an account changes net worth, allocation, and the portfolio
      "net-worth-now",
      "net-worth-history",
      "asset-allocation",
      "portfolio",
      "portfolio-history",
    ]) {
      qc.invalidateQueries({ queryKey: [k] });
    }
  };

  const [openAccount, setOpenAccount] = useState<AccountView | null>(null);
  const [linkToken, setLinkToken] = useState<string | null>(null);
  const [linkMsg, setLinkMsg] = useState<string | null>(null);
  const pollCount = useRef(0);

  const startLink = useMutation({
    mutationFn: api.plaidLinkStart,
    onSuccess: ({ link_token }) => {
      pollCount.current = 0;
      setLinkMsg("Complete the connection in your browser — this updates automatically.");
      setLinkToken(link_token);
    },
    onError: (e) => setLinkMsg(errorMessage(e)),
  });

  useEffect(() => {
    if (!linkToken) return;
    const timer = setInterval(async () => {
      pollCount.current += 1;
      try {
        const { linked } = await api.plaidLinkPoll(linkToken);
        if (linked.length > 0) {
          setLinkToken(null);
          setLinkMsg(
            `Linked ${linked.map((l) => l.institution_name).join(", ")}.`,
          );
          refresh();
        }
      } catch (e) {
        setLinkToken(null);
        setLinkMsg(errorMessage(e));
      }
      if (pollCount.current > 60) {
        setLinkToken(null);
        setLinkMsg("Stopped checking. Click “Check now” if you finished linking.");
      }
    }, 3000);
    return () => clearInterval(timer);
  }, [linkToken]);

  const syncAll = useMutation({
    mutationFn: api.syncAll,
    onSuccess: () => refresh(),
  });

  if (setup.data && !setup.data.plaid_configured) {
    return (
      <>
        <h1 className="mb-6 text-xl font-semibold">Accounts</h1>
        <Banner tone="info">
          Add your Plaid credentials under <strong>Settings</strong> to link banks and cards.
        </Banner>
      </>
    );
  }

  const byItem = (itemId: string) =>
    (accounts.data ?? []).filter((a) => a.item_id === itemId);

  return (
    <>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-xl font-semibold">Accounts</h1>
        <div className="flex gap-2">
          {(items.data?.length ?? 0) > 0 && (
            <Button
              variant="ghost"
              disabled={syncAll.isPending}
              onClick={() => syncAll.mutate()}
            >
              {syncAll.isPending ? "Syncing…" : "Sync all"}
            </Button>
          )}
          <Button
            disabled={startLink.isPending || !!linkToken}
            onClick={() => startLink.mutate()}
          >
            {linkToken ? "Waiting…" : "Link account"}
          </Button>
        </div>
      </div>

      {linkMsg && (
        <div className="mb-4">
          <Banner tone={linkToken ? "info" : "success"}>
            <div className="flex items-center justify-between gap-3">
              <span>{linkMsg}</span>
              {linkToken && (
                <span className="flex gap-2">
                  <Button
                    variant="ghost"
                    onClick={async () => {
                      try {
                        const { linked } = await api.plaidLinkPoll(linkToken);
                        if (linked.length > 0) {
                          setLinkToken(null);
                          setLinkMsg(
                            `Linked ${linked.map((l) => l.institution_name).join(", ")}.`,
                          );
                          refresh();
                        }
                      } catch (e) {
                        setLinkToken(null);
                        setLinkMsg(errorMessage(e));
                      }
                    }}
                  >
                    Check now
                  </Button>
                  <Button variant="ghost" onClick={() => setLinkToken(null)}>
                    Cancel
                  </Button>
                </span>
              )}
            </div>
          </Banner>
        </div>
      )}

      {items.data?.length === 0 && !linkToken && (
        <Banner tone="info">
          No institutions linked yet. Click <strong>Link account</strong> to connect
          your bank or card through Plaid.
        </Banner>
      )}

      <div className="space-y-5">
        {items.data?.map((item) => (
          <ItemCard
            key={item.id}
            item={item}
            accounts={byItem(item.id)}
            onChanged={refresh}
            onOpenAccount={setOpenAccount}
          />
        ))}
      </div>

      {openAccount && (
        <AccountDetailModal
          account={
            (accounts.data ?? []).find((a) => a.id === openAccount.id) ??
            openAccount
          }
          onClose={() => setOpenAccount(null)}
        />
      )}
    </>
  );
}
