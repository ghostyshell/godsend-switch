// Mock bridge: installs window.__TAURI_INTERNALS__ with Switch fixture data
// so the UI runs in a plain browser (?mock=1). Never imported by the app.

type Handler = (payload: unknown) => void;

const listeners = new Map<number, { event: string; handler: Handler }>();
let nextCallbackId = 1;

const GB = 1024 * 1024 * 1024;
const MB = 1024 * 1024;

const host = {
  id: "192.168.1.50:5000",
  name: "Living room Switch",
  addr: "192.168.1.50",
  ftp_port: 5000,
  ftp_user: "anonymous",
  ftp_pass: "",
};

let config = {
  hosts: [host],
  active_host: host.id,
  mtp_serial: "",
  install_target: "sd",
  syscon_addr: "192.168.1.50",
  syscon_port: 56789,
  download_dir: "/Users/demo/Downloads/godsend-switch",
  rd_api_key: "",
  tb_api_key: "",
  gamesearch_base: "https://game-debrid.link/api",
  covers_base: "https://covers.debrid.in",
  aria2_path: "",
  split_bytes: 4095 * MB,
  mcp_port: 3939,
  mcp_enabled: true,
  auto_install: true,
  pad_release_first: true,
};

const storages = [
  { id: 1, description: "SD Card", kind: "sd", free_bytes: 187 * GB, total_bytes: 238 * GB, writable: true },
  { id: 2, description: "NAND USER", kind: "nand_user", free_bytes: 20 * GB, total_bytes: 26 * GB, writable: true },
  { id: 3, description: "NAND SYSTEM", kind: "nand_system", free_bytes: 2 * GB, total_bytes: 3 * GB, writable: false },
  { id: 4, description: "Installed games", kind: "installed", free_bytes: 0, total_bytes: 0, writable: false },
  { id: 5, description: "SD install", kind: "sd_install", free_bytes: 187 * GB, total_bytes: 238 * GB, writable: true },
  { id: 6, description: "NAND install", kind: "nand_install", free_bytes: 20 * GB, total_bytes: 26 * GB, writable: true },
  { id: 7, description: "Saves", kind: "saves", free_bytes: 0, total_bytes: 0, writable: false },
  { id: 8, description: "Album", kind: "album", free_bytes: 0, total_bytes: 0, writable: false },
];

const status = {
  mtp_ok: true,
  mtp_serial: "XAW10000000000",
  mtp_product: "Nintendo Switch",
  storages,
  tinfoil_ok: false,
  ftp_ok: true,
  ftp_ms: 9,
  syscon_ok: true,
  message: "",
};

const checks = [
  { name: "MTP responder", ok: true, detail: "Nintendo Switch (8 storages) - serial XAW10000000000" },
  { name: "install storage", ok: true, detail: "SD install / NAND install visible over MTP" },
  { name: "Tinfoil USB", ok: false, detail: "optional: open a Tinfoil-family installer and pick USB" },
  { name: "FTP host", ok: true, detail: "9 ms" },
  { name: "sys-con pad", ok: true, detail: "192.168.1.50:56789" },
  { name: "Download folder", ok: true, detail: "/Users/demo/Downloads/godsend-switch" },
  { name: "aria2c", ok: true, detail: "/opt/homebrew/bin/aria2c" },
  { name: "Debrid", ok: false, detail: "optional: keys unlock debrid download links" },
  { name: "MCP server", ok: true, detail: "http://127.0.0.1:3939/mcp" },
];

const installed = [
  {
    name: "Mario Kart 8 Deluxe [0100152000022000][v0]",
    title_id: "0100152000022000",
    size: 6.8 * GB,
    kind: "installed",
    format: "nsp",
    location: "Installed games",
    path: "/Mario Kart 8 Deluxe [0100152000022000][v0].nsp",
    cover: null,
  },
  {
    name: "The Legend of Zelda: Tears of the Kingdom [0100F2C0115B6000][v0]",
    title_id: "0100F2C0115B6000",
    size: 16.3 * GB,
    kind: "installed",
    format: "nsp",
    location: "Installed games",
    path: "/The Legend of Zelda: Tears of the Kingdom [0100F2C0115B6000][v0].nsp",
    cover: null,
  },
  {
    name: "Animal Crossing: New Horizons [01006F8002326000][v0]",
    title_id: "01006F8002326000",
    size: 7.1 * GB,
    kind: "installed",
    format: "nsp",
    location: "Installed games",
    path: "/Animal Crossing: New Horizons [01006F8002326000][v0].nsp",
    cover: null,
  },
  {
    name: "Super Mario Odyssey [0100000000010000][v0]",
    title_id: "0100000000010000",
    size: 5.7 * GB,
    kind: "installed",
    format: "nsp",
    location: "Installed games",
    path: "/Super Mario Odyssey [0100000000010000][v0].nsp",
    cover: null,
  },
];

