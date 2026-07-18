import { useState, type CSSProperties, type ReactNode } from "react";

export const Kbd = ({ children }: { children: ReactNode }) => (
  <span
    style={{
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      minWidth: 18,
      height: 18,
      padding: "0 5px",
      fontFamily: "var(--font-mono)",
      fontSize: 10,
      color: "var(--color-text-2)",
      background: "rgba(255,255,255,0.04)",
      border: "0.5px solid rgba(255,255,255,0.12)",
      borderRadius: 3,
      letterSpacing: 0,
    }}
  >
    {children}
  </span>
);

export const Eyebrow = ({ children, style = {} }: { children: ReactNode; style?: CSSProperties }) => (
  <div
    style={{
      fontFamily: "var(--font-mono)",
      fontSize: 10,
      letterSpacing: "0.16em",
      textTransform: "uppercase",
      color: "var(--color-text-3)",
      ...style,
    }}
  >
    {children}
  </div>
);

export const Rule = ({ style = {} }: { style?: CSSProperties }) => (
  <div style={{ height: 0.5, background: "rgba(255,255,255,0.07)", width: "100%", ...style }} />
);

type ButtonKind = "ghost" | "solid" | "primary" | "danger";
type ButtonSize = "sm" | "md" | "lg";

export function DjcButton({
  children,
  onClick,
  kind = "ghost",
  hint,
  size = "md",
  style = {},
  disabled,
  icon,
  iconRight,
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  kind?: ButtonKind;
  hint?: string;
  size?: ButtonSize;
  style?: CSSProperties;
  disabled?: boolean;
  icon?: ReactNode;
  iconRight?: ReactNode;
  title?: string;
}) {
  const [hover, setHover] = useState(false);
  const [down, setDown] = useState(false);
  const sizes = {
    sm: { h: 26, px: 10, fs: 12 },
    md: { h: 32, px: 14, fs: 13 },
    lg: { h: 40, px: 18, fs: 14 },
  }[size];

  const kinds: Record<ButtonKind, { bg: string; color: string; border: string }> = {
    ghost: {
      bg: hover ? "rgba(255,255,255,0.05)" : "transparent",
      color: "var(--color-text)",
      border: "0.5px solid rgba(255,255,255,0.12)",
    },
    solid: {
      bg: hover ? "#242428" : "var(--color-surface-3)",
      color: "var(--color-text)",
      border: "0.5px solid rgba(255,255,255,0.08)",
    },
    primary: {
      bg: hover ? "var(--color-accent-2)" : "var(--color-accent)",
      color: "#0b0b0c",
      border: "none",
    },
    danger: {
      bg: hover ? "rgba(224,97,74,0.12)" : "transparent",
      color: "var(--color-err)",
      border: "0.5px solid rgba(224,97,74,0.35)",
    },
  };
  const k = kinds[kind];

  return (
    <button
      onClick={disabled ? undefined : onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => {
        setHover(false);
        setDown(false);
      }}
      onMouseDown={() => setDown(true)}
      onMouseUp={() => setDown(false)}
      disabled={disabled}
      title={title}
      style={{
        position: "relative",
        height: sizes.h,
        padding: `0 ${sizes.px}px`,
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        fontFamily: "var(--font-sans)",
        fontSize: sizes.fs,
        fontWeight: 500,
        background: k.bg,
        color: k.color,
        border: k.border,
        borderRadius: 6,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
        transform: down ? "translateY(0.5px)" : "translateY(0)",
        boxShadow: down ? "none" : kind === "primary" ? "0 1px 0 rgba(0,0,0,0.4)" : "none",
        transition: "background 80ms ease, transform 60ms ease",
        whiteSpace: "nowrap",
        ...style,
      }}
    >
      {icon}
      <span>{children}</span>
      {iconRight}
      {hint && hover && !disabled && (
        <span
          style={{
            position: "absolute",
            left: "50%",
            top: "calc(100% + 6px)",
            transform: "translateX(-50%)",
            display: "flex",
            gap: 3,
            alignItems: "center",
            background: "#000",
            border: "0.5px solid rgba(255,255,255,0.14)",
            padding: "4px 6px",
            borderRadius: 4,
            fontFamily: "var(--font-mono)",
            fontSize: 10,
            color: "var(--color-text-2)",
            whiteSpace: "nowrap",
            zIndex: 50,
            pointerEvents: "none",
          }}
        >
          {hint.split("+").map((s, i) => (
            <Kbd key={i}>{s}</Kbd>
          ))}
        </span>
      )}
    </button>
  );
}

export function FolderRow({
  label,
  path,
  onPick,
  status = "ok",
  hint,
}: {
  label: string;
  path: string;
  onPick?: () => void;
  status?: "ok" | "empty";
  hint?: string;
}) {
  const [hover, setHover] = useState(false);
  const ok = status === "ok" && !!path;
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 14,
        padding: "14px 16px",
        background: hover ? "rgba(255,255,255,0.02)" : "transparent",
        border: "0.5px solid rgba(255,255,255,0.08)",
        borderRadius: 8,
        transition: "background 100ms ease",
      }}
    >
      <div
        style={{
          width: 28,
          height: 28,
          flexShrink: 0,
          borderRadius: 6,
          background: "var(--color-surface-2)",
          border: "0.5px solid rgba(255,255,255,0.08)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: ok ? "var(--color-accent)" : "var(--color-text-3)",
        }}
      >
        <Icon.Folder />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 3 }}>
          <Eyebrow>{label}</Eyebrow>
          {ok ? (
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 9,
                letterSpacing: "0.1em",
                color: "var(--color-ok)",
                textTransform: "uppercase",
              }}
            >
              ● linked
            </span>
          ) : (
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 9,
                letterSpacing: "0.1em",
                color: "var(--color-text-3)",
                textTransform: "uppercase",
              }}
            >
              ◌ not set
            </span>
          )}
        </div>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            color: ok ? "var(--color-text)" : "var(--color-text-3)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {path || "—"}
        </div>
        {hint && <div style={{ fontSize: 11, color: "var(--color-text-3)", marginTop: 4 }}>{hint}</div>}
      </div>
      {onPick && (
        <DjcButton size="sm" kind="ghost" onClick={onPick}>
          {ok ? "Change…" : "Choose…"}
        </DjcButton>
      )}
    </div>
  );
}

