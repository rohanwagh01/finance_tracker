import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import { money } from "../lib/format";
import { Banner, Card } from "../components/ui";

function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div className="flex items-center justify-between py-1.5 text-sm">
      <span>{label}</span>
      <span className={ok ? "text-green-600 dark:text-green-400" : "text-[var(--muted)]"}>
        {ok ? "Connected" : "Not set up"}
      </span>
    </div>
  );
}

export default function Dashboard() {
  const setup = useQuery({ queryKey: ["setup-status"], queryFn: api.getSetupStatus });
  const people = useQuery({ queryKey: ["people"], queryFn: api.listPeople });
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: api.listAccounts });
  const reviewCount = useQuery({ queryKey: ["review-count"], queryFn: api.reviewCount });

  const visible = (accounts.data ?? []).filter((a) => !a.is_hidden);
  const cash = visible
    .filter((a) => a.account_type === "depository")
    .reduce((s, a) => s + (a.current_balance ?? 0), 0);
  const debt = visible
    .filter((a) => ["credit", "loan"].includes(a.account_type))
    .reduce((s, a) => s + (a.current_balance ?? 0), 0);

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold">Dashboard</h1>

      <div className="grid gap-5 md:grid-cols-2">
        <Card title="Connections">
          <StatusRow label="Plaid (banks, cards, brokerages)" ok={!!setup.data?.plaid_configured} />
          <StatusRow
            label={`Research LLM (${setup.data?.llm_provider ?? "none"})`}
            ok={!!setup.data?.llm_configured}
          />
        </Card>

        <Card title="Balances">
          {visible.length === 0 ? (
            <p className="text-sm text-[var(--muted)]">
              No linked accounts. <Link className="underline" to="/accounts">Link one →</Link>
            </p>
          ) : (
            <div className="space-y-1.5 text-sm">
              <div className="flex justify-between">
                <span>Cash (checking + savings)</span>
                <span className="font-semibold">{money(cash)}</span>
              </div>
              <div className="flex justify-between">
                <span>Card & loan balances</span>
                <span className="font-semibold text-red-500">{money(debt)}</span>
              </div>
              <div className="mt-1 flex justify-between border-t border-[var(--border)] pt-1.5">
                <span>Net</span>
                <span className="font-semibold">{money(cash - debt)}</span>
              </div>
            </div>
          )}
        </Card>

        <Card title="People">
          <p className="text-sm text-[var(--muted)]">
            {people.data?.length ?? 0} configured for shared-card attribution.
          </p>
          <ul className="mt-2 flex flex-wrap gap-2">
            {people.data?.map((p) => (
              <li
                key={p.id}
                className="rounded-full border border-[var(--border)] px-2.5 py-1 text-xs"
                style={p.color ? { borderColor: p.color } : undefined}
              >
                {p.name}
                {p.is_self ? " (you)" : ""}
              </li>
            ))}
          </ul>
        </Card>
      </div>

      {(reviewCount.data ?? 0) > 0 && (
        <div className="mt-5">
          <Banner tone="info">
            <Link className="underline" to="/review">
              {reviewCount.data} charge{reviewCount.data === 1 ? "" : "s"} waiting
              in Review
            </Link>{" "}
            — decide whose spending they are.
          </Banner>
        </div>
      )}

      {visible.length === 0 && (
        <div className="mt-5">
          <Banner tone="info">
            Head to <strong>Accounts</strong> to link your bank and cards through Plaid.
          </Banner>
        </div>
      )}
    </>
  );
}
