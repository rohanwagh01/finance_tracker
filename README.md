# Finance Tracker

A local, read-only personal finance desktop app (Tauri + React + SQLite).
Three areas: **Spending**, **Investments** (with market research), and **Net Worth**.

## Privacy model

- Runs entirely on your machine. There is no server and no account.
- **Everything is encrypted at rest.** The whole database is SQLCipher-encrypted
  (AES-256); the key is derived from a master password you set (Argon2id) and is
  never written to disk. You enter it every time the app starts — nothing,
  including API keys and Plaid tokens (stored in a table inside that DB), is
  readable without it.
- **There is no password recovery.** No server can reset it. If you forget it the
  data is unrecoverable; the app offers "Reset app" which wipes the database and
  starts over.
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

On first launch you set a **master password** (it encrypts the local database),
then an onboarding wizard collects your API keys.

If you ran an earlier build that used the macOS Keychain, you can clear the old
entries: `security delete-generic-password -s com.financetracker.app` (repeat
until it says "not found").

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
- **Encryption** — replaced the OS keychain with SQLCipher full-database
  encryption gated by a master password at every launch.
- **Milestone 3** — the **Spending** page: date-range picker, KPIs, monthly bar
  chart, expandable category → subcategory → merchant → transaction breakdown,
  and a searchable transaction table with inline recategorization.
- **Milestone 4** — attribution: the **Review** inbox (shared-card charges you
  Keep / Assign / Exclude, with bulk actions and a nav badge), a rules engine
  that pre-fills the suggested person (contains / equals / regex / amount
  comparisons, optional auto-confirm for high-confidence rules), and per-person
  spending on the Spending page.
- **Milestone 5** — Investments through **Plaid Investments** (`/investments/holdings/get`):
  brokerage holdings sync alongside transactions, and the **Investments** page —
  portfolio value / cost basis / gain, combined and per-account holdings tables
  with allocation. A **"pull history since"** date in Settings sets the Plaid
  transaction window (up to Plaid's ~2-year max).

- **Milestone 6** — over-time charts from the daily `value_snapshots`: net-worth
  history on the Dashboard, portfolio value on Investments, a per-account value
  chart in the account detail modal, and a **"By category"** stacked view on the
  Spending monthly chart. History carries the last snapshot forward over
  un-synced days.
- **Milestone 7** — the **Net Worth** page: net-worth-over-time chart with a
  range picker, asset allocation breakdown, total assets vs. debt, and a monthly
  cash-flow / savings-rate card. Manually-entered **assets** (car, home, cash
  held elsewhere — with optional straight-line depreciation) and **liabilities**
  (mortgage, loans) with full add/edit/delete, folded into net worth and its
  history from each item's `as_of` date. **Recurring payments & income**: manual
  entry plus a **"Detect from transactions"** scan that clusters repeating
  merchant + amount patterns into weekly/biweekly/monthly/quarterly/yearly
  cadences, with monthly-equivalent totals.

Next: research (8), packaging (9).
