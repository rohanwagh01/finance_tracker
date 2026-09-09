import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
} from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

/** An external link that opens in the user's real browser (not the webview). */
export function ExtLink({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={() => openUrl(href)}
      className="text-[var(--accent)] underline"
    >
      {children}
    </button>
  );
}

/** Collapsible "how do I get this?" panel for setup steps. */
export function Help({
  title = "How do I get this?",
  children,
}: {
  title?: string;
  children: ReactNode;
}) {
  return (
    <details className="rounded-lg border border-[var(--border)] bg-[var(--bg)]">
      <summary className="cursor-pointer list-none px-3 py-2 text-xs font-medium text-[var(--muted)] marker:content-none hover:text-[var(--fg)] [&::-webkit-details-marker]:hidden">
        {title}
      </summary>
      <div className="space-y-2 border-t border-[var(--border)] px-3 py-3 text-xs leading-relaxed text-[var(--muted)]">
        {children}
      </div>
    </details>
  );
}

export function Card({
  title,
  children,
  actions,
}: {
  title?: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <section className="rounded-xl border border-[var(--border)] bg-[var(--card)] p-5">
      {(title || actions) && (
        <header className="mb-4 flex items-center justify-between gap-3">
          {title && <h2 className="text-sm font-semibold">{title}</h2>}
          {actions}
        </header>
      )}
      {children}
    </section>
  );
}

export function Button({
  variant = "primary",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "danger";
}) {
  const styles = {
    primary:
      "bg-[var(--accent)] text-white hover:opacity-90 disabled:opacity-50",
    ghost:
      "border border-[var(--border)] bg-transparent hover:bg-black/5 dark:hover:bg-white/5",
    danger: "bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
  }[variant];
  return (
    <button
      className={`inline-flex items-center justify-center gap-2 rounded-lg px-3.5 py-2 text-sm font-medium transition disabled:cursor-not-allowed ${styles} ${className}`}
      {...props}
    />
  );
}

export function TextField({
  label,
  hint,
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  hint?: ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--muted)]">
        {label}
      </span>
      <input
        className={`w-full rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)] ${className}`}
        {...props}
      />
      {hint && (
        <span className="mt-1 block text-xs text-[var(--muted)]">{hint}</span>
      )}
    </label>
  );
}

export function Select({
  label,
  children,
  value,
  onChange,
}: {
  label: string;
  children: ReactNode;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-medium text-[var(--muted)]">
        {label}
      </span>
      <select
        className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)]"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {children}
      </select>
    </label>
  );
}

export function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-3 text-sm">
      <span
        onClick={() => onChange(!checked)}
        className={`relative h-5 w-9 rounded-full transition ${
          checked ? "bg-[var(--accent)]" : "bg-[var(--border)]"
        }`}
      >
        <span
          className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition ${
            checked ? "left-4" : "left-0.5"
          }`}
        />
      </span>
      {label}
    </label>
  );
}

export function Banner({
  tone = "info",
  children,
}: {
  tone?: "info" | "success" | "error";
  children: ReactNode;
}) {
  const styles = {
    info: "border-[var(--border)] bg-[var(--bg)] text-[var(--fg)]",
    success:
      "border-green-500/30 bg-green-500/10 text-green-700 dark:text-green-300",
    error: "border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300",
  }[tone];
  return (
    <div className={`rounded-lg border px-3.5 py-2.5 text-sm ${styles}`}>
      {children}
    </div>
  );
}

export function Placeholder({ title, note }: { title: string; note: string }) {
  return (
    <div className="rounded-xl border border-dashed border-[var(--border)] bg-[var(--card)] p-10 text-center">
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="mx-auto mt-2 max-w-md text-sm text-[var(--muted)]">{note}</p>
    </div>
  );
}