export function InfoRow({ k, v, mono = false }: { k: string; v: ReactNode; mono?: boolean }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "baseline",
        padding: "6px 0",
        gap: 20,
      }}
    >
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10,
          letterSpacing: "0.14em",
          textTransform: "uppercase",
          color: "var(--color-text-3)",
        }}
      >
        {k}
      </div>
      <div
        style={{
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
          fontSize: 12,
          color: "var(--color-text)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {v}
      </div>
    </div>
  );
}

export function Modal({
  title,
  subtitle,
  children,
  onClose,
  width = 560,
  footer,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  onClose: () => void;
  width?: number;
  footer?: ReactNode;
}) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.55)",
        backdropFilter: "blur(4px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 30,
      }}
    >
      <div
        style={{
          width,
          maxWidth: "92vw",
          maxHeight: "86%",
          display: "flex",
          flexDirection: "column",
          background: "var(--color-surface)",
          border: "0.5px solid rgba(255,255,255,0.1)",
          borderRadius: 10,
          boxShadow: "0 20px 60px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.03)",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 12,
            padding: "14px 18px",
            borderBottom: "0.5px solid rgba(255,255,255,0.07)",
          }}
        >
          <div style={{ flex: 1, minWidth: 0 }}>
            <Eyebrow>{title}</Eyebrow>
            {subtitle && (
              <div style={{ fontSize: 13, color: "var(--color-text-2)", marginTop: 2 }}>
                {subtitle}
              </div>
            )}
          </div>
          <button
            onClick={onClose}
            style={{
              width: 24,
              height: 24,
              borderRadius: 4,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              color: "var(--color-text-3)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <svg width="12" height="12" viewBox="0 0 12 12">
              <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          </button>
        </div>
        <div style={{ flex: 1, overflow: "auto", padding: 18 }}>{children}</div>
        {footer && (
          <div
            style={{
              display: "flex",
              gap: 8,
              justifyContent: "flex-end",
              padding: "12px 16px",
              borderTop: "0.5px solid rgba(255,255,255,0.07)",
              background: "rgba(255,255,255,0.01)",
            }}
          >
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}

export const Icon = {
  Sync: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path
        d="M3 8a5 5 0 019-3M13 8a5 5 0 01-9 3M13 2v3h-3M3 14v-3h3"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
  Wand: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path
        d="M2 14L10 6M10 2l1 2 2 1-2 1-1 2-1-2-2-1 2-1 1-2zM13 9l.5 1 1 .5-1 .5-.5 1-.5-1-1-.5 1-.5.5-1z"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinejoin="round"
      />
    </svg>
  ),
  Settings: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <circle cx="8" cy="8" r="2" stroke="currentColor" strokeWidth="1.2" />
      <path
        d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.5 1.5M11.5 11.5L13 13M3 13l1.5-1.5M11.5 4.5L13 3"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  ),
  Check: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path d="M3 8l3 3 7-7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  Plus: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" {...p}>
      <path d="M6 2v8M2 6h8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  ),
  Minus: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" {...p}>
      <path d="M2 6h8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  ),
  Arrow: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" {...p}>
      <path d="M3 6h6M6 3l3 3-3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  Folder: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path
        d="M1.5 4a1.5 1.5 0 011.5-1.5h3l1.5 1.5h5A1.5 1.5 0 0114 5.5v6A1.5 1.5 0 0112.5 13h-9A1.5 1.5 0 012 11.5V4z"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  ),
  Spotify: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeWidth="1" />
      <path
        d="M4.5 6.5c2-.6 4.5-.4 6.5.8M5 8.5c1.5-.4 3.5-.2 5 .8M5.5 10.5c1-.3 2.5-.2 3.5.5"
        stroke="currentColor"
        strokeWidth="1"
        strokeLinecap="round"
      />
    </svg>
  ),
  Diff: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path d="M5 2v12M11 2v12M2 5h6M8 11h6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  ),
  Trash: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" {...p}>
      <path
        d="M3 4h10M6 4V3a1 1 0 011-1h2a1 1 0 011 1v1M5 4l.5 9a1 1 0 001 1h3a1 1 0 001-1l.5-9"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  ),
  Save: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" {...p}>
      <path
        d="M3 3h8l2 2v8a1 1 0 01-1 1H3a1 1 0 01-1-1V4a1 1 0 011-1zM5 3v4h6V3M5 14v-4h6v4"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  ),
  Eye: (p: React.SVGProps<SVGSVGElement>) => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" {...p}>
      <path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5z" stroke="currentColor" strokeWidth="1.2" />
      <circle cx="8" cy="8" r="2" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  ),
};
