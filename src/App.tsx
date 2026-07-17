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
  BpmFixReport,
  ClearTagsReport,
  Config,
  DupesResult,
  EnrichResult,
  MoodReport,
  MoveReport,
  SanitizeReport,
  SmartCratePreset,
  SyncResult,
  ProgressPayload,
  UnsortedMove,
  UnsortedTrack,
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
      // 1. Scan Unsorted (if configured) and move EVERYTHING — confident
      //    matches into GENRES/<g>/<sub>/, edits into GENRES/<g>/EDITS/,
      //    leftovers into _REVIEW/_Unrouted/. Tags are written during apply.
      let movedLabel = "";
      if (config.unsortedPath) {
        const tracks = await invoke<UnsortedTrack[]>("scan_unsorted", {
          unsortedPath: config.unsortedPath,
          cratesRoot: config.cratesRoot,
          spotifyClientId: config.spotifyClientId || null,
          spotifyClientSecret: config.spotifyClientSecret || null,
        });
        const moves: UnsortedMove[] = tracks.map((t) => ({
          abs_path: t.abs_path,
          dest_dir: t.suggested_dest,
          dest_filename: t.cleaned_filename || t.filename,
          genre: t.detected_genre ?? "",
          grouping: t.is_edit ? "Edit" : t.detected_subgenre ?? "",
        }));
        if (moves.length > 0) {
          const report = await invoke<MoveReport>("apply_unsorted_moves", { moves });
          movedLabel = ` · ${report.moved} moved`;
        }
      }

      // 2. Rebuild Serato crates from the updated folder tree.
      setSyncState("syncing");
      const result = await invoke<SyncResult>("sync_to_serato", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath,
        pythonVenv: config.pythonVenv || null,
        scriptsDir: config.scriptsDir || null,
        unsortedPath: config.unsortedPath || null,
        clean: true,
        preview: false,
      });
      setSummary(
        `${result.crates_written.toLocaleString()} crates · ${result.total_track_entries.toLocaleString()} tracks` +
          movedLabel +
          (result.bpm_adjusted > 0 ? ` · ${result.bpm_adjusted} bpm fixed` : "") +
          (result.database_tracks_new > 0 ? ` · ${result.database_tracks_new} db rows` : "") +
          (result.new_this_week > 0 ? ` · ${result.new_this_week} new this week` : ""),
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

  const runFixBpms = async () => {
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
      const r = await invoke<BpmFixReport>("normalize_bpms", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath || null,
      });
      setSummary(
        `${r.scanned} scanned · ${r.halved} halved · ${r.doubled} doubled` +
          (r.quartered > 0 ? ` · ${r.quartered} quartered` : "") +
          (r.serato_db_updated > 0
            ? ` · ${r.serato_db_updated} synced to Serato db (restart Serato to see)`
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

  const runLabelMoods = async () => {
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
      const r = await invoke<MoodReport>("label_moods", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath || null,
      });
      const breakdown = r.by_mood
        .map(([m, n]) => `${m}: ${n}`)
        .join(" · ");
      const seratoNote =
        r.serato_db_updated > 0
          ? ` · ${r.serato_db_updated} synced to Serato db (restart Serato to see)`
          : "";
      setSummary(
        `${r.labeled}/${r.scanned} labeled · ${r.blank} blank${seratoNote}` +
          (breakdown ? ` — ${breakdown}` : ""),
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

  const runEnrichPopularity = async () => {
    if (!config || !isConfigured(config)) {
      setShowSettings(true);
      return;
    }
    if (!config.spotifyClientId || !config.spotifyClientSecret) {
      setError("Set Spotify Client ID + Secret in Settings first.");
      setSyncState("error");
      return;
    }
    setError("");
    setSyncState("syncing");
    setScreen("sync");
    resetLog();
    await yieldToPaint();
    try {
      const r = await invoke<EnrichResult>("enrich_spotify_popularity", {
        clientId: config.spotifyClientId,
        clientSecret: config.spotifyClientSecret,
        force: false,
      });
      setSummary(
        `${r.total} total · ${r.enriched} fetched · ${r.cached} cached · ${r.not_found} not found` +
          (r.errors > 0 ? ` · ${r.errors} errors` : ""),
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

  const runSanitize = async () => {
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
      const r = await invoke<SanitizeReport>("sanitize_library", {
        cratesRoot: config.cratesRoot,
        seratoPath: config.seratoPath || null,
        spotifyClientId: config.spotifyClientId || null,
        spotifyClientSecret: config.spotifyClientSecret || null,
      });
      setSummary(
        `${r.updated}/${r.scanned} updated (genre + grouping + year)` +
          (r.comments_written > 0
            ? ` · ${r.comments_written} comments written (Spotify subgenre array)`
            : "") +
          (r.serato_db_updated > 0
            ? ` · ${r.serato_db_updated} Serato db entries updated`
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
          onFixBpms={runFixBpms}
          onLabelMoods={runLabelMoods}
          onEnrichPop={runEnrichPopularity}
          onClearTags={runClearTags}
          onSanitize={runSanitize}
          onAutoCue={() => runAutoCue(false)}
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
  onFixBpms,
  onLabelMoods,
  onEnrichPop,
  onClearTags,
  onSanitize,
  onAutoCue,
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
  onFixBpms: () => void;
  onLabelMoods: () => void;
  onEnrichPop: () => void;
  onClearTags: () => void;
  onSanitize: () => void;
  onAutoCue: () => void;
  onResetCues: () => void;
  onFindDupes: () => void;
  onSmart: () => void;
  onSettings: () => void;
  disabled: boolean;
}) {
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
              One click. Sorts Unsorted into your genre folders, writes tags, rebuilds your Serato
              crates to match.
            </div>
          )}
        </div>

        {/* Post-sync pipeline — numbered 1-5, run left to right */}
        <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10 }}>
          <Eyebrow>Post-Sync Pipeline · run in order</Eyebrow>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "center" }}>
            <PipelineStep
              n={1}
              label="Sanitize"
              hint="normalize genre · grouping · year"
              onClick={onSanitize}
              disabled={disabled}
            />
            <PipelineStep
              n={2}
              label="Fix BPMs"
              hint="halve / double out-of-range"
              onClick={onFixBpms}
              disabled={disabled}
            />
            <PipelineStep
              n={3}
              label="Label Moods"
              hint="warmup · peak · slowdown"
              onClick={onLabelMoods}
              disabled={disabled}
            />
            <PipelineStep
              n={4}
              label="Fetch Popularity"
              hint="spotify popularity score"
              onClick={onEnrichPop}
              disabled={disabled}
            />
            <PipelineStep
              n={5}
              label="Downbeat Cue"
              hint="set hot cue 8 at first downbeat"
              onClick={onAutoCue}
              disabled={disabled}
            />
          </div>
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
        <div style={{ flex: 1 }} />
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
        <DjcButton kind="ghost" size="sm" icon={<Icon.Settings />} onClick={onSettings} hint="⌘+,">
          Settings
        </DjcButton>
      </div>
    </div>
  );
}

function PipelineStep({
  n,
  label,
  hint,
  onClick,
  disabled,
}: {
  n: number;
  label: string;
  hint: string;
  onClick: () => void;
  disabled: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={disabled ? undefined : onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      disabled={disabled}
      title={hint}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        height: 30,
        padding: "0 12px 0 6px",
        fontFamily: "var(--font-sans)",
        fontSize: 12,
        fontWeight: 500,
        color: "var(--color-text)",
        background: hover && !disabled ? "rgba(255,255,255,0.05)" : "transparent",
        border: "0.5px solid rgba(255,255,255,0.12)",
        borderRadius: 15,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
        transition: "background 80ms ease",
        whiteSpace: "nowrap",
      }}
    >
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: 20,
          height: 20,
          borderRadius: "50%",
          background: "var(--color-accent)",
          color: "#0b0b0c",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          fontWeight: 600,
        }}
      >
        {n}
      </span>
      {label}
    </button>
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
