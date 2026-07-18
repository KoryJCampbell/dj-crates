import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  SmartCratePreset,
  SmartCrateRules,
  SmartCrateResult,
  EnrichResult,
} from "../types";
import { savePresets, saveConfig } from "../settings";
import { DjcButton, Eyebrow, Icon, Modal, Rule } from "./primitives";

type Props = {
  config: Config;
  presets: SmartCratePreset[];
  onPresetsChange: (presets: SmartCratePreset[]) => void;
  onConfigChange: (config: Config) => void;
  onClose: () => void;
};

type Mode = "nl" | "preset" | "rule";

const EMPTY_RULES: SmartCrateRules = {
  genres: [],
  subgenres: [],
  artists: [],
};

export function SmartCratesModal({
  config,
  presets: initialPresets,
  onPresetsChange,
  onConfigChange,
  onClose,
}: Props) {
  const [presets, setPresetsLocal] = useState<SmartCratePreset[]>(initialPresets);
  const [mode, setMode] = useState<Mode>("nl");
  const [selectedId, setSelectedId] = useState<string>(initialPresets[0]?.id ?? "");
  const [quickText, setQuickText] = useState<string>("chill 90s R&B under 95 bpm");
  const [crateName, setCrateName] = useState<string>("");
  const [rules, setRules] = useState<SmartCrateRules>({ ...EMPTY_RULES });
  const [result, setResult] = useState<SmartCrateResult | null>(null);
  const [enrichStatus, setEnrichStatus] = useState<EnrichResult | null>(null);
  const [enrich, setEnrich] = useState<boolean>(
    Boolean(config.spotifyClientId && config.spotifyClientSecret),
  );
  const [spotifyId, setSpotifyId] = useState(config.spotifyClientId);
  const [spotifySecret, setSpotifySecret] = useState(config.spotifyClientSecret);
  const [error, setError] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [enriching, setEnriching] = useState(false);

  const parsedQuickRules = useMemo(() => parseQuickDescription(quickText), [quickText]);

  const connected = Boolean(config.spotifyClientId && config.spotifyClientSecret);

  const effectiveRules = (): SmartCrateRules => {
    if (mode === "nl") return parsedQuickRules;
    if (mode === "preset") {
      const p = presets.find((x) => x.id === selectedId);
      return p ? p.rules : rules;
    }
    return rules;
  };

  const effectiveName = (): string => {
    if (crateName.trim()) return crateName.trim();
    if (mode === "nl") return quickText.trim() || "Smart Crate";
    if (mode === "preset") {
      const p = presets.find((x) => x.id === selectedId);
      return p?.name ?? "Smart Crate";
    }
    return "Smart Crate";
  };

  const run = async (preview: boolean) => {
    setBusy(true);
    setError("");
    try {
      const r = await invoke<SmartCrateResult>("generate_smart_crate", {
        cratesRoot: config.cratesRoot || null,
        seratoPath: config.seratoPath,
        name: effectiveName(),
        rules: effectiveRules(),
        preview,
      });
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const connectSpotify = async () => {
    if (!spotifyId.trim() || !spotifySecret.trim()) {
      setError("Paste both Client ID and Client Secret.");
      return;
    }
    const next: Config = {
      ...config,
      spotifyClientId: spotifyId.trim(),
      spotifyClientSecret: spotifySecret.trim(),
    };
    await saveConfig(next);
    onConfigChange(next);
    setError("");
  };

  const runEnrich = async () => {
    if (!config.spotifyClientId || !config.spotifyClientSecret) {
      setError("Connect Spotify first.");
      return;
    }
    setEnriching(true);
    setError("");
    setEnrichStatus(null);
    try {
      const r = await invoke<EnrichResult>("enrich_spotify_popularity", {
        clientId: config.spotifyClientId,
        clientSecret: config.spotifyClientSecret,
        force: false,
      });
      setEnrichStatus(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setEnriching(false);
    }
  };

  const saveAsPreset = async () => {
    const name = effectiveName();
    if (!name || name === "Smart Crate") {
      setError("Give the preset a name first (Crate name field).");
      return;
    }
    const id =
      name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "") +
      "-" +
      Date.now().toString(36);
    const newPreset: SmartCratePreset = {
      id,
      name,
      description: describeRules(effectiveRules()),
      rules: effectiveRules(),
      builtin: false,
    };
    const next = [...presets, newPreset];
    setPresetsLocal(next);
    await savePresets(next);
    onPresetsChange(next);
    setError("");
  };

  const deletePreset = async (id: string) => {
    const target = presets.find((p) => p.id === id);
    if (!target || target.builtin) return;
    const next = presets.filter((p) => p.id !== id);
    setPresetsLocal(next);
    await savePresets(next);
    onPresetsChange(next);
    if (selectedId === id) setSelectedId(next[0]?.id ?? "");
  };

  return (
    <Modal
      title="Smart Crates"
      subtitle="Generate rule-based crates from your library."
      onClose={onClose}
      width={640}
      footer={
        <>
          {mode !== "preset" && (
            <DjcButton kind="ghost" onClick={saveAsPreset} icon={<Icon.Save />} disabled={busy}>
              Save preset
            </DjcButton>
          )}
          <DjcButton kind="ghost" onClick={() => run(true)} disabled={busy} icon={<Icon.Eye />}>
            Preview
          </DjcButton>
          <DjcButton
            kind="primary"
            onClick={() => run(false)}
            disabled={busy}
            icon={<Icon.Wand />}
            hint="⌘+↵"
          >
            {busy ? "Generating…" : "Generate crate"}
          </DjcButton>
        </>
      }
    >
      {/* Mode tabs */}
      <div
        style={{
          display: "flex",
          gap: 4,
          padding: 3,
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.06)",
          borderRadius: 7,
        }}
      >
        {([
          ["nl", "Describe it"],
          ["preset", "Presets"],
          ["rule", "Rule builder"],
        ] as const).map(([k, lbl]) => (
          <button
            key={k}
            onClick={() => setMode(k)}
            style={{
              flex: 1,
              height: 30,
              borderRadius: 5,
              border: "none",
              cursor: "pointer",
              background: mode === k ? "var(--color-surface-3)" : "transparent",
              color: mode === k ? "var(--color-text)" : "var(--color-text-2)",
              boxShadow:
                mode === k
                  ? "inset 0 1px 0 rgba(255,255,255,0.05), 0 1px 2px rgba(0,0,0,0.3)"
                  : "none",
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              letterSpacing: "0.1em",
              textTransform: "uppercase",
              transition: "background 120ms",
            }}
          >
            {lbl}
          </button>
        ))}
      </div>

      {/* Mode body */}
      <div style={{ marginTop: 16, minHeight: 140 }}>
        {mode === "nl" && (
          <NLMode value={quickText} onChange={setQuickText} parsed={parsedQuickRules} />
        )}
        {mode === "preset" && (
          <PresetMode
            presets={presets}
            selectedId={selectedId}
            onSelect={setSelectedId}
            onDelete={deletePreset}
          />
        )}
        {mode === "rule" && <RuleBuilder rules={rules} onChange={setRules} />}
      </div>

      <Rule style={{ margin: "18px 0" }} />

      <SpotifyPrompt
        enrich={enrich}
        setEnrich={setEnrich}
        connected={connected}
        spotifyId={spotifyId}
        spotifySecret={spotifySecret}
        onIdChange={setSpotifyId}
        onSecretChange={setSpotifySecret}
        onConnect={connectSpotify}
        onRunEnrich={runEnrich}
        enriching={enriching}
      />

      {/* Crate name */}
      <div
        style={{
          marginTop: 14,
          display: "flex",
          alignItems: "center",
          gap: 10,
        }}
      >
        <Eyebrow>Crate name</Eyebrow>
        <input
          value={crateName}
          onChange={(e) => setCrateName(e.target.value)}
          placeholder={effectiveName()}
          style={{
            flex: 1,
            height: 30,
            padding: "0 10px",
            background: "var(--color-surface-2)",
            border: "0.5px solid rgba(255,255,255,0.1)",
            borderRadius: 5,
            color: "var(--color-text)",
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            outline: "none",
          }}
        />
      </div>

      {result && (
        <div
          style={{
            marginTop: 14,
            border: "0.5px solid rgba(255,255,255,0.07)",
            borderRadius: 6,
            padding: "10px 12px",
            background: "rgba(255,255,255,0.015)",
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: 6,
            }}
          >
            <Eyebrow>{result.preview ? "Preview" : "Generated"}</Eyebrow>
            <div
              style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--color-text-3)" }}
            >
              <span style={{ color: "var(--color-accent)" }}>{result.matched_tracks}</span> tracks
              match
            </div>
          </div>
          {result.sample_tracks.length > 0 && (
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                color: "var(--color-text-2)",
                lineHeight: 1.8,
              }}
            >
              {result.sample_tracks.map((t, i) => (
                <div
                  key={i}
                  style={{
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                  }}
                >
                  {t}
                </div>
              ))}
              {result.matched_tracks > result.sample_tracks.length && (
                <div style={{ color: "var(--color-text-3)" }}>
                  + {result.matched_tracks - result.sample_tracks.length} more
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {enrichStatus && (
        <div
          style={{
            marginTop: 10,
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--color-text-3)",
          }}
        >
          enriched <span style={{ color: "var(--color-accent)" }}>{enrichStatus.enriched}</span> ·
          cached {enrichStatus.cached} · not found {enrichStatus.not_found}
          {enrichStatus.errors > 0 && ` · errors ${enrichStatus.errors}`}
        </div>
      )}

      {error && (
        <div
          style={{
            marginTop: 14,
            padding: "8px 12px",
            border: "0.5px solid rgba(224,97,74,0.35)",
            borderRadius: 6,
            background: "rgba(224,97,74,0.08)",
            color: "var(--color-err)",
            fontSize: 12,
            fontFamily: "var(--font-mono)",
          }}
        >
          {error}
        </div>
      )}
    </Modal>
  );
}

// ─── Describe it mode ────────────────────────────────────────────────────────

function NLMode({
  value,
  onChange,
  parsed,
}: {
  value: string;
  onChange: (v: string) => void;
  parsed: SmartCrateRules;
}) {
  const suggestions = [
    "late-night R&B under 95",
    "peak-hour afrobeats 118-128",
    "underground 2000s hip-hop",
  ];
  return (
    <div>
      <Eyebrow style={{ marginBottom: 8 }}>Describe what you want</Eyebrow>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="e.g. aggressive techno 130-140"
        style={{
          width: "100%",
          height: 48,
          padding: "0 16px",
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.14)",
          borderRadius: 8,
          color: "var(--color-text)",
          fontFamily: "var(--font-mono)",
          fontSize: 14,
          outline: "none",
        }}
      />
      <div style={{ marginTop: 10, display: "flex", gap: 6, flexWrap: "wrap" }}>
        {suggestions.map((s) => (
          <button
            key={s}
            onClick={() => onChange(s)}
            style={{
              height: 24,
              padding: "0 10px",
              background: "transparent",
              border: "0.5px solid rgba(255,255,255,0.08)",
              borderRadius: 4,
              cursor: "pointer",
              color: "var(--color-text-2)",
              fontFamily: "var(--font-mono)",
              fontSize: 10,
              letterSpacing: "0.05em",
            }}
          >
            {s}
          </button>
        ))}
      </div>
      <div
        style={{
          marginTop: 12,
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: 11,
          color: "var(--color-text-3)",
        }}
      >
        <div style={{ width: 4, height: 4, borderRadius: "50%", background: "var(--color-accent)" }} />
        Parsed:{" "}
        <span
          style={{
            fontFamily: "var(--font-mono)",
            color: "var(--color-text-2)",
          }}
        >
          {describeRules(parsed)}
        </span>
      </div>
    </div>
  );
}

// ─── Presets mode ────────────────────────────────────────────────────────────

function PresetMode({
  presets,
  selectedId,
  onSelect,
  onDelete,
}: {
  presets: SmartCratePreset[];
  selectedId: string;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        gap: 8,
        maxHeight: 260,
        overflowY: "auto",
      }}
    >
      {presets.map((p) => {
        const sel = selectedId === p.id;
        return (
          <div
            key={p.id}
            onClick={() => onSelect(p.id)}
            style={{
              position: "relative",
              padding: 12,
              border: sel
                ? "0.5px solid var(--color-accent)"
                : "0.5px solid rgba(255,255,255,0.08)",
              background: sel ? "rgba(255,90,31,0.06)" : "transparent",
              borderRadius: 6,
              cursor: "pointer",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontFamily: "var(--font-mono)",
                fontSize: 12,
                fontWeight: 500,
                color: sel ? "var(--color-accent)" : "var(--color-text)",
              }}
            >
              {p.name}
              {!p.builtin && (
                <span
                  style={{
                    padding: "1px 5px",
                    borderRadius: 3,
                    fontSize: 9,
                    letterSpacing: "0.1em",
                    color: "var(--color-accent)",
                    background: "rgba(255,90,31,0.12)",
                  }}
                >
                  CUSTOM
                </span>
              )}
            </div>
            <div style={{ fontSize: 11, color: "var(--color-text-3)", marginTop: 4 }}>
              {p.description}
            </div>
            {!p.builtin && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(p.id);
                }}
                title="Delete preset"
                style={{
                  position: "absolute",
                  top: 8,
                  right: 8,
                  width: 20,
                  height: 20,
                  border: "none",
                  background: "transparent",
                  color: "var(--color-text-3)",
                  cursor: "pointer",
                  borderRadius: 3,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                <Icon.Trash />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

// ─── Rule builder mode ───────────────────────────────────────────────────────

function RuleBuilder({
  rules,
  onChange,
}: {
  rules: SmartCrateRules;
  onChange: (r: SmartCrateRules) => void;
}) {
  const listFromCSV = (s: string) =>
    s
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
  const setField = <K extends keyof SmartCrateRules>(k: K, v: SmartCrateRules[K]) =>
    onChange({ ...rules, [k]: v });

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
        <NumInput
          label="Year from"
          value={rules.year_min ?? null}
          onChange={(v) => setField("year_min", v)}
        />
        <NumInput
          label="Year to"
          value={rules.year_max ?? null}
          onChange={(v) => setField("year_max", v)}
        />
        <NumInput
          label="BPM min"
          value={rules.bpm_min ?? null}
          onChange={(v) => setField("bpm_min", v)}
        />
        <NumInput
          label="BPM max"
          value={rules.bpm_max ?? null}
          onChange={(v) => setField("bpm_max", v)}
        />
        <NumInput
          label="Pop min (0–100)"
          value={rules.popularity_min ?? null}
          onChange={(v) => setField("popularity_min", v)}
        />
        <NumInput
          label="Pop max (0–100)"
          value={rules.popularity_max ?? null}
          onChange={(v) => setField("popularity_max", v)}
        />
      </div>
      <TextInput
        label="Genres (comma)"
        placeholder="hip-hop, rap"
        value={rules.genres.join(", ")}
        onChange={(v) => setField("genres", listFromCSV(v))}
      />
      <TextInput
        label="Subgenres (comma)"
        placeholder="boom bap, drill"
        value={rules.subgenres.join(", ")}
        onChange={(v) => setField("subgenres", listFromCSV(v))}
      />
      <TextInput
        label="Artists contains (comma)"
        placeholder="badu, d'angelo"
        value={rules.artists.join(", ")}
        onChange={(v) => setField("artists", listFromCSV(v))}
      />
      <TextInput
        label="Title contains"
        placeholder=""
        value={rules.title_contains ?? ""}
        onChange={(v) => setField("title_contains", v || null)}
      />
      <NumInput
        label="Max tracks"
        value={rules.limit ?? null}
        onChange={(v) => setField("limit", v)}
      />
    </div>
  );
}

function NumInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number | null;
  onChange: (v: number | null) => void;
}) {
  return (
    <div>
      <Eyebrow style={{ marginBottom: 4 }}>{label}</Eyebrow>
      <input
        type="number"
        value={value ?? ""}
        onChange={(e) =>
          onChange(e.target.value === "" ? null : Number(e.target.value))
        }
        style={{
          width: "100%",
          height: 28,
          padding: "0 10px",
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.1)",
          borderRadius: 4,
          color: "var(--color-text)",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          outline: "none",
        }}
      />
    </div>
  );
}

function TextInput({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div>
      <Eyebrow style={{ marginBottom: 4 }}>{label}</Eyebrow>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        style={{
          width: "100%",
          height: 28,
          padding: "0 10px",
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.1)",
          borderRadius: 4,
          color: "var(--color-text)",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          outline: "none",
        }}
      />
    </div>
  );
}

// ─── Inline Spotify prompt ───────────────────────────────────────────────────

function SpotifyPrompt({
  enrich,
  setEnrich,
  connected,
  spotifyId,
  spotifySecret,
  onIdChange,
  onSecretChange,
  onConnect,
  onRunEnrich,
  enriching,
}: {
  enrich: boolean;
  setEnrich: (v: boolean) => void;
  connected: boolean;
  spotifyId: string;
  spotifySecret: string;
  onIdChange: (v: string) => void;
  onSecretChange: (v: string) => void;
  onConnect: () => void;
  onRunEnrich: () => void;
  enriching: boolean;
}) {
  return (
    <div
      style={{
        border: "0.5px solid rgba(255,255,255,0.08)",
        borderRadius: 8,
        background: "rgba(255,255,255,0.015)",
        overflow: "hidden",
      }}
    >
      <div
        onClick={() => setEnrich(!enrich)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "12px 14px",
          cursor: "pointer",
        }}
      >
        <div
          style={{
            width: 28,
            height: 16,
            borderRadius: 8,
            background: enrich ? "var(--color-accent)" : "rgba(255,255,255,0.1)",
            position: "relative",
            transition: "background 120ms",
            flexShrink: 0,
          }}
        >
          <div
            style={{
              position: "absolute",
              top: 1,
              left: enrich ? 13 : 1,
              width: 14,
              height: 14,
              borderRadius: "50%",
              background: "#fff",
              transition: "left 120ms",
            }}
          />
        </div>
        <div style={{ flex: 1 }}>
          <div
            style={{
              fontSize: 13,
              color: "var(--color-text)",
              display: "flex",
              alignItems: "center",
              gap: 8,
            }}
          >
            <Icon.Spotify style={{ color: "var(--color-text-2)" }} />
            Enrich with Spotify popularity
          </div>
          <div style={{ fontSize: 11, color: "var(--color-text-3)", marginTop: 2 }}>
            Weights results using Spotify's popularity score. Optional.
          </div>
        </div>
        {connected && (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <div style={{ width: 5, height: 5, borderRadius: "50%", background: "var(--color-ok)" }} />
            <Eyebrow>connected</Eyebrow>
          </div>
        )}
      </div>

      {enrich && (
        <div
          style={{
            borderTop: "0.5px solid rgba(255,255,255,0.06)",
            padding: 14,
            background: "rgba(0,0,0,0.2)",
            display: "flex",
            flexDirection: "column",
            gap: 8,
          }}
        >
          <div style={{ fontSize: 11, color: "var(--color-text-3)", lineHeight: 1.5 }}>
            {connected
              ? "Edit to rotate credentials — e.g. if you regenerated the Client Secret on the Spotify Developer Dashboard."
              : "One-time setup. Create an app at developer.spotify.com/dashboard, then paste the credentials below."}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>
            <LabeledInput
              label="Client ID"
              placeholder="a3b7…"
              value={spotifyId}
              onChange={onIdChange}
            />
            <LabeledInput
              label="Client Secret"
              placeholder="••••••••"
              type="password"
              value={spotifySecret}
              onChange={onSecretChange}
            />
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 6, marginTop: 4 }}>
            <DjcButton size="sm" kind="primary" onClick={onConnect}>
              {connected ? "Update" : "Connect"}
            </DjcButton>
          </div>
        </div>
      )}

      {enrich && connected && (
        <div
          style={{
            borderTop: "0.5px solid rgba(255,255,255,0.06)",
            padding: 12,
            display: "flex",
            alignItems: "center",
            gap: 10,
          }}
        >
          <div style={{ fontSize: 11, color: "var(--color-text-3)", lineHeight: 1.5, flex: 1 }}>
            Run once to populate popularity. Cached to disk — subsequent runs are fast.
          </div>
          <DjcButton size="sm" kind="ghost" onClick={onRunEnrich} disabled={enriching}>
            {enriching ? "Enriching…" : "Run enrichment"}
          </DjcButton>
        </div>
      )}
    </div>
  );
}

