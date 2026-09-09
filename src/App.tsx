import { useQuery } from "@tanstack/react-query";
import { NavLink, Navigate, Route, Routes } from "react-router-dom";
import { api } from "./lib/api";
import Onboarding from "./routes/Onboarding";
import Dashboard from "./routes/Dashboard";
import Spending from "./routes/Spending";
import Investments from "./routes/Investments";
import NetWorth from "./routes/NetWorth";
import Research from "./routes/Research";
import SettingsPage from "./routes/Settings";

const NAV = [
  { to: "/", label: "Dashboard", end: true },
  { to: "/spending", label: "Spending" },
  { to: "/investments", label: "Investments" },
  { to: "/net-worth", label: "Net Worth" },
  { to: "/research", label: "Research" },
  { to: "/settings", label: "Settings" },
];

export default function App() {
  const setup = useQuery({
    queryKey: ["setup-status"],
    queryFn: api.getSetupStatus,
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
              `rounded-lg px-3 py-2 text-sm transition ${
                isActive
                  ? "bg-[var(--accent)] text-white"
                  : "text-[var(--fg)] hover:bg-black/5 dark:hover:bg-white/5"
              }`
            }
          >
            {n.label}
          </NavLink>
        ))}
      </aside>

      <main className="overflow-y-auto p-8">
        <div className="mx-auto max-w-5xl">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/spending" element={<Spending />} />
            <Route path="/investments" element={<Investments />} />
            <Route path="/net-worth" element={<NetWorth />} />
            <Route path="/research" element={<Research />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </div>
      </main>
    </div>
  );
}
