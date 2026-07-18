import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { loadConfig, saveConfig, loadPresets, isConfigured } from "./settings";
import { SettingsModal } from "./components/SettingsModal";
import { SmartCratesModal } from "./components/SmartCratesModal";
import { SyncButton, type SyncState } from "./components/SyncButton";
import { SyncLog, logLineFromProgress, phaseTag, type LogLine } from "./components/SyncLog";
import { DjcButton, Eyebrow, Icon } from "./components/primitives";
import type {
  AutoCueReport,
  ClearTagsReport,
  Config,
  DupesResult,
  SmartCratePreset,
  ProgressPayload,
} from "./types";
import "./index.css";

type Screen = "main" | "sync";

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [presets, setPresets] = useState<SmartCratePreset[]>([]);
  const [screen, setScreen] = useState<Screen>("main");
  const [syncState, setSyncState] = useState<SyncState>("idle");
  const [summary, setSummary] = useState<string>("");
  const [error, setError] = useState("");

  const [showSettings, setShowSettings] = useState(false);
  const [showSmart, setShowSmart] = useState(false);

  // Terminal log state
  const [logLines, setLogLines] = useState<LogLine[]>([]);
  const [logPhase, setLogPhase] = useState<string>("boot");
  const [logPct, setLogPct] = useState<number>(0);
  const [logOps, setLogOps] = useState<string>("waiting…");

  const progressUnlistenRef = useRef<null | (() => void)>(null);
  const logIdxRef = useRef<number>(0);

  // Load config + presets on mount
  useEffect(() => {
    (async () => {
      const [cfg, ps] = await Promise.all([loadConfig(), loadPresets()]);
      setConfig(cfg);
      setPresets(ps);
      if (!isConfigured(cfg)) setShowSettings(true);

      const unlisten = await listen<ProgressPayload>("sync:progress", (evt) => {
        const p = evt.payload;
        logIdxRef.current += 1;
        // Keep only the last 200 lines — unbounded growth + full-array
        // spread on every event was blocking the UI thread on long runs.
        setLogLines((prev) => {
          const next = prev.length >= 200 ? prev.slice(-199) : prev.slice();
          next.push(logLineFromProgress(p, logIdxRef.current));
          return next;
        });
        setLogPhase(phaseTag(p.stage));
        if (p.total > 0) {
          setLogPct(Math.min(1, p.current / p.total));
          setLogOps(`${p.current.toLocaleString()}/${p.total.toLocaleString()} ops`);
        } else {
          setLogOps(p.stage);
        }
      });
      progressUnlistenRef.current = unlisten;
    })();
    return () => {
      progressUnlistenRef.current?.();
    };
  }, []);

  // Keyboard shortcuts: ⌘K smart, ⌘, settings, Esc close modals
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const m = e.metaKey || e.ctrlKey;
      if (m && e.key === ",") {
        e.preventDefault();
        setShowSettings(true);
      }
      if (m && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (config && isConfigured(config)) setShowSmart(true);
        else setShowSettings(true);
      }
      if (e.key === "Escape") {
        setShowSmart(false);
        setShowSettings(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [config]);

  const resetLog = () => {
    setLogLines([]);
    setLogPhase("boot");
    setLogPct(0);
    setLogOps("starting");
    logIdxRef.current = 0;
  };

  // Force a render tick so the sync screen paints before a heavy invoke
  // starts saturating the worker thread + event bus.
  const yieldToPaint = () => new Promise<void>((resolve) => setTimeout(resolve, 32));

  const runSync = async () => {
    if (!config || !isConfigured(config)) {
      setShowSettings(true);
      return;
    }
    setError("");
    setSyncState("scanning");
    setScreen("sync");
    resetLog();
    await yieldToPaint();
    try {
      // One button, whole pipeline: intake → sanitize → fix-bpms →
      // downbeat-cue → serato sync → apple-music mirror, wrapped in the
      // safety rails (Serato-closed guard, pipeline lock, snapshot).
      setSyncState("syncing");
      const result = await invoke<{ summary: string[]; failed: number }>(
        "run_full_pipeline",
      );
      setSummary(result.summary.join("  ·  "));
      if (result.failed > 0) {
        setError(`${result.failed} step(s) failed — see summary`);
      }
      setLogPct(1);
      setLogPhase("done");
      setScreen("main");
      setSyncState(result.failed > 0 ? "error" : "done");
    } catch (e) {
      setError(String(e));
      setScreen("main");
      setSyncState("error");
    }
  };

  const runClearTags = async () => {
    if (!config || !isConfigured(config)) {
      setShowSettings(true);
      return;
    }
    setError("");
    setSyncState("syncing");
    setScreen("sync");
    resetLog();
    await yieldToPaint();
    try {
      const r = await invoke<ClearTagsReport>("clear_tags_library", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath || null,
      });
      setSummary(
        `${r.cleared}/${r.scanned} cleared (Comment/Grouping/Label)` +
          (r.serato_db_updated > 0
            ? ` · ${r.serato_db_updated} Serato db entries cleared (restart Serato)`
            : "") +
          (r.errors.length > 0 ? ` · ${r.errors.length} errors` : ""),
      );
      setLogPct(1);
      setLogPhase("done");
      setScreen("main");
      setSyncState("done");
    } catch (e) {
      setError(String(e));
      setScreen("main");
      setSyncState("error");
    }
  };

  const runAutoCue = async (force: boolean = false) => {
    if (!config || !isConfigured(config)) {
      setShowSettings(true);
      return;
    }
    if (force) {
      const ok = window.confirm(
        "Reset Cues will OVERWRITE every hot cue in your library — sets cue slot 8 at each track's first downbeat. Existing cue points will be lost. Continue?",
      );
      if (!ok) return;
    }
    setError("");
    setSyncState("syncing");
    setScreen("sync");
    resetLog();
    await yieldToPaint();
    try {
      const r = await invoke<AutoCueReport>("auto_cue_library", {
        cratesRoot: config.cratesRoot,
        pythonVenv: config.pythonVenv || null,
        scriptsDir: config.scriptsDir || null,
        barsPerCue: 16,
        numCues: 1,
        cueIndex: 7,
        force,
      });
      setSummary(
        force
          ? `${r.written}/${r.scanned} cue 8 set at downbeat` +
              (r.errors.length > 0 ? ` · ${r.errors.length} errors` : "")
          : `${r.written}/${r.scanned} cued · ${r.skipped} skipped (already had cues)` +
              (r.errors.length > 0 ? ` · ${r.errors.length} errors` : ""),
      );
      setLogPct(1);
      setLogPhase("done");
      setScreen("main");
      setSyncState("done");
    } catch (e) {
      setError(String(e));
      setScreen("main");
      setSyncState("error");
    }
  };

  const runFindDupes = async () => {
    if (!config || !isConfigured(config)) {
      setShowSettings(true);
      return;
    }
    setError("");
    setSyncState("syncing");
    setScreen("sync");
    resetLog();
    await yieldToPaint();
    try {
      const r = await invoke<DupesResult>("find_duplicates", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath || null,
        writeCrate: true,
      });
      const gb = (r.wasted_bytes / 1e9).toFixed(2);
      setSummary(
        r.groups === 0
          ? `No duplicates found across ${r.scanned.toLocaleString()} tracks 🎉`
          : `${r.groups} dupe groups · ${r.extra_copies} extra copies · ${gb} GB reclaimable` +
              (r.crate_name
                ? ` · review in "${r.crate_name.replace("%%", " → ")}" (restart Serato to see)`
                : ""),
      );
      setLogPct(1);
      setLogPhase("done");
      setScreen("main");
      setSyncState("done");
    } catch (e) {
      setError(String(e));
      setScreen("main");
      setSyncState("error");
    }
  };

  const cancelSync = () => {
    setScreen("main");
    setSyncState("idle");
  };

  if (!config) return null;

  const configured = isConfigured(config);
  const isBusy = syncState === "scanning" || syncState === "syncing";

  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: "var(--color-bg)",
      }}
    >
      {screen === "main" && (
        <MainScreen
          config={config}
          syncState={syncState}
          summary={summary}
          error={error}
          onSync={runSync}
          onClearTags={runClearTags}
          onResetCues={() => runAutoCue(true)}
          onFindDupes={runFindDupes}
          onSmart={() => (configured ? setShowSmart(true) : setShowSettings(true))}
          onSettings={() => setShowSettings(true)}
          disabled={isBusy}
        />
      )}

      {screen === "sync" && (
        <SyncLog
          lines={logLines}
          phase={logPhase}
          pct={logPct}
          opsLabel={logOps}
          onCancel={cancelSync}
          cancelable={syncState === "syncing"}
        />
      )}

      {showSettings && (
        <SettingsModal
          config={config}
          forceInitialSetup={!configured}
          onSave={async (next) => {
            await saveConfig(next);
            setConfig(next);
          }}
          onClose={() => setShowSettings(false)}
        />
      )}

      {showSmart && (
        <SmartCratesModal
          config={config}
          presets={presets}
          onPresetsChange={setPresets}
          onConfigChange={setConfig}
          onClose={() => setShowSmart(false)}
        />
      )}
    </div>
  );
}

