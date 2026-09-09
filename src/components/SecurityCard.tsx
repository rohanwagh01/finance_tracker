import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import { Banner, Button, Card, TextField } from "./ui";

export default function SecurityCard() {
  const [oldPw, setOldPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [confirmPw, setConfirmPw] = useState("");
  const [resetText, setResetText] = useState("");
  const [showReset, setShowReset] = useState(false);

  const change = useMutation({
    mutationFn: () => api.vaultChangePassword(oldPw, newPw),
    onSuccess: () => {
      setOldPw("");
      setNewPw("");
      setConfirmPw("");
    },
  });
  const lock = useMutation({
    mutationFn: api.vaultLock,
    onSuccess: () => window.location.reload(),
  });
  const reset = useMutation({
    mutationFn: api.vaultReset,
    onSuccess: () => window.location.reload(),
  });

  const canChange =
    oldPw && newPw.length >= 8 && newPw === confirmPw && !change.isPending;

  return (
    <Card title="Security">
      <p className="mb-4 text-xs text-[var(--muted)]">
        The whole database is encrypted with your master password (Argon2id +
        SQLCipher). It's never stored or sent anywhere. There is no recovery.
      </p>

      <div className="space-y-3">
        <h3 className="text-xs font-semibold text-[var(--muted)]">
          Change master password
        </h3>
        <TextField
          label="Current password"
          type="password"
          value={oldPw}
          onChange={(e) => setOldPw(e.target.value)}
        />
        <TextField
          label="New password"
          type="password"
          value={newPw}
          onChange={(e) => setNewPw(e.target.value)}
        />
        <TextField
          label="Confirm new password"
          type="password"
          value={confirmPw}
          onChange={(e) => setConfirmPw(e.target.value)}
        />
        <Button disabled={!canChange} onClick={() => change.mutate()}>
          {change.isPending ? "Changing…" : "Change password"}
        </Button>
        {change.isError && (
          <Banner tone="error">{errorMessage(change.error)}</Banner>
        )}
        {change.isSuccess && (
          <Banner tone="success">Password changed.</Banner>
        )}
      </div>

      <div className="mt-6 flex flex-wrap items-center gap-3 border-t border-[var(--border)] pt-4">
        <Button variant="ghost" onClick={() => lock.mutate()}>
          Lock now
        </Button>
        <span className="text-xs text-[var(--muted)]">
          Locks immediately; you'll re-enter the password.
        </span>
      </div>

      <div className="mt-4 border-t border-[var(--border)] pt-4">
        {!showReset ? (
          <button
            className="text-xs text-red-500 underline"
            onClick={() => setShowReset(true)}
          >
            Reset app (delete everything)
          </button>
        ) : (
          <div className="space-y-3">
            <Banner tone="error">
              Permanently deletes the encrypted database — accounts, transactions,
              settings, keys. No undo, no recovery.
            </Banner>
            <TextField
              label="Type DELETE to confirm"
              value={resetText}
              onChange={(e) => setResetText(e.target.value)}
            />
            <div className="flex gap-2">
              <Button
                variant="danger"
                disabled={resetText !== "DELETE" || reset.isPending}
                onClick={() => reset.mutate()}
              >
                Reset the app
              </Button>
              <Button variant="ghost" onClick={() => setShowReset(false)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}
