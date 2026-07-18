import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Config } from "../types";
import { DjcButton, Eyebrow, FolderRow, Modal } from "./primitives";

type Props = {
  config: Config;
  onSave: (config: Config) => Promise<void>;
  onClose: () => void;
  forceInitialSetup?: boolean;
};

export function SettingsModal({ config, onSave, onClose, forceInitialSetup }: Props) {
  const [draft, setDraft] = useState<Config>(config);
  const [saving, setSaving] = useState(false);

  const pickFolder = async (key: keyof Config, title: string) => {
    const picked = await open({ title, directory: true, multiple: false });
    if (typeof picked === "string") setDraft((d) => ({ ...d, [key]: picked }));
  };

  const canSave = Boolean(draft.cratesRoot && draft.seratoPath);

  const handleSave = async () => {
    if (!canSave) return;
    setSaving(true);
    try {
      await onSave(draft);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={forceInitialSetup ? "First run" : "Settings"}
      subtitle={forceInitialSetup ? "Point us at two folders and you're done." : "Two folders. That's the whole thing."}
      onClose={forceInitialSetup ? () => {} : onClose}
      width={540}
      footer={
        <>
          {!forceInitialSetup && (
            <DjcButton kind="ghost" onClick={onClose}>
              Cancel
            </DjcButton>
          )}
          <DjcButton kind="primary" onClick={handleSave} disabled={!canSave || saving}>
            {saving ? "Saving…" : forceInitialSetup ? "Let's go" : "Save"}
          </DjcButton>
        </>
      }
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <FolderRow
          label="01 · Crates library"
          path={draft.cratesRoot}
          status={draft.cratesRoot ? "ok" : "empty"}
          onPick={() => pickFolder("cratesRoot", "Pick your CRATES folder")}
          hint="The top-level folder containing GENRES, EDITS, etc."
        />
        <FolderRow
          label="02 · Serato folder"
          path={draft.seratoPath}
          status={draft.seratoPath ? "ok" : "empty"}
          onPick={() => pickFolder("seratoPath", "Pick your _Serato_ folder")}
          hint="Usually ~/Music/_Serato_"
        />
        <FolderRow
          label="03 · Unsorted (optional)"
          path={draft.unsortedPath}
          status={draft.unsortedPath ? "ok" : "empty"}
          onPick={() => pickFolder("unsortedPath", "Pick your Unsorted / downloads folder")}
          hint="Where new SoundCloud / yt-dlp pulls land. Empty = skip auto-routing."
        />
      </div>

      <div
        style={{
          marginTop: 20,
          padding: "14px 16px",
          border: "0.5px dashed rgba(255,255,255,0.08)",
          borderRadius: 8,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: draft.seratoPath ? "var(--color-ok)" : "var(--color-text-3)",
            }}
          />
          <Eyebrow>
            {draft.seratoPath ? "Serato linked · ready to sync" : "Pick a Serato folder to detect"}
          </Eyebrow>
        </div>
        <div
          style={{
            fontSize: 12,
            color: "var(--color-text-3)",
            marginTop: 6,
            lineHeight: 1.5,
          }}
        >
          Nothing else to configure. Spotify lives inside Smart Crates. Energy analysis runs
          automatically when available.
        </div>
      </div>

      {!forceInitialSetup && (
        <div
          style={{
            marginTop: 16,
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <Eyebrow>DJ CRATES</Eyebrow>
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--color-text-3)", letterSpacing: "0.14em" }}>
            ONE-WAY · SAFE · DRY-RUN BY DEFAULT
          </div>
        </div>
      )}
    </Modal>
  );
}
