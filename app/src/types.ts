export interface Host {
  id: string;
  name: string;
  addr: string;
  ftp_port: number;
  ftp_user: string;
  ftp_pass: string;
}

export interface StorageOut {
  id: number;
  description: string;
  kind: string;
  free_bytes: number;
  total_bytes: number;
  writable: boolean;
}

export interface DeviceStatus {
  mtp_ok: boolean;
  mtp_serial: string;
  mtp_product: string;
  storages: StorageOut[];
  tinfoil_ok: boolean;
  ftp_ok: boolean;
  ftp_ms: number;
  syscon_ok: boolean;
  message: string;
}

export type InstallTarget = "sd" | "nand" | "tinfoil" | "network";

export interface Config {
  hosts: Host[];
  active_host: string | null;
  mtp_serial: string;
  install_target: InstallTarget;
  syscon_addr: string;
  syscon_port: number;
  download_dir: string;
  rd_api_key: string;
  tb_api_key: string;
  gamesearch_base: string;
  covers_base: string;
  aria2_path: string;
  split_bytes: number;
  mcp_port: number;
  mcp_enabled: boolean;
  auto_install: boolean;
  pad_release_first: boolean;
}

export interface Job {
  id: number;
  kind: string;
  label: string;
  state: "running" | "done" | "failed" | "cancelled";
  done_bytes: number;
  total_bytes: number;
  message: string;
  started_at: number;
  finished_at: number | null;
}

export interface Check {
  name: string;
  ok: boolean;
  detail: string;
}

export interface Entry {
  name: string;
  title_id: string | null;
  size: number;
  kind: "installed" | "package" | "homebrew";
  format: string;
  location: string;
  path: string;
  cover: string | null;
}

export interface RemoteEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface LocalEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface GameHit {
  id: string;
  gameSlug: string;
  title: string;
  platform: string;
  source: string;
  host: string;
  url: string;
  linkType: string;
  supportedBy: string[];
  rawgCover: string | null;
  cover: string | null;
  rawgRating: number | null;
  rawgReleased: string | null;
  linkCount: number;
  size: number | null;
}

export interface IaItem {
  id: string;
  title: string;
  size_bytes: number;
  files_count: number;
}

export interface IaFile {
  url: string;
  name: string;
  size_bytes: number;
}

export interface NspInfo {
  path: string;
  name: string;
  size: number;
  format: string;
  title_id: string | null;
  parts: number;
  is_part: boolean;
}

export interface InstallReport {
  transport: string;
  destination: string;
  bytes: number;
}

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  repo: string;
  asset: string;
  latest_tag: string | null;
  nro_url: string | null;
  published_at: string | null;
  size_bytes: number;
}

export interface Snapshot {
  hosts: Host[];
  active: string | null;
  config: Config;
  status: DeviceStatus;
  doctor: Check[];
  jobs: Job[];
  mcp_port: number;
  mcp_enabled: boolean;
}

export const PAGE_TITLES: Record<string, string> = {
  home: "Console",
  devices: "Devices",
  library: "Library",
  controller: "Controller",
  store: "Game Index",
  downloads: "Downloads",
  files: "Files",
  convert: "Convert",
  homebrew: "Homebrew",
  settings: "Settings",
};

export const STORAGE_LABELS: Record<string, string> = {
  sd: "SD card",
  nand_user: "NAND USER",
  nand_system: "NAND SYSTEM",
  sd_install: "SD install",
  nand_install: "NAND install",
  installed: "Installed games",
  saves: "Saves",
  album: "Album",
  gamecard: "Gamecard",
};

export const PAD_BUTTONS = [
  "y",
  "x",
  "b",
  "a",
  "l",
  "r",
  "zl",
  "zr",
  "minus",
  "plus",
  "lstick",
  "rstick",
  "up",
  "down",
  "left",
  "right",
  "capture",
  "home",
];
