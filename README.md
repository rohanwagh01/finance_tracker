# Finance Tracker

A local, read-only personal finance desktop app (Tauri + React + SQLite).
Three areas: **Spending**, **Investments** (with market research), and **Net Worth**.

## Privacy model

- Runs entirely on your machine. Data lives in a local SQLite file; API keys live
  in the OS keychain.
- The only outbound traffic is to a fixed allowlist: Plaid, SnapTrade, your chosen
  news API, and (if enabled) the research LLM. Everything else is blocked in code
  (`src-tauri/src/http.rs`).
- All aggregator access is **read-only** — balances, transactions, holdings.
- The research feature sends **only tickers and rounded allocation percentages** —
  never balances, amounts, or share counts (`src-tauri/src/research/safe_context.rs`).

## Prerequisites

- **Rust** (stable) — https://rustup.rs
- **Node** 18+ — https://nodejs.org or `nvm install 22`
- **Xcode Command Line Tools** (macOS): `xcode-select --install`

## Setup

```bash
npm install
npm run tauri dev      # run in development
npm run tauri build    # produce a .dmg / .app (or .msi / .AppImage)
```

On first launch an onboarding wizard collects your API keys.

### Getting API keys

| Service | Needed for | Notes |
|---|---|---|
| **Plaid** | banks & cards | Sign up, use the free **Trial plan** (auto-approved, up to 10 items). Dashboard → Developers → Keys. |
| **SnapTrade** | Robinhood / E*Trade holdings | Optional. Free personal-use tier. |
| **Anthropic** or **Ollama** | research LLM | Optional. Claude API key, or run Ollama locally. |
| **Finnhub** | market news / earnings | Optional. Free tier. |

## Development

```bash
cd src-tauri && cargo test      # Rust unit tests (http allowlist, privacy guard, migrations)
npm run build                   # type-check + bundle the frontend
```

## Status

- **Milestone 1** — app shell, onboarding, settings, keychain credentials, SQLite
  schema + migrations, people management, HTTP allowlist, research privacy guard.
- **Milestone 2** — Plaid Hosted Link + polling, account/balance/transaction sync
  (cursor-based), institution linking/unlinking, per-account shared/hidden flags,
  the **Accounts** page. Shared-card charges auto-route to the review inbox.

Next: Spending UI (3), attribution review inbox (4), SnapTrade + portfolio (5),
over-time charts (6), net worth page (7), research (8).
