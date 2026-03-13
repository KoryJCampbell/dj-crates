import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Disc3, Loader2, CheckCircle2 } from "lucide-react";
import "./index.css";

type Status = "idle" | "syncing" | "done" | "error";

function App() {
  const [status, setStatus] = useState<Status>("idle");
  const [stats, setStats] = useState<{ crates: number; tracks: number } | null>(null);
  const [error, setError] = useState("");

  const handleSync = async () => {
    setStatus("syncing");
    setError("");
    try {
      const result = await invoke<{
        crates_written: number;
        total_track_entries: number;
      }>("sync_to_serato", {
        cratesRoot: "/Users/koryjcampbell/Music/CRATES",
        seratoPath: "/Users/koryjcampbell/Music/_Serato_",
        clean: true,
      });
      setStats({ crates: result.crates_written, tracks: result.total_track_entries });
      setStatus("done");
      setTimeout(() => setStatus("idle"), 4000);
    } catch (e) {
      setError(String(e));
      setStatus("error");
      setTimeout(() => setStatus("idle"), 4000);
    }
  };

  return (
    <div className="h-full flex flex-col items-center justify-center select-none">
      <button
        onClick={handleSync}
        disabled={status === "syncing"}
        className="group relative w-32 h-32 rounded-full flex items-center justify-center transition-all duration-300 cursor-pointer disabled:cursor-wait border-2 border-[var(--color-border)] hover:border-[var(--color-accent)] hover:shadow-[0_0_30px_rgba(99,102,241,0.3)] active:scale-95"
        style={{
          background:
            status === "done"
              ? "var(--color-success)"
              : status === "error"
              ? "var(--color-danger)"
              : "var(--color-surface)",
        }}
      >
        {status === "syncing" ? (
          <Loader2 size={48} className="animate-spin text-[var(--color-accent)]" />
        ) : status === "done" ? (
          <CheckCircle2 size={48} className="text-white" />
        ) : (
          <Disc3
            size={48}
            className="text-[var(--color-text-dim)] group-hover:text-[var(--color-accent)] transition-colors"
          />
        )}
      </button>

      <p className="mt-6 text-sm text-[var(--color-text-dim)]">
        {status === "syncing"
          ? "Syncing..."
          : status === "done" && stats
          ? `${stats.crates.toLocaleString()} crates · ${stats.tracks.toLocaleString()} tracks`
          : status === "error"
          ? error
          : "Sync to Serato"}
      </p>
    </div>
  );
}

export default App;
