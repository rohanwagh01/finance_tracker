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
- The only outbound traffic is to a fixed allowlist: Plaid, your chosen news API,
  and (if enabled) the research LLM. Everything else is blocked in code
  (`src-tauri/src/http.rs`).
- All aggregator access is **read-only** — balances, transactions, holdings.
- The research feature sends **only tickers and rounded allocation percentages** —
  never balances, amounts, or share counts (`src-tauri/src/research/safe_context.rs`).
  An optional setting ("Let the research LLM see gain/loss %") additionally sends
  each holding's rounded gain/loss percent vs. cost basis; it's off by default and
  still never sends dollar figures.

## Install

**Easiest — download a build.** Grab the installer for your OS from the
[Releases page](../../releases): `.dmg` (macOS), `.msi` (Windows), `.AppImage`
(Linux). The builds are not code-signed, so the first launch needs one extra click:

- **macOS**: right-click the app → **Open** → **Open** (only the first time). Or
  run `xattr -dr com.apple.quarantine "/Applications/Finance Tracker.app"`.
- **Windows**: "Windows protected your PC" → **More info** → **Run anyway**.

**Or build from source:**

```bash
# prerequisites: Rust (https://rustup.rs), Node 18+ , and on macOS: xcode-select --install
npm install
npm run tauri build    # installer lands in src-tauri/target/release/bundle/
npm run tauri dev      # or run in development
```

> On macOS, run `npm run tauri build` from a normal Terminal in a logged-in
> session — the `.dmg` step drives Finder via AppleScript to lay out the window.
> If it fails there (e.g. over SSH or in a sandbox), the `.app` in
> `src-tauri/target/release/bundle/macos/` is still valid, and the GitHub Actions
> release workflow builds the `.dmg` regardless.

The app icon is generated from `src-tauri/icons/source-icon.svg` via
`npm run tauri icon`.

On first launch you set a **master password** (it encrypts the local database),
then an onboarding wizard collects your API keys. Every step in the wizard has a
**"How do I get this?"** section with click-by-click instructions.

### API keys — quick reference

The wizard explains each of these in detail; this is the summary.

| Service | Needed for | How |
|---|---|---|
| **Plaid** | banks & cards (required) | Free account at [dashboard.plaid.com](https://dashboard.plaid.com/signup) → Developers → Keys. Start in **Sandbox** (fake data, log in with `user_good` / `pass_good`); switch to **Production** for real accounts (self-serve, up to 100 institutions). |
| **Anthropic** *or* **Ollama** | research LLM (optional) | Claude: key from [console.anthropic.com](https://console.anthropic.com/) + a few $ of credit — best quality, any machine. Ollama: install from [ollama.com](https://ollama.com/download), `ollama pull llama3.1` — free and fully local, needs a capable machine. |
| **Finnhub** | market news (optional) | Free key, shown on the dashboard right after you register at [finnhub.io/register](https://finnhub.io/register). |

Each person who uses the app brings **their own** keys — there is no shared
account or server, and one person's data is never visible to anyone else.

## Development

```bash
cd src-tauri && cargo test      # Rust unit tests (http allowlist, privacy guard, migrations)
npm run build                   # type-check + bundle the frontend
```

## Status

- **Milestone 1** — app shell, onboarding, settings, credential storage, SQLite
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
- **Milestone 8** — the **Research** page. A watchlist of extra tickers; a
  "what leaves your machine" panel showing the exact outbound payload (ticker +
  rounded allocation % only — enforced by `research::safe_context`, the single
  path from portfolio data to any external service); per-ticker market news from
  Finnhub or Marketaux (cached per day); and an LLM analysis run (Anthropic
  Claude or local Ollama, both asked for strict JSON) that synthesises the
  headlines, calls out concentration risk, and suggests companies to look at.
  Reports are saved locally; the newest shows on load. A follow-up box lets you
  ask questions that revise the analysis in place. An optional setting shares
  rounded gain/loss % (never dollars). The `snaptrade.com` host was dropped from
  the outbound allowlist.
- **Milestone 9** — packaging & polish: app icon, richer bundle metadata,
  a GitHub Actions release workflow (`tauri-action`, macOS/Windows/Linux
  installers on tag), route-level code-splitting, expandable setup-wizard help,
  and first-run empty states. Distribution: unsigned installers from GitHub
  Releases (documented right-click-Open step), or build from source.

All nine milestones are complete.
