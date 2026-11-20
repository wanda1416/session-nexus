import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type CoreStatus = {
  phase: string;
  providers: string[];
};

function App() {
  const [status, setStatus] = useState<CoreStatus | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<CoreStatus>("core_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  return (
    <main className="container">
      <h1>SessionNexus</h1>
      <p>Phase {status?.phase ?? "…"} skeleton. CLI providers are declared, not connected yet.</p>
      {error ? <p>{error}</p> : null}
      <ul>
        {(status?.providers ?? []).map((id) => (
          <li key={id}>{id}</li>
        ))}
      </ul>
    </main>
  );
}

export default App;
