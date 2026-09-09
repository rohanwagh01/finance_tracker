import { invoke } from "@tauri-apps/api/core";
import type {
  AccountView,
  CategoryRow,
  ChildRow,
  CredentialStatus,
  ItemView,
  LinkedItem,
  Person,
  Settings,
  SetupStatus,
  SpendingFilter,
  SpendingSummary,
  SyncSummary,
  TxnOpts,
  TxnPage,
  VaultStatus,
} from "./types";

export const api = {
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  vaultInitialize: (password: string) =>
    invoke<void>("vault_initialize", { password }),
  vaultUnlock: (password: string) => invoke<void>("vault_unlock", { password }),
  vaultLock: () => invoke<void>("vault_lock"),
  vaultChangePassword: (oldPassword: string, newPassword: string) =>
    invoke<void>("vault_change_password", { oldPassword, newPassword }),
  vaultReset: () => invoke<void>("vault_reset"),

  getSetupStatus: () => invoke<SetupStatus>("get_setup_status"),
  listCredentials: () => invoke<CredentialStatus[]>("list_credentials"),
  saveCredential: (name: string, value: string) =>
    invoke<void>("save_credential", { name, value }),
  deleteCredential: (name: string) =>
    invoke<void>("delete_credential", { name }),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) =>
    invoke<void>("update_settings", { settings }),
  testPlaidConnection: () => invoke<void>("test_plaid_connection"),
  completeOnboarding: () => invoke<void>("complete_onboarding"),

  plaidLinkStart: () => invoke<{ link_token: string }>("plaid_link_start"),
  plaidLinkPoll: (linkToken: string) =>
    invoke<{ linked: LinkedItem[] }>("plaid_link_poll", { linkToken }),
  listItems: () => invoke<ItemView[]>("list_items"),
  listAccounts: () => invoke<AccountView[]>("list_accounts"),
  syncItem: (itemId: string) => invoke<SyncSummary>("sync_item", { itemId }),
  syncAll: () => invoke<SyncSummary[]>("sync_all"),
  unlinkItem: (itemId: string) => invoke<void>("unlink_item", { itemId }),
  setAccountShared: (accountId: string, shared: boolean) =>
    invoke<void>("set_account_shared", { accountId, shared }),
  setAccountHidden: (accountId: string, hidden: boolean) =>
    invoke<void>("set_account_hidden", { accountId, hidden }),

  spendingSummary: (filter: SpendingFilter) =>
    invoke<SpendingSummary>("spending_summary", { filter }),
  spendingChildren: (filter: SpendingFilter, categoryId: string) =>
    invoke<ChildRow[]>("spending_children", { filter, categoryId }),
  listTransactions: (filter: SpendingFilter, opts?: TxnOpts) =>
    invoke<TxnPage>("list_transactions", { filter, opts: opts ?? null }),
  listCategories: () => invoke<CategoryRow[]>("list_categories"),
  setTransactionCategory: (txnId: string, categoryId: string | null) =>
    invoke<void>("set_transaction_category", { txnId, categoryId }),

  listPeople: () => invoke<Person[]>("list_people"),
  createPerson: (input: { name: string; color?: string | null }) =>
    invoke<Person>("create_person", { input }),
  updatePerson: (id: string, input: { name: string; color?: string | null }) =>
    invoke<void>("update_person", { id, input }),
  deletePerson: (id: string) => invoke<void>("delete_person", { id }),
};

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}
