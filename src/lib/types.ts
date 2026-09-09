export type VaultState = "uninitialized" | "locked" | "unlocked";

export interface VaultStatus {
  state: VaultState;
}

export interface SetupStatus {
  onboarding_complete: boolean;
  plaid_configured: boolean;
  snaptrade_configured: boolean;
  llm_provider: string;
  llm_configured: boolean;
}

export interface CredentialStatus {
  name: string;
  present: boolean;
}

export interface Settings {
  plaid_env: string;
  llm_provider: "anthropic" | "ollama" | "none";
  anthropic_model: string;
  ollama_url: string;
  ollama_model: string;
  news_provider: "finnhub" | "marketaux" | "none";
  auto_confirm_high_confidence: boolean;
}

export interface ItemView {
  id: string;
  institution_name: string | null;
  status: string;
  error_message: string | null;
  last_synced_at: string | null;
  account_count: number;
}

export interface AccountView {
  id: string;
  item_id: string;
  institution_name: string | null;
  name: string;
  official_name: string | null;
  mask: string | null;
  account_type: string;
  subtype: string | null;
  currency: string;
  current_balance: number | null;
  available_balance: number | null;
  credit_limit: number | null;
  is_shared: boolean;
  is_hidden: boolean;
}

export interface LinkedItem {
  item_id: string;
  institution_name: string;
  accounts_added: number;
}

export interface SyncSummary {
  item_id: string;
  accounts_updated: number;
  transactions_added: number;
  transactions_modified: number;
  transactions_removed: number;
  transactions_pending: boolean;
}

export interface SpendingFilter {
  from: string;
  to: string;
  account_ids?: string[] | null;
  person_id?: string | null;
}

export interface MonthTotal {
  month: string;
  total: number;
}

export interface Bucket {
  id: string;
  label: string;
  total: number;
  count: number;
}

export interface SpendingSummary {
  total: number;
  txn_count: number;
  by_month: MonthTotal[];
  by_category: Bucket[];
}

export interface ChildRow {
  id: string;
  label: string;
  total: number;
  count: number;
  is_leaf: boolean;
}

export interface TxnRow {
  id: string;
  posted_date: string;
  amount: number;
  currency: string;
  description: string;
  merchant_name: string | null;
  category_id: string | null;
  category_label: string | null;
  account_name: string;
  review_status: string;
  owner_person_id: string | null;
  pending: boolean;
}

export interface TxnPage {
  rows: TxnRow[];
  total_count: number;
}

export interface TxnOpts {
  category_id?: string | null;
  merchant?: string | null;
  search?: string | null;
  limit?: number | null;
  offset?: number | null;
}

export interface CategoryRow {
  id: string;
  parent_id: string | null;
  label: string;
}

export interface Person {
  id: string;
  name: string;
  is_self: boolean;
  color: string | null;
  created_at: string;
}

export interface AppError {
  kind: string;
  message: string;
}

export const CREDENTIAL_LABELS: Record<string, string> = {
  "plaid.client_id": "Plaid Client ID",
  "plaid.secret": "Plaid Secret",
  "snaptrade.client_id": "SnapTrade Client ID",
  "snaptrade.consumer_key": "SnapTrade Consumer Key",
  "anthropic.api_key": "Anthropic API Key",
  "finnhub.api_key": "Finnhub API Key",
  "marketaux.api_key": "Marketaux API Key",
};