const localFiles = [
  {
    name: "Metroid Dread [010093801237C000][v0]",
    title_id: "010093801237C000",
    size: 4.2 * GB,
    kind: "package",
    format: "nsp",
    location: "local",
    path: "/Users/demo/Downloads/godsend-switch/Metroid Dread [010093801237C000][v0].nsp",
    cover: null,
  },
  {
    name: "Splatoon 3 [0100C2500FC20000][v0]",
    title_id: "0100C2500FC20000",
    size: 6.1 * GB,
    kind: "package",
    format: "nsz",
    location: "local",
    path: "/Users/demo/Downloads/godsend-switch/Splatoon 3 [0100C2500FC20000][v0].nsz",
    cover: null,
  },
];

const storageTree: Record<string, { name: string; path: string; is_dir: boolean; size: number }[]> = {
  "sd:/": [
    { name: "switch", path: "/switch", is_dir: true, size: 0 },
    { name: "Nintendo", path: "/Nintendo", is_dir: true, size: 0 },
    { name: "atmosphere", path: "/atmosphere", is_dir: true, size: 0 },
  ],
  "sd:/switch": [
    { name: "ftpd.nro", path: "/switch/ftpd.nro", is_dir: false, size: 1.2 * MB },
    { name: "sys-con.nro", path: "/switch/sys-con.nro", is_dir: false, size: 812 * 1024 },
    { name: "NX-Shell.nro", path: "/switch/NX-Shell.nro", is_dir: false, size: 3.4 * MB },
  ],
  "installed:/": installed.map((g) => ({ name: g.path.slice(1), path: g.path, is_dir: false, size: g.size })),
  "saves:/": [
    { name: "Mario Kart 8 Deluxe", path: "/Mario Kart 8 Deluxe", is_dir: true, size: 0 },
  ],
  "album:/": [
    { name: "2026", path: "/2026", is_dir: true, size: 0 },
  ],
  "sd_install:/": [],
  "nand_install:/": [],
};

const homebrew = [
  {
    id: "hb-appstore",
    name: "Homebrew App Store",
    description: "Browse and install homebrew from the console itself.",
    category: "store",
    repo: "fortheusers/hb-appstore",
    asset: "appstore.nro",
    latest_tag: "v2.4",
    nro_url: "https://example.invalid/appstore.nro",
    published_at: "2026-10-01T00:00:00Z",
    size_bytes: 12 * MB,
  },
  {
    id: "ftpd",
    name: "ftpd",
    description: "Minimal FTP server for the SD card. The app's FTP transport talks to this.",
    category: "tool",
    repo: "mtheall/ftpd",
    asset: "ftpd.nro",
    latest_tag: "v3.1.0",
    nro_url: "https://example.invalid/ftpd.nro",
    published_at: "2026-08-12T00:00:00Z",
    size_bytes: 1.2 * MB,
  },
  {
    id: "sys-con",
    name: "sys-con",
    description: "USB and network controller support. Set network_controller=1 to use the app's virtual pad.",
    category: "system",
    repo: "o0Zz/sys-con",
    asset: "sys-con.nro",
    latest_tag: "v2.0.1",
    nro_url: "https://example.invalid/sys-con.nro",
    published_at: "2026-09-20T00:00:00Z",
    size_bytes: 812 * 1024,
  },
  {
    id: "nx-shell",
    name: "NX-Shell",
    description: "File manager with copy, move, delete and archive support.",
    category: "tool",
    repo: "joel16/NX-Shell",
    asset: "NX-Shell.nro",
    latest_tag: "v3.60",
    nro_url: "https://example.invalid/nx-shell.nro",
    published_at: "2026-07-03T00:00:00Z",
    size_bytes: 3.4 * MB,
  },
  {
    id: "dbi",
    name: "DBI",
    description: "Installer and MTP responder. Its MTP mode is the app's primary USB transport.",
    category: "installer",
    repo: "rashevskyv/dbi",
    asset: "DBI.nro",
    latest_tag: "v658",
    nro_url: "https://example.invalid/dbi.nro",
    published_at: "2026-10-18T00:00:00Z",
    size_bytes: 4.6 * MB,
  },
  {
    id: "awoo",
    name: "Awoo Installer",
    description: "Installs NSP, NSZ, XCI and XCZ over USB or the LAN.",
    category: "installer",
    repo: "Huntereb/Awoo-Installer",
    asset: "Awoo-Installer.nro",
    latest_tag: "v1.4.2",
    nro_url: "https://example.invalid/awoo.nro",
    published_at: "2026-05-11T00:00:00Z",
    size_bytes: 2.1 * MB,
  },
];

