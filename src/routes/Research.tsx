import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Link } from "react-router-dom";
import { api, errorMessage } from "../lib/api";
import { relativeTime } from "../lib/format";
import { Banner, Button, Card } from "../components/ui";
import Markdown, { MarkdownInline } from "../components/Markdown";
import type { NewsItem, ResearchReport } from "../lib/types";

function NewsRow({ n }: { n: NewsItem }) {
  return (
    <button
      onClick={() => openUrl(n.url)}
      className="block w-full border-t border-[var(--border)] py-2.5 text-left first:border-t-0"
    >
      <div className="text-sm font-medium hover:underline">{n.headline}</div>
      <div className="mt-0.5 text-xs text-[var(--muted)]">
        {n.source || "news"} · {n.published_at}
      </div>
      {n.summary && (
        <p className="mt-1 line-clamp-2 text-xs text-[var(--muted)]">
          {n.summary}
        </p>
      )}
    </button>
  );
}

function Analysis({
  report,
  onWatch,
  onAsk,
  asking,
  askError,
}: {
  report: ResearchReport;
  onWatch: (t: string) => void;
  onAsk: (q: string) => void;
  asking: boolean;
  askError: unknown;
}) {
  const a = report.analysis;
  const [q, setQ] = useState("");
  const submit = () => {
    if (q.trim()) {
      onAsk(q.trim());
      setQ("");
    }
  };
  return (
    <Card
      title="Analysis"
      actions={
        <span className="text-xs text-[var(--muted)]">
          {report.provider}/{report.model} · {relativeTime(report.created_at)}
        </span>
      }
    >
      {report.prompt && (
        <p className="mb-3 border-l-2 border-[var(--accent)] pl-3 text-sm text-[var(--muted)]">
          Follow-up: {report.prompt}
        </p>
      )}

      <Markdown text={a.summary} />

      {a.concentration_risk && (
        <div className="mt-4">
          <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
            Concentration risk
          </h3>
          <Markdown text={a.concentration_risk} />
        </div>
      )}

      {a.per_holding.length > 0 && (
        <div className="mt-4">
          <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
            By holding
          </h3>
          <ul className="space-y-1.5 text-sm">
            {a.per_holding.map((h) => (
              <li key={h.ticker}>
                <span className="font-semibold">{h.ticker}</span> —{" "}
                <MarkdownInline text={h.note} />
              </li>
            ))}
          </ul>
        </div>
      )}

      {a.ideas.length > 0 && (
        <div className="mt-4">
          <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
            Ideas to research
          </h3>
          <ul className="space-y-2 text-sm">
            {a.ideas.map((idea) => (
              <li
                key={idea.ticker}
                className="flex items-start justify-between gap-3"
              >
                <span>
                  <span className="font-semibold">{idea.ticker}</span>
                  {idea.name ? ` · ${idea.name}` : ""} —{" "}
                  <MarkdownInline text={idea.rationale} />
                </span>
                <button
                  className="shrink-0 text-xs text-[var(--accent)] underline"
                  onClick={() => onWatch(idea.ticker)}
                >
                  Watch
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      <p className="mt-4 text-[11px] text-[var(--muted)]">
        Generated from tickers and rounded allocation percentages only. Not
        personalized financial advice.
      </p>

      <div className="mt-4 border-t border-[var(--border)] pt-4">
        <div className="flex gap-2">
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            disabled={asking}
            placeholder="Ask a follow-up — e.g. “How exposed am I to rate cuts?”"
            className="flex-1 rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)] disabled:opacity-60"
          />
          <Button disabled={asking || !q.trim()} onClick={submit}>
            {asking ? "Thinking…" : "Ask"}
          </Button>
        </div>
        <p className="mt-1.5 text-[11px] text-[var(--muted)]">
          The answer replaces this analysis. Same privacy rules — only tickers and
          your question are sent.
        </p>
        {askError != null && (
          <p className="mt-2 text-sm text-red-500">{errorMessage(askError)}</p>
        )}
      </div>
    </Card>
  );
}

export default function Research() {
  const qc = useQueryClient();
  const [ticker, setTicker] = useState("");

  const setup = useQuery({ queryKey: ["setup-status"], queryFn: api.getSetupStatus });
  const ctx = useQuery({
    queryKey: ["research-context"],
    queryFn: api.researchSafeContext,
  });
  const watchlist = useQuery({
    queryKey: ["research-watchlist"],
    queryFn: api.researchWatchlist,
  });
  const latest = useQuery({
    queryKey: ["research-latest"],
    queryFn: api.researchLatest,
  });
  const news = useQuery({
    queryKey: ["research-news"],
    queryFn: () => api.researchNews(false),
  });

  const invalidateCtx = () => {
    qc.invalidateQueries({ queryKey: ["research-context"] });
    qc.invalidateQueries({ queryKey: ["research-watchlist"] });
  };

  const addWatch = useMutation({
    mutationFn: (t: string) => api.researchWatchlistAdd(t),
    onSuccess: () => {
      setTicker("");
      invalidateCtx();
      qc.invalidateQueries({ queryKey: ["research-news"] });
    },
  });
  const removeWatch = useMutation({
    mutationFn: (t: string) => api.researchWatchlistRemove(t),
    onSuccess: invalidateCtx,
  });
  const refreshNews = useMutation({
    mutationFn: () => api.researchNews(true),
    onSuccess: (data) => qc.setQueryData(["research-news"], data),
  });
  const run = useMutation({
    mutationFn: () => api.researchRun(),
    onSuccess: (report) => {
      qc.setQueryData(["research-latest"], report);
      qc.setQueryData(["research-news"], report.news);
    },
  });
  const followup = useMutation({
    mutationFn: (q: string) => api.researchFollowup(q),
    onSuccess: (report) => qc.setQueryData(["research-latest"], report),
  });

  const llmReady = !!setup.data?.llm_configured;
  const holdings = ctx.data?.holdings ?? [];
  const wl = watchlist.data ?? [];
  const report = followup.data ?? run.data ?? latest.data ?? null;

  const newsByTicker = (news.data ?? []).reduce<Record<string, NewsItem[]>>(
    (acc, n) => {
      (acc[n.ticker] ??= []).push(n);
      return acc;
    },
    {},
  );

  return (
    <>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">Research</h1>
        <Button
          disabled={run.isPending || !llmReady || (holdings.length === 0 && wl.length === 0)}
          onClick={() => run.mutate()}
        >
          {run.isPending ? "Analyzing…" : "Run analysis"}
        </Button>
      </div>

      {!llmReady && (
        <div className="mb-5">
          <Banner tone="info">
            Choose a research LLM (Claude API or local Ollama) in{" "}
            <Link className="underline" to="/settings">
              Settings
            </Link>{" "}
            to generate an analysis. News below works with just a Finnhub or
            Marketaux key.
          </Banner>
        </div>
      )}

      {run.isError && (
        <div className="mb-5">
          <Banner tone="error">{errorMessage(run.error)}</Banner>
        </div>
      )}

      <div className="grid gap-5 lg:grid-cols-2">
        <Card title="What leaves your machine">
          <p className="mb-3 text-xs text-[var(--muted)]">
            Only these ticker symbols and whole-number allocation percentages
            {holdings.some((h) => h.gain_pct != null)
              ? " (plus rounded gain/loss %, which you enabled in Settings)"
              : ""}{" "}
            are sent to the news APIs and the research LLM — never balances, share
            counts, or account names.
          </p>
          {holdings.length === 0 ? (
            <p className="text-sm text-[var(--muted)]">
              No investment holdings synced yet.{" "}
              <Link className="underline" to="/accounts">
                Link a brokerage →
              </Link>
            </p>
          ) : (
            <ul className="space-y-1.5 text-sm">
              {holdings.map((h) => (
                <li key={h.ticker} className="flex justify-between">
                  <span className="font-medium">
                    {h.ticker}
                    {h.sector ? (
                      <span className="text-[var(--muted)]"> · {h.sector}</span>
                    ) : null}
                  </span>
                  <span className="flex gap-2 text-[var(--muted)]">
                    {h.gain_pct != null && (
                      <span
                        className={
                          h.gain_pct >= 0
                            ? "text-green-600 dark:text-green-400"
                            : "text-red-500"
                        }
                      >
                        {h.gain_pct >= 0 ? "+" : ""}
                        {h.gain_pct}%
                      </span>
                    )}
                    <span>{h.allocation_pct}%</span>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Watchlist">
          <div className="mb-3 flex gap-2">
            <input
              value={ticker}
              onChange={(e) => setTicker(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && ticker.trim())
                  addWatch.mutate(ticker.trim());
              }}
              placeholder="Add a ticker, e.g. NVDA"
              className="flex-1 rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm outline-none focus:border-[var(--accent)]"
            />
            <Button
              variant="ghost"
              disabled={!ticker.trim() || addWatch.isPending}
              onClick={() => addWatch.mutate(ticker.trim())}
            >
              Add
            </Button>
          </div>
          {wl.length === 0 ? (
            <p className="text-sm text-[var(--muted)]">
              Tickers you want news and ideas for, even if you don't hold them.
            </p>
          ) : (
            <ul className="flex flex-wrap gap-2">
              {wl.map((t) => (
                <li
                  key={t}
                  className="flex items-center gap-1.5 rounded-full border border-[var(--border)] px-2.5 py-1 text-xs"
                >
                  {t}
                  <button
                    className="text-[var(--muted)] hover:text-red-500"
                    onClick={() => removeWatch.mutate(t)}
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
          )}
          {(addWatch.isError || removeWatch.isError) && (
            <p className="mt-2 text-sm text-red-500">
              {errorMessage(addWatch.error ?? removeWatch.error)}
            </p>
          )}
        </Card>
      </div>

      {report && (
        <div className="mt-5">
          <Analysis
            report={report}
            onWatch={(t) => addWatch.mutate(t)}
            onAsk={(q) => followup.mutate(q)}
            asking={followup.isPending}
            askError={followup.isError ? followup.error : null}
          />
        </div>
      )}

      <div className="mt-5">
        <Card
          title="News"
          actions={
            <Button
              variant="ghost"
              disabled={refreshNews.isPending}
              onClick={() => refreshNews.mutate()}
            >
              {refreshNews.isPending ? "Refreshing…" : "Refresh"}
            </Button>
          }
        >
          {news.isLoading ? (
            <p className="text-sm text-[var(--muted)]">Loading news…</p>
          ) : (news.data ?? []).length === 0 ? (
            <p className="text-sm text-[var(--muted)]">
              {setup.data && !setup.data.plaid_configured
                ? "Link a brokerage and add a news API key in Settings to see headlines."
                : "No recent headlines for your tickers, or no news API key set in Settings."}
            </p>
          ) : (
            <div className="space-y-4">
              {Object.entries(newsByTicker).map(([t, items]) => (
                <div key={t}>
                  <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                    {t}
                  </h3>
                  <div>
                    {items.slice(0, 6).map((n, i) => (
                      <NewsRow key={`${n.url}-${i}`} n={n} />
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </>
  );
}
