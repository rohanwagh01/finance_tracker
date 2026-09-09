import { lazy, Suspense } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { NavLink, Navigate, Route, Routes } from "react-router-dom";
import { api } from "./lib/api";
import Onboarding from "./routes/Onboarding";
import Vault from "./routes/Vault";
import Dashboard from "./routes/Dashboard";

// Secondary routes are code-split so the initial bundle stays small.
const Accounts = lazy(() => import("./routes/Accounts"));
const Spending = lazy(() => import("./routes/Spending"));
const Review = lazy(() => import("./routes/Review"));
const Investments = lazy(() => import("./routes/Investments"));
const NetWorth = lazy(() => import("./routes/NetWorth"));
const Research = lazy(() => import("./routes/Research"));
const SettingsPage = lazy(() => import("./routes/Settings"));

const NAV = [
  { to: "/", label: "Dashboard", end: true },
  { to: "/accounts", label: "Accounts" },
  { to: "/spending", label: "Spending" },
  { to: "/review", label: "Review", badge: true },
  { to: "/investments", label: "Investments" },
  { to: "/net-worth", label: "Net Worth" },
  { to: "/research", label: "Research" },
  { to: "/settings", label: "Settings" },
];

export default function App() {
  const qc = useQueryClient();
  const vault = useQuery({ queryKey: ["vault-status"], queryFn: api.vaultStatus });

  if (vault.isLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--muted)]">
        Loading…
      </div>
    );
  }

  const vstate = vault.data?.state ?? "locked";
  if (vstate !== "unlocked") {
    return (
      <Vault
        mode={vstate}
        onUnlocked={() => {
          qc.clear();
          vault.refetch();
        }}
      />
    );
  }

  return <AppShell />;
}

function AppShell() {
  const setup = useQuery({
    queryKey: ["setup-status"],
    queryFn: api.getSetupStatus,
  });
  const reviewCount = useQuery({
    queryKey: ["review-count"],
    queryFn: api.reviewCount,
    refetchInterval: 30_000,
  });

  if (setup.isLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--muted)]">
        Loading…
      </div>
    );
  }

  if (setup.isError) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-sm text-red-500">
        Failed to start: {String(setup.error)}
      </div>
    );
  }

  if (!setup.data?.onboarding_complete) {
    return <Onboarding onDone={() => setup.refetch()} />;
  }

  return (
    <div className="grid h-full grid-cols-[220px_1fr]">
      <aside className="flex flex-col gap-1 border-r border-[var(--border)] bg-[var(--card)] p-3">
        <div className="px-3 py-3 text-sm font-semibold tracking-tight">
          Finance Tracker
        </div>
        {NAV.map((n) => (
          <NavLink
            key={n.to}
            to={n.to}
            end={n.end}
            className={({ isActive }) =>
              `flex items-center justify-between rounded-lg px-3 py-2 text-sm transition ${
                isActive
                  ? "bg-[var(--accent)] text-white"
                  : "text-[var(--fg)] hover:bg-black/5 dark:hover:bg-white/5"
              }`
            }
          >
            <span>{n.label}</span>
            {n.badge && (reviewCount.data ?? 0) > 0 && (
              <span className="rounded-full bg-red-500 px-1.5 text-[10px] font-semibold text-white">
                {reviewCount.data}
              </span>
            )}
          </NavLink>
        ))}
      </aside>

      <main className="overflow-y-auto p-8">
        <div className="mx-auto max-w-5xl">
          <Suspense
            fallback={
              <div className="pt-10 text-center text-sm text-[var(--muted)]">
                Loading…
              </div>
            }
          >
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/accounts" element={<Accounts />} />
              <Route path="/spending" element={<Spending />} />
              <Route path="/review" element={<Review />} />
              <Route path="/investments" element={<Investments />} />
              <Route path="/net-worth" element={<NetWorth />} />
              <Route path="/research" element={<Research />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Suspense>
        </div>
      </main>
    </div>
  );
}