const hits = [
  {
    id: "g1",
    gameSlug: "mario-kart-8-deluxe",
    title: "Mario Kart 8 Deluxe",
    platform: "switch",
    source: "Game index",
    host: "debrid",
    url: "",
    linkType: "torrent",
    supportedBy: ["Real-Debrid", "TorBox"],
    rawgCover: null,
    cover: null,
    rawgRating: 4.6,
    rawgReleased: "2017-04-28",
    linkCount: 6,
    size: 6.8 * GB,
  },
  {
    id: "g2",
    gameSlug: "metroid-dread",
    title: "Metroid Dread",
    platform: "switch",
    source: "Game index",
    host: "debrid",
    url: "",
    linkType: "torrent",
    supportedBy: ["Real-Debrid"],
    rawgCover: null,
    cover: null,
    rawgRating: 4.4,
    rawgReleased: "2021-10-08",
    linkCount: 4,
    size: 4.2 * GB,
  },
];

const iaItems = [
  { id: "switch-homebrew-pack", title: "Switch Homebrew Pack", size_bytes: 2.4 * GB, files_count: 14 },
];

let jobs: any[] = [
  {
    id: 1,
    kind: "homebrew",
    label: "homebrew ftpd",
    state: "done",
    done_bytes: 1.2 * MB,
    total_bytes: 1.2 * MB,
    message: "uploaded to /switch",
    started_at: Date.now() / 1000 - 300,
    finished_at: Date.now() / 1000 - 290,
  },
  {
    id: 2,
    kind: "download",
    label: "Metroid Dread",
    state: "done",
    done_bytes: 4.2 * GB,
    total_bytes: 4.2 * GB,
    message: "saved to Downloads/godsend-switch",
    started_at: Date.now() / 1000 - 900,
    finished_at: Date.now() / 1000 - 700,
  },
];
let nextJob = 3;

function fire(event: string, payload: unknown) {
  for (const entry of listeners.values()) {
    if (entry.event === event) entry.handler(payload);
  }
}

function startJob(kind: string, label: string) {
  const job = {
    id: nextJob++,
    kind,
    label,
    state: "running" as const,
    done_bytes: 0,
    total_bytes: 100 * MB,
    message: "mock job",
    started_at: Date.now() / 1000,
    finished_at: null,
  };
  jobs = [job, ...jobs];
  fire("job://job", job);
  setTimeout(() => {
    const done = { ...job, state: "done", done_bytes: 100 * MB, message: "done", finished_at: Date.now() / 1000 };
    jobs = jobs.map((j) => (j.id === job.id ? done : j));
    fire("job://job", done);
  }, 1200);
  return job.id;
}

function listStorage(args: { storage: string; path: string }) {
  const dir = (args.path || "/").replace(/\/$/, "") || "/";
  return storageTree[`${args.storage}:${dir}`] ?? [];
}