function LabeledInput({
  label,
  placeholder,
  type = "text",
  value,
  onChange,
}: {
  label: string;
  placeholder?: string;
  type?: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div>
      <Eyebrow style={{ marginBottom: 4 }}>{label}</Eyebrow>
      <input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{
          width: "100%",
          height: 28,
          padding: "0 10px",
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.1)",
          borderRadius: 4,
          color: "var(--color-text)",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          outline: "none",
        }}
      />
    </div>
  );
}

// ─── Quick parser ────────────────────────────────────────────────────────────

const GENRE_KEYWORDS: Array<[string, string[]]> = [
  ["hip-hop", ["hip-hop", "hiphop", "hip hop", "rap"]],
  ["r&b", ["r&b", "rnb", "r & b", "rhythm and blues"]],
  ["neo-soul", ["neo-soul", "neo soul"]],
  ["soul", ["soul"]],
  ["gospel", ["gospel"]],
  ["jazz", ["jazz"]],
  ["afrobeats", ["afrobeats", "afrobeat", "afro-beats"]],
  ["amapiano", ["amapiano"]],
  ["highlife", ["highlife", "high-life"]],
  ["house", ["house"]],
  ["trap", ["trap"]],
  ["drill", ["drill"]],
  ["boom bap", ["boom bap", "boom-bap"]],
  ["reggae", ["reggae"]],
  ["dancehall", ["dancehall"]],
];

