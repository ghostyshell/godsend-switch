// Typed bridge to the Tauri runtime. In ?mock=1 mode mock.ts installs
// window.__TAURI_INTERNALS__ so this file works unchanged in a browser.

import type {
  CatalogEntry,
  Check,
  Config,
  DeviceStatus,
  Entry,
  GameHit,
  Host,
  IaFile,
  IaItem,
  Job,
  LocalEntry,
  NspInfo,
  RemoteEntry,
  Snapshot,
  StorageOut,
} from "./types";

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
  return internals.invoke(cmd, args);
}

type UnlistenFn = () => void;

export function listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  const api = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
  const wrapped = (e: { payload: T }) => handler(e.payload);
  return api
    .invoke("plugin:event|listen", {
      event,
      handler: api.transformCallback(wrapped),
      target: { kind: "Any" },
    })
    .then((id: number) => () => {
      void api.invoke("plugin:event|unlisten", { event, eventId: id });
    });
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  probe: () => invoke<DeviceStatus>("probe"),
  discover: (subnet: string, port: number) => invoke<Host[]>("discover", { subnet, port }),

  hostAdd: (host: Host) => invoke<Config>("host_add", { host }),
  hostRemove: (id: string) => invoke<Config>("host_remove", { id }),
  hostSetActive: (id: string) => invoke<Config>("host_set_active", { id }),

  storages: () => invoke<StorageOut[]>("storages"),
  library: (source: "installed" | "local") => invoke<Entry[]>("library", { source }),
  listStorage: (storage: string, path: string) =>
    invoke<RemoteEntry[]>("list_storage", { storage, path }),
  storageMkdir: (storage: string, path: string) =>
    invoke<void>("storage_mkdir", { storage, path }),
  storageDelete: (storage: string, path: string) =>
    invoke<void>("storage_delete", { storage, path }),
  uploadLocal: (local: string, storage: string, dir: string) =>
    invoke<number>("upload_local", { local, storage, dir }),
  downloadRemote: (storage: string, path: string, localDir: string) =>
    invoke<number>("download_remote", { storage, path, localDir }),

  installPath: (path: string, via?: string) =>
    invoke<number>("install_path", { path, via: via ?? null }),
  tinfoilAttached: () => invoke<{ product: string; serial: string } | null>("tinfoil_attached"),
  padSet: (addr: string, port: number, buttons: string[], left?: [number, number], right?: [number, number]) =>
    invoke<void>("pad_set", { addr, port, buttons, left: left ?? null, right: right ?? null }),
  padRelease: (addr: string, port: number) => invoke<void>("pad_release", { addr, port }),

  homebrewCatalog: (refresh: boolean) => invoke<CatalogEntry[]>("homebrew_catalog", { refresh }),
  homebrewInstall: (id: string) => invoke<number>("homebrew_install", { id }),

  storeSearch: (query: string, limit = 20) =>
    invoke<GameHit[]>("store_search", { query, limit }),
  iaSearch: (query: string, rows = 15) => invoke<IaItem[]>("ia_search", { query, rows }),
  iaFiles: (id: string) => invoke<IaFile[]>("ia_files", { id }),
  storeDownload: (slug: string) => invoke<number>("store_download", { slug }),
  downloadUrl: (url: string, name: string) => invoke<number>("download_url", { url, name }),

  downloadsList: () => invoke<LocalEntry[]>("downloads_list"),
  downloadOpen: (path: string) => invoke<void>("download_open", { path }),
  downloadDelete: (path: string) => invoke<void>("download_delete", { path }),
  downloadPush: (path: string) => invoke<number>("download_push", { path }),

  localRoot: () => invoke<string>("local_root"),
  fmLocal: (dir: string) => invoke<LocalEntry[]>("fm_local", { dir }),
  fmMkdir: (dir: string) => invoke<void>("fm_mkdir", { dir }),
  fmDelete: (path: string) => invoke<void>("fm_delete", { path }),

  convertInspect: (path: string) => invoke<NspInfo>("convert_inspect", { path }),
  convertSplit: (path: string) => invoke<number>("convert_split", { path }),
  convertMerge: (base: string, out: string) => invoke<number>("convert_merge", { base, out }),

  configGet: () => invoke<Config>("config_get"),
  configSet: (patch: Record<string, unknown>) => invoke<Config>("config_set", { patch }),
  doctorRun: () => invoke<Check[]>("doctor_run"),

  jobs: () => invoke<Job[]>("jobs"),
  jobCancel: (id: number) => invoke<boolean>("job_cancel", { id }),
  jobsClear: () => invoke<void>("jobs_clear"),

  mcpInfo: () => invoke<[boolean, number]>("mcp_info"),
  openDataDir: () => invoke<void>("open_data_dir"),
  reveal: (path: string) => invoke<void>("reveal", { path }),
};
