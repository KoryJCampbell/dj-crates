import { useEffect, useState } from "react";

export type SyncState = "idle" | "scanning" | "syncing" | "done" | "error";

const LABELS: Record<SyncState, string> = {
  idle: "SYNC",
  scanning: "SCANNING",
  syncing: "WRITING",
  done: "DONE",
  error: "RETRY",
};

const SUBS: Record<SyncState, string> = {
  idle: "press to begin",
  scanning: "reading folders",
  syncing: "writing serato",
  done: "library synced",
  error: "see log",
};

export function SyncButton({
  state = "idle",
  onClick,
  size = 220,
  disabled,
}: {
  state?: SyncState;
  onClick?: () => void;
  size?: number;
  disabled?: boolean;
}) {
  const [hover, setHover] = useState(false);
  const [down, setDown] = useState(false);
  const [ping, setPing] = useState(false);

  useEffect(() => {
    if (state === "done") {
      setPing(true);
      const t = setTimeout(() => setPing(false), 1200);
      return () => clearTimeout(t);
    }
  }, [state]);

  const active = state !== "idle" && state !== "done" && state !== "error";
  const label = LABELS[state];
  const sub = SUBS[state];

  return (
    <div style={{ position: "relative", width: size, height: size, userSelect: "none" }}>
      {state === "idle" && (
        <div
          style={{
            position: "absolute",
            inset: -20,
            borderRadius: "50%",
            background: "radial-gradient(circle, rgba(255,90,31,0.08) 0%, transparent 60%)",
            animation: "djc-breathe 3.2s ease-in-out infinite",
            pointerEvents: "none",
          }}
        />
      )}
      {ping && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "50%",
            border: "1px solid var(--color-accent)",
            animation: "djc-ping 1.1s cubic-bezier(0.2,0.7,0.3,1) forwards",
            pointerEvents: "none",
          }}
        />
      )}
      <div
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => {
          setHover(false);
          setDown(false);
        }}
        onMouseDown={() => setDown(true)}
        onMouseUp={() => setDown(false)}
        onClick={disabled ? undefined : onClick}
        style={{
          position: "relative",
          width: "100%",
          height: "100%",
          borderRadius: "50%",
          background: "linear-gradient(180deg, #2a2a2f 0%, #0d0d0e 100%)",
          padding: 6,
          boxShadow: down
            ? "0 2px 6px rgba(0,0,0,0.6), inset 0 1px 1px rgba(255,255,255,0.04)"
            : "0 20px 40px rgba(0,0,0,0.6), 0 6px 12px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.08), inset 0 -1px 0 rgba(0,0,0,0.5)",
          transform: down ? "translateY(2px)" : "translateY(0)",
          transition: "transform 80ms ease, box-shadow 80ms ease",
          cursor: disabled ? "wait" : "pointer",
          opacity: disabled ? 0.7 : 1,
        }}
      >
        <div
          style={{
            position: "relative",
            width: "100%",
            height: "100%",
            borderRadius: "50%",
            background:
              state === "error"
                ? "radial-gradient(circle at 50% 35%, #2a1512 0%, #0b0b0c 70%)"
                : state === "done"
                ? "radial-gradient(circle at 50% 35%, #1a2218 0%, #0b0b0c 70%)"
                : hover || active
                ? "radial-gradient(circle at 50% 35%, #1f1a17 0%, #0b0b0c 70%)"
                : "radial-gradient(circle at 50% 35%, #17171a 0%, #0b0b0c 70%)",
            boxShadow:
              "inset 0 2px 6px rgba(0,0,0,0.7), inset 0 -1px 0 rgba(255,255,255,0.04)",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 6,
            transition: "background 160ms ease",
          }}
        >
          <svg
            style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
            viewBox="0 0 100 100"
          >
            <circle
              cx="50"
              cy="50"
              r="47"
              fill="none"
              stroke={active ? "rgba(255,90,31,0.15)" : "rgba(255,255,255,0.04)"}
              strokeWidth="0.6"
            />
            {active && (
              <circle
                cx="50"
                cy="50"
                r="47"
                fill="none"
                stroke="var(--color-accent)"
                strokeWidth="0.6"
                strokeDasharray="20 275"
                strokeLinecap="round"
                style={{
                  transformOrigin: "50% 50%",
                  animation: "djc-spin 1.4s linear infinite",
                }}
              />
            )}
          </svg>

          <div
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background:
                state === "error"
                  ? "var(--color-err)"
                  : state === "done"
                  ? "var(--color-ok)"
                  : active
                  ? "var(--color-accent)"
                  : "var(--color-text-3)",
              boxShadow: active || state === "done" ? "0 0 8px currentColor" : "none",
              transition: "background 160ms",
            }}
          />

          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: size > 180 ? 26 : 20,
              fontWeight: 500,
              letterSpacing: "0.2em",
              color:
                state === "error"
                  ? "var(--color-err)"
                  : state === "done"
                  ? "var(--color-ok)"
                  : active
                  ? "var(--color-accent)"
                  : "var(--color-text)",
              transition: "color 160ms",
            }}
          >
            {label}
          </div>

          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 9,
              letterSpacing: "0.18em",
              textTransform: "uppercase",
              color: "var(--color-text-3)",
            }}
          >
            {sub}
          </div>
        </div>

        <svg
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            pointerEvents: "none",
          }}
          viewBox="0 0 100 100"
        >
          {Array.from({ length: 24 }).map((_, i) => {
            const a = (i / 24) * Math.PI * 2 - Math.PI / 2;
            const r1 = 49.2;
            const r2 = i % 6 === 0 ? 47.5 : 48.3;
            const x1 = 50 + Math.cos(a) * r1;
            const y1 = 50 + Math.sin(a) * r1;
            const x2 = 50 + Math.cos(a) * r2;
            const y2 = 50 + Math.sin(a) * r2;
            return (
              <line
                key={i}
                x1={x1}
                y1={y1}
                x2={x2}
                y2={y2}
                stroke="rgba(255,255,255,0.14)"
                strokeWidth="0.2"
              />
            );
          })}
        </svg>
      </div>
    </div>
  );
}
