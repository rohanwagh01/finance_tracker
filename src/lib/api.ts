import { invoke } from "@tauri-apps/api/core";
import type {
  CredentialStatus,
  Person,
  Settings,
  SetupStatus,
} from "./types";

export const api = {
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