export function parseQuickDescription(text: string): SmartCrateRules {
  const lower = ` ${text.toLowerCase()} `;
  const rules: SmartCrateRules = { genres: [], subgenres: [], artists: [] };

  const range = lower.match(/\b(\d{4})\s*[-–to]+\s*(\d{4})\b/);
  const bpmRange = lower.match(/\b(\d{2,3})\s*[-–]\s*(\d{2,3})\b/);
  if (range) {
    rules.year_min = parseInt(range[1]);
    rules.year_max = parseInt(range[2]);
  } else {
    const decade = lower.match(/\b(19|20)?(\d0)s\b/);
    if (decade) {
      const tens = parseInt(decade[2]);
      const century = decade[1] ? parseInt(decade[1]) : tens >= 60 ? 19 : 20;
      const base = century * 100 + tens;
      rules.year_min = base;
      rules.year_max = base + 9;
    } else {
      const singleYear = lower.match(/\b(19\d{2}|20\d{2})\b/);
      if (singleYear) {
        const y = parseInt(singleYear[1]);
        rules.year_min = y;
        rules.year_max = y;
      }
    }
  }

  // BPM range like "118-128"
  if (bpmRange) {
    const lo = parseInt(bpmRange[1]);
    const hi = parseInt(bpmRange[2]);
    if (lo >= 40 && lo <= 200 && hi >= 40 && hi <= 220 && !(rules.year_min && rules.year_max)) {
      rules.bpm_min = lo;
      rules.bpm_max = hi;
    }
  }

  // "under 95 bpm" / "over 120"
  const under = lower.match(/\bunder\s*(\d{2,3})\b/);
  const over = lower.match(/\bover\s*(\d{2,3})\b/);
  if (under) rules.bpm_max = parseInt(under[1]);
  if (over) rules.bpm_min = parseInt(over[1]);

  if (/\b(late night|chill|slow|mellow|smooth|sleepy)\b/.test(lower)) {
    if (rules.bpm_max == null) rules.bpm_max = 95;
  }
  if (/\b(peak hour|peak|high energy|club|banger|hype|workout)\b/.test(lower)) {
    if (rules.bpm_min == null) rules.bpm_min = 118;
  }
  if (/\b(midtempo|mid-tempo|mid tempo|groove|bounce)\b/.test(lower)) {
    if (rules.bpm_min == null) rules.bpm_min = 95;
    if (rules.bpm_max == null) rules.bpm_max = 115;
  }
  if (/\b(uptempo|up-tempo|fast)\b/.test(lower)) {
    if (rules.bpm_min == null) rules.bpm_min = 120;
  }

  if (/\b(hits|popular|mainstream|radio|top 40)\b/.test(lower)) rules.popularity_min = 60;
  if (/\b(underground|deep cuts|slept on|obscure|rare|b-sides|bsides)\b/.test(lower))
    rules.popularity_max = 35;

  for (const [canonical, aliases] of GENRE_KEYWORDS) {
    if (aliases.some((a) => lower.includes(` ${a} `) || lower.includes(` ${a},`))) {
      if (!rules.genres.includes(canonical)) rules.genres.push(canonical);
    }
  }

  return rules;
}

