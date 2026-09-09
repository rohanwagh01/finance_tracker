import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
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

  return (
    <>
      <h1 className="mb-6 text-xl font-semibold">Dashboard</h1>

      <div className="grid gap-5 md:grid-cols-2">
        <Card title="Connections">
          <StatusRow label="Plaid (banks & cards)" ok={!!setup.data?.plaid_configured} />
          <StatusRow label="SnapTrade (brokerages)" ok={!!setup.data?.snaptrade_configured} />
          <StatusRow
            label={`Research LLM (${setup.data?.llm_provider ?? "none"})`}
            ok={!!setup.data?.llm_configured}
          />
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

      <div className="mt-5">
        <Banner tone="info">
          Accounts aren't linked yet. Plaid Link and SnapTrade connection flows
          arrive in the next milestones; for now you can manage credentials and
          people under <strong>Settings</strong>.
        </Banner>
      </div>
    </>
  );
}