function MainScreen({
  config,
  syncState,
  summary,
  error,
  onSync,
  onClearTags,
  onResetCues,
  onFindDupes,
  onSmart,
  onSettings,
  disabled,
}: {
  config: Config;
  syncState: SyncState;
  summary: string;
  error: string;
  onSync: () => void;
  onClearTags: () => void;
  onResetCues: () => void;
  onFindDupes: () => void;
  onSmart: () => void;
  onSettings: () => void;
  disabled: boolean;
}) {
  const [showAdvanced, setShowAdvanced] = useState(false);
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        background: "radial-gradient(ellipse at 50% 0%, #16161a 0%, #0b0b0c 55%)",
      }}
    >
      {/* Context strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 24,
          padding: "18px 28px",
          borderBottom: "0.5px solid var(--color-rule)",
        }}
      >
        <Stat label="Library" value="CRATES" sub={shortenHome(config.cratesRoot) || "—"} />
        <Divider />
        <Stat label="Target" value="SERATO" sub={shortenHome(config.seratoPath) || "—"} />
        <div style={{ flex: 1 }} />
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <div
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background:
                syncState === "done"
                  ? "var(--color-ok)"
                  : syncState === "error"
                  ? "var(--color-err)"
                  : "var(--color-text-3)",
            }}
          />
          <Eyebrow>
            {syncState === "done"
              ? "just synced"
              : syncState === "error"
              ? "last run failed"
              : "ready"}
          </Eyebrow>
        </div>
      </div>

      {/* Hero */}
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexDirection: "column",
          gap: 20,
          minHeight: 0,
          padding: "20px 40px",
        }}
      >
        <SyncButton state={syncState} onClick={onSync} size={220} disabled={disabled} />
        <div style={{ textAlign: "center", maxWidth: 380, minHeight: 40 }}>
          {syncState === "done" && summary ? (
            <div
              style={{
                fontSize: 13,
                color: "var(--color-text-2)",
                fontFamily: "var(--font-mono)",
              }}
            >
              {summary}
            </div>
          ) : syncState === "error" && error ? (
            <div
              style={{
                fontSize: 12,
                color: "var(--color-err)",
                fontFamily: "var(--font-mono)",
                wordBreak: "break-word",
              }}
            >
              {error}
            </div>
          ) : (
            <div style={{ fontSize: 13, color: "var(--color-text-2)", lineHeight: 1.5 }}>
              One button. Files new music into your genre folders, cleans tags, fixes BPMs, sets
              downbeat cues, rebuilds your Serato crates, and updates Apple Music.
            </div>
          )}
        </div>
      </div>

      {/* Footer */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "14px 18px",
          borderTop: "0.5px solid var(--color-rule)",
        }}
      >
        <DjcButton kind="ghost" size="sm" icon={<Icon.Wand />} onClick={onSmart} hint="⌘+K">
          Smart Crates
        </DjcButton>
        <div style={{ flex: 1 }} />
        {showAdvanced && (
          <>
            <DjcButton
              kind="ghost"
              size="sm"
              icon={<Icon.Diff />}
              onClick={onFindDupes}
              disabled={disabled}
              title="Find duplicate tracks (same artist + title + duration). Read-only — writes a report and a SMART › Duplicates Review crate, never deletes files."
            >
              Find Dupes
            </DjcButton>
            <DjcButton
              kind="danger"
              size="sm"
              icon={<Icon.Trash />}
              onClick={onResetCues}
              disabled={disabled}
              title="Overwrite every existing hot cue in the library — sets cue 8 at the first downbeat of each track. Use this when cues are drifting / off-beat."
            >
              Reset Cues
            </DjcButton>
            <DjcButton
              kind="danger"
              size="sm"
              icon={<Icon.Trash />}
              onClick={onClearTags}
              disabled={disabled}
              title="Wipe Comment / Grouping / Label from every file. Destructive — undoable only by re-running the pipeline."
            >
              Clear Tags
            </DjcButton>
          </>
        )}
        <DjcButton kind="ghost" size="sm" onClick={() => setShowAdvanced(!showAdvanced)}>
          {showAdvanced ? "Hide Advanced" : "Advanced"}
        </DjcButton>
        <DjcButton kind="ghost" size="sm" icon={<Icon.Settings />} onClick={onSettings} hint="⌘+,">
          Settings
        </DjcButton>
      </div>
    </div>
  );
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div style={{ minWidth: 0 }}>
      <Eyebrow style={{ marginBottom: 4 }}>{label}</Eyebrow>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 14,
          fontWeight: 500,
          color: "var(--color-text)",
          letterSpacing: "0.08em",
        }}
      >
        {value}
      </div>
      {sub && (
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10,
            color: "var(--color-text-3)",
            marginTop: 2,
            maxWidth: 260,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {sub}
        </div>
      )}
    </div>
  );
}

const Divider = () => (
  <div style={{ width: 0.5, height: 32, background: "rgba(255,255,255,0.07)" }} />
);

function shortenHome(p: string): string {
  if (!p) return "";
  return p.replace(/^\/Users\/[^/]+/, "~");
}

export default App;
