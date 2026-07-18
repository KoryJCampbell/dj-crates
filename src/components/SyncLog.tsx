import { useEffect, useRef } from "react";
import type { ProgressPayload } from "../types";
import { DjcButton, Eyebrow } from "./primitives";

type Level = "info" | "ok" | "warn" | "err";
export type LogLine = {
  t: string;
  phase: string;
  lvl: Level;
  msg: string;
};

const PHASE_TAG: Record<string, string> = {
  starting: "boot",
  counting: "scan",
  scanning: "scan",
  analyzing_energy: "nrg",
  writing_crates: "write",
  writing_db: "db",
  writing_cache: "cache",
  smart_matching: "rule",
  done: "done",
};

export function phaseTag(stage: string): string {
  return PHASE_TAG[stage] ?? stage.slice(0, 5);
}

export function logLineFromProgress(p: ProgressPayload, idx: number): LogLine {
  const now = new Date().toLocaleTimeString("en-US", { hour12: false });
  const total = p.total > 0 ? ` · ${p.current.toLocaleString()}/${p.total.toLocaleString()}` : "";
  const lvl: Level = p.stage === "done" ? "ok" : "info";
  return {
    t: `${now}.${String(idx).padStart(3, "0")}`,
    phase: phaseTag(p.stage),
    lvl,
    msg: `${p.message || p.stage}${total}`,
  };
}

const LVL_COLOR: Record<Level, string> = {
  info: "var(--color-text-2)",
  ok: "var(--color-ok)",
  warn: "var(--color-warn)",
  err: "var(--color-err)",
};

export function SyncLog({
  lines,
  phase,
  pct,
  opsLabel,
  onCancel,
  cancelable = true,
}: {
  lines: LogLine[];
  phase: string;
  pct: number; // 0..1
  opsLabel: string;
  onCancel?: () => void;
  cancelable?: boolean;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 24,
          padding: "18px 24px",
          borderBottom: "0.5px solid var(--color-rule)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: "var(--color-accent)",
              boxShadow: "0 0 8px var(--color-accent)",
              animation: "djc-pulse 1s ease-in-out infinite",
            }}
          />
          <Eyebrow style={{ color: "var(--color-accent)" }}>
            {phase.toUpperCase()} · in progress
          </Eyebrow>
        </div>
        <div style={{ flex: 1 }} />
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--color-text-3)",
          }}
        >
          <span style={{ color: "var(--color-text)" }}>{Math.round(pct * 100)}%</span> · {opsLabel}
        </div>
      </div>

      <div
        style={{
          height: 10,
          background: "rgba(255,255,255,0.06)",
          position: "relative",
          borderRadius: 3,
          margin: "0 20px 4px 20px",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            height: "100%",
            width: `${pct * 100}%`,
            background: "var(--color-accent)",
            boxShadow: "0 0 8px var(--color-accent)",
            transition: "width 300ms ease",
          }}
        />
      </div>

      <div
        ref={scrollRef}
        style={{
          flex: 1,
          overflow: "auto",
          padding: "18px 24px",
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          lineHeight: 1.75,
          background: "linear-gradient(180deg, #0a0a0b 0%, #0d0d0e 100%)",
        }}
      >
        {lines.map((l, i) => (
          <div
            key={i}
            style={{
              display: "grid",
              gridTemplateColumns: "110px 56px 44px 1fr",
              gap: 14,
              color: LVL_COLOR[l.lvl],
            }}
          >
            <span style={{ color: "var(--color-text-4)" }}>{l.t}</span>
            <span style={{ color: "var(--color-text-3)" }}>[{l.phase}]</span>
            <span
              style={{
                color: LVL_COLOR[l.lvl],
                textTransform: "uppercase",
                fontSize: 10,
                letterSpacing: "0.1em",
                alignSelf: "center",
              }}
            >
              {l.lvl}
            </span>
            <span>{l.msg}</span>
          </div>
        ))}
        <div
          style={{
            color: "var(--color-accent)",
            display: "flex",
            alignItems: "center",
            gap: 2,
            marginTop: 4,
          }}
        >
          <span>▸ </span>
          <span
            style={{
              width: 7,
              height: 13,
              background: "var(--color-accent)",
              animation: "djc-blink 1s steps(2) infinite",
            }}
          />
        </div>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "12px 20px",
          borderTop: "0.5px solid var(--color-rule)",
        }}
      >
        <Eyebrow>streaming live from sync engine</Eyebrow>
        <div style={{ flex: 1 }} />
        {cancelable && onCancel && (
          <DjcButton kind="danger" size="sm" onClick={onCancel}>
            Cancel sync
          </DjcButton>
        )}
      </div>
    </div>
  );
}
