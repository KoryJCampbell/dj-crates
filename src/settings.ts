import { LazyStore } from "@tauri-apps/plugin-store";
import type { Config, SmartCratePreset } from "./types";

const store = new LazyStore("settings.json");

const DEFAULT_CONFIG: Config = {
  cratesRoot: "",
  seratoPath: "",
  unsortedPath: "",
  pythonVenv: "",
  scriptsDir: "",
  spotifyClientId: "",
  spotifyClientSecret: "",
};

export const DEFAULT_PRESETS: SmartCratePreset[] = [
  {
    id: "2000s-hip-hop",
    name: "2000s Hip-Hop",
    description: "Year 2000–2009, genre contains hip-hop/rap",
    builtin: true,
    rules: {
      year_min: 2000,
      year_max: 2009,
      genres: ["hip-hop", "rap"],
      subgenres: [],
      artists: [],
    },
  },
  {
    id: "90s-rnb",
    name: "90s R&B",
    description: "Year 1990–1999, genre R&B",
    builtin: true,
    rules: {
      year_min: 1990,
      year_max: 1999,
      genres: ["r&b", "rnb"],
      subgenres: [],
      artists: [],
    },
  },
  {
    id: "late-night-vibes",
    name: "Late Night Vibes",
    description: "Slow soul/R&B, 70–95 BPM",
    builtin: true,
    rules: {
      bpm_min: 70,
      bpm_max: 95,
      genres: ["soul", "r&b", "neo-soul"],
      subgenres: [],
      artists: [],
    },
  },
  {
    id: "peak-hour",
    name: "Peak Hour",
    description: "High-energy house/hip-hop, 118–130 BPM",
    builtin: true,
    rules: {
      bpm_min: 118,
      bpm_max: 130,
      genres: ["house", "hip-hop", "afrobeats"],
      subgenres: [],
      artists: [],
    },
  },
  {
    id: "2010s-hits",
    name: "2010s",
    description: "Anything from 2010–2019",
    builtin: true,
    rules: {
      year_min: 2010,
      year_max: 2019,
      genres: [],
      subgenres: [],
      artists: [],
    },
  },
  {
    id: "afrobeats-rising",
    name: "Afrobeats + Amapiano",
    description: "African lane — Afrobeats, Amapiano, Highlife",
    builtin: true,
    rules: {
      genres: ["afrobeats", "amapiano", "highlife", "afro"],
      subgenres: [],
      artists: [],
    },
  },
];

export async function loadConfig(): Promise<Config> {
  const saved = await store.get<Config>("config");
  return { ...DEFAULT_CONFIG, ...(saved ?? {}) };
}

export async function saveConfig(config: Config): Promise<void> {
  await store.set("config", config);
  await store.save();
}

export async function loadPresets(): Promise<SmartCratePreset[]> {
  const saved = await store.get<SmartCratePreset[]>("presets");
  return saved && saved.length > 0 ? saved : DEFAULT_PRESETS;
}

export async function savePresets(presets: SmartCratePreset[]): Promise<void> {
  await store.set("presets", presets);
  await store.save();
}

export function isConfigured(config: Config): boolean {
  return Boolean(config.cratesRoot && config.seratoPath);
}
