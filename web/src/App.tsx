// Phase 1b smoke UI: proves the webview can drive the real Anki backend.
// The deck list and review UI proper arrive in 1c; this screen exists to show
// the whole stack working end to end on a device.

import { useCallback, useEffect, useState } from "react";
import {
  dataDir,
  health,
  isConfigured,
  NotConfiguredError,
  type Health,
} from "./backend/client.js";
import { getDeckNames, openCollection, type Deck } from "./backend/collection.js";

type Status = "starting" | "ready" | "error";

export function App() {
  const [status, setStatus] = useState<Status>("starting");
  const [info, setInfo] = useState<Health | null>(null);
  const [decks, setDecks] = useState<Deck[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    (async () => {
      try {
        setInfo(await health(controller.signal));
        setStatus("ready");
      } catch (err) {
        if (controller.signal.aborted) return;
        setError(describe(err));
        setStatus("error");
      }
    })();
    return () => controller.abort();
  }, []);

  const loadDecks = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      // rslib resolves relative paths against the working directory, which is
      // not meaningful on iOS, so build absolute paths under the shell's dir.
      const dir = dataDir();
      await openCollection({
        collectionPath: `${dir}/collection.anki2`,
        mediaFolderPath: `${dir}/collection.media`,
        mediaDbPath: `${dir}/collection.media.db2`,
      });
      setDecks(await getDeckNames());
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <div className="wrap">
      <h1>AnkiFruit</h1>
      <p className="sub">Anki's own backend, running on iOS.</p>

      {status === "error" && error && (
        <div className="card">
          <p className="error">{error}</p>
        </div>
      )}

      {info && (
        <div className="card">
          <h2>Backend</h2>
          <dl>
            <dt>Status</dt>
            <dd>{info.ok ? "running" : "unavailable"}</dd>
            <dt>Anki</dt>
            <dd>{info.anki || "unknown"}</dd>
            <dt>AnkiFruit</dt>
            <dd>{info.ankifruit}</dd>
          </dl>
        </div>
      )}

      <div className="card">
        <h2>Collection</h2>
        <button onClick={loadDecks} disabled={busy || status !== "ready"}>
          {busy ? "Opening…" : decks ? "Reload decks" : "Open collection"}
        </button>
        {decks && (
          <ul className="decks" style={{ marginTop: "1rem" }}>
            {decks.map((deck) => (
              <li key={String(deck.id)}>
                <span>{deck.name}</span>
                <span className="id">{String(deck.id)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function describe(err: unknown): string {
  if (err instanceof NotConfiguredError) {
    return "Open this through the AnkiFruit app — the backend token is injected by the native shell.";
  }
  return err instanceof Error ? err.message : String(err);
}

// Surface a clear message when the page is opened outside the shell.
if (typeof window !== "undefined" && !isConfigured()) {
  console.warn("AnkiFruit: no backend credentials injected");
}
