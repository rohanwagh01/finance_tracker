import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import { Banner, Button, TextField } from "../components/ui";

type Props = {
  mode: "uninitialized" | "locked";
  onUnlocked: () => void;
};

export default function Vault({ mode, onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showReset, setShowReset] = useState(false);
  const [resetText, setResetText] = useState("");

  const isSetup = mode === "uninitialized";

  const submit = async () => {
    setError(null);
    if (isSetup) {
      if (password.length < 8) return setError("Use at least 8 characters.");
      if (password !== confirm) return setError("Passwords don't match.");
    } else if (!password) {
      return;
    }
    setBusy(true);
    try {
      if (isSetup) await api.vaultInitialize(password);
      else await api.vaultUnlock(password);
      setPassword("");
      setConfirm("");
      onUnlocked();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const doReset = async () => {
    setBusy(true);
    try {
      await api.vaultReset();
      setShowReset(false);
      setResetText("");
      setPassword("");
      onUnlocked(); // re-checks status → lands on "set password"
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto flex min-h-full max-w-sm flex-col justify-center p-8">
      <h1 className="text-2xl font-semibold">Finance Tracker</h1>

      {showReset ? (
        <div className="mt-4 space-y-4">
          <Banner tone="error">
            This permanently deletes the encrypted database — all accounts,
            transactions, and settings. There is no undo and no recovery. You'll
            set a new password and start over.
          </Banner>
          <TextField
            label='Type DELETE to confirm'
            value={resetText}
            onChange={(e) => setResetText(e.target.value)}
          />
          <div className="flex gap-2">
            <Button
              variant="danger"
              disabled={busy || resetText !== "DELETE"}
              onClick={doReset}
            >
              Reset the app
            </Button>
            <Button variant="ghost" onClick={() => setShowReset(false)}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="mt-4 space-y-4">
          <p className="text-sm text-[var(--muted)]">
            {isSetup
              ? "Choose a master password. It encrypts everything this app stores on your machine. It is never sent anywhere and cannot be recovered — if you forget it, the data is gone."
              : "Enter your master password to unlock."}
          </p>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              submit();
            }}
            className="space-y-3"
          >
            <TextField
              label={isSetup ? "New master password" : "Master password"}
              type="password"
              autoFocus
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
            {isSetup && (
              <TextField
                label="Confirm password"
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
              />
            )}
            {error && <Banner tone="error">{error}</Banner>}
            <Button type="submit" disabled={busy || !password}>
              {busy ? "Working…" : isSetup ? "Create & open" : "Unlock"}
            </Button>
          </form>

          {!isSetup && (
            <button
              className="text-xs text-[var(--muted)] underline"
              onClick={() => setShowReset(true)}
            >
              Forgot password? Reset the app
            </button>
          )}
        </div>
      )}
    </div>
  );
}