function describeRules(r: SmartCrateRules): string {
  const parts: string[] = [];
  if (r.year_min && r.year_max && r.year_min !== r.year_max) {
    parts.push(`${r.year_min}–${r.year_max}`);
  } else if (r.year_min) parts.push(`${r.year_min}`);
  else if (r.year_max) parts.push(`≤ ${r.year_max}`);
  if (r.genres.length) parts.push(`genre: ${r.genres.join("/")}`);
  if (r.subgenres.length) parts.push(`sub: ${r.subgenres.join("/")}`);
  if (r.artists.length) parts.push(`artists: ${r.artists.join(", ")}`);
  if (r.bpm_min != null && r.bpm_max != null) parts.push(`${r.bpm_min}–${r.bpm_max} bpm`);
  else if (r.bpm_min != null) parts.push(`≥${r.bpm_min} bpm`);
  else if (r.bpm_max != null) parts.push(`<${r.bpm_max} bpm`);
  if (r.popularity_min != null && r.popularity_max != null)
    parts.push(`pop ${r.popularity_min}–${r.popularity_max}`);
  else if (r.popularity_min != null) parts.push(`pop ≥${r.popularity_min}`);
  else if (r.popularity_max != null) parts.push(`pop ≤${r.popularity_max}`);
  if (r.title_contains) parts.push(`title:"${r.title_contains}"`);
  if (r.limit) parts.push(`limit ${r.limit}`);
  return parts.length ? parts.join(" · ") : "no filters";
}