function handle(cmd: string, args: Record<string, any> = {}) {
  switch (cmd) {
    case "snapshot":
      return {
        hosts: config.hosts,
        active: config.active_host,
        config,
        status,
        doctor: checks,
        jobs,
        mcp_port: config.mcp_port,
        mcp_enabled: config.mcp_enabled,
      };
    case "probe":
      return status;
    case "discover":
      return [host];
    case "host_add": {
      config = { ...config, hosts: [...config.hosts, args.host], active_host: args.host.id };
      return config;
    }
    case "host_remove": {
      config = { ...config, hosts: config.hosts.filter((h) => h.id !== args.id) };
      return config;
    }
    case "host_set_active":
      config = { ...config, active_host: args.id };
      return config;
    case "storages":
      return storages;
    case "library":
      return args.source === "local" ? localFiles : installed;
    case "list_storage":
      return listStorage(args as { storage: string; path: string });
    case "storage_mkdir":
    case "storage_delete":
    case "download_open":
    case "download_delete":
    case "fm_mkdir":
    case "fm_delete":
    case "jobs_clear":
    case "open_data_dir":
    case "reveal":
    case "pad_release":
    case "pad_set":
      return null;
    case "upload_local":
      return startJob("upload", "Upload");
    case "download_remote":
      return startJob("download", "Download");
    case "install_path":
      return startJob("install", args.path?.split("/").pop() ?? "Install");
    case "tinfoil_attached":
      return null;
    case "homebrew_catalog":
      return homebrew;
    case "homebrew_install": {
      const row = homebrew.find((h) => h.id === args.id);
      return startJob("homebrew", `homebrew ${row?.name ?? args.id}`);
    }
    case "store_search":
      return hits.filter((h) => h.title.toLowerCase().includes(String(args.query).toLowerCase()));
    case "ia_search":
      return iaItems;
    case "ia_files":
      return [
        { url: "https://archive.org/x/switch-pack.nsp", name: "switch-pack.nsp", size_bytes: 2.4 * GB },
      ];
    case "store_download":
      return startJob("download", "Game index download");
    case "download_url":
      return startJob("download", args.name);
    case "downloads_list":
      return localFiles.map((f) => ({
        name: f.name + (f.format === "nsp" ? ".nsp" : ".nsz"),
        path: f.path,
        is_dir: false,
        size: f.size,
      }));
    case "local_root":
      return "/Users/demo/Downloads/godsend-switch";
    case "fm_local":
      return [
        { name: "godsend-switch", path: "/Users/demo/Downloads/godsend-switch", is_dir: true, size: 0 },
        { name: "Games", path: "/Users/demo/Games", is_dir: true, size: 0 },
        { name: "Metroid Dread.nsp", path: "/Users/demo/Metroid Dread.nsp", is_dir: false, size: 4.2 * GB },
      ];
    case "convert_inspect":
      return {
        path: args.path,
        name: String(args.path ?? "").split("/").pop(),
        size: 6.8 * GB,
        format: "nsp",
        title_id: "0100152000022000",
        parts: 0,
        is_part: false,
      };
    case "convert_split":
      return startJob("split", "Split for FAT32");
    case "convert_merge":
      return startJob("merge", "Merge parts");
    case "config_get":
      return config;
    case "config_set": {
      config = { ...config, ...(args.patch ?? {}) };
      return config;
    }
    case "doctor_run":
      return checks;
    case "jobs":
      return jobs;
    case "job_cancel":
      jobs = jobs.map((j) => (j.id === args.id ? { ...j, state: "cancelled" as const } : j));
      return true;
    case "mcp_info":
      return [config.mcp_enabled, config.mcp_port];
    default:
      throw new Error(`mock: unknown command ${cmd}`);
  }
}

(window as unknown as Record<string, any>).__TAURI_INTERNALS__ = {
  invoke(cmd: string, args?: Record<string, unknown>) {
    if (cmd === "plugin:event|listen") {
      const id = nextCallbackId++;
      listeners.set(id, { event: String((args as any)?.event), handler: (args as any)?.handler });
      return Promise.resolve(id);
    }
    if (cmd === "plugin:event|unlisten") {
      listeners.delete(Number((args as any)?.eventId));
      return Promise.resolve();
    }
    try {
      return Promise.resolve(handle(cmd, (args as Record<string, any>) ?? {}));
    } catch (err) {
      return Promise.reject(String(err));
    }
  },
  transformCallback(cb: Handler) {
    const id = nextCallbackId++;
    (window as any)[`_${id}`] = cb;
    return id;
  },
};

export {};
