import { useCallback, useEffect, useMemo, useState } from "react";
import { api, listen } from "./api";
import type { Job, Snapshot } from "./types";
import { PAGE_TITLES } from "./types";
import { Chip, fmtBytes } from "./ui";
import {
  House,
  MonitorSmartphone,
  Library as LibraryIcon,
  Gamepad2,
  Store as StoreIcon,
  Download as DownloadIcon,
  FolderOpen,
  ArrowLeftRight,
  Package as PackageIcon,
  Settings as SettingsIcon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import logo from "./assets/logo.png";
import Home from "./pages/Home";
import Devices from "./pages/Devices";
import Library from "./pages/Library";
import Controller from "./pages/Controller";
import GameStore from "./pages/GameStore";
import Downloads from "./pages/Downloads";
import Files from "./pages/Files";
import Convert from "./pages/Convert";
import Homebrew from "./pages/Homebrew";
import Settings from "./pages/Settings";

type Page =
  | "home"
  | "devices"
  | "library"
  | "controller"
  | "store"
  | "downloads"
  | "files"
  | "convert"
  | "homebrew"
  | "settings";

const NAV: { id: Page; label: string; icon: LucideIcon }[] = [
  { id: "home", label: "Console", icon: House },
  { id: "devices", label: "Devices", icon: MonitorSmartphone },
  { id: "library", label: "Library", icon: LibraryIcon },
  { id: "controller", label: "Controller", icon: Gamepad2 },
  { id: "store", label: "Game Index", icon: StoreIcon },
  { id: "downloads", label: "Downloads", icon: DownloadIcon },
  { id: "files", label: "Files", icon: FolderOpen },
  { id: "convert", label: "Convert", icon: ArrowLeftRight },
  { id: "homebrew", label: "Homebrew", icon: PackageIcon },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

const VERSION = "0.1.1";

export interface PageProps {
  snap: Snapshot;
  reload: () => void;
  toast: (message: string) => void;
  refreshKey: number;
  go: (page: string) => void;
}

const PAGES: Record<Page, (props: PageProps) => JSX.Element> = {
  home: Home,
  devices: Devices,
  library: Library,
  controller: Controller,
  store: GameStore,
  downloads: Downloads,
  files: Files,
  convert: Convert,
  homebrew: Homebrew,
  settings: Settings,
};

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  const toast = useCallback((message: string) => {
    setToastMsg(message);
    window.setTimeout(() => setToastMsg((cur) => (cur === message ? null : cur)), 4600);
  }, []);

  const loadSnapshot = useCallback(async () => {
    try {
      setSnap(await api.snapshot());
    } catch (err) {
      console.error("snapshot failed", err);
    }
  }, []);

  // Refresh both the pages and the snapshot, so a saved setting (sys-con
  // address, install target, MCP port) takes effect without a restart.
  const reload = useCallback(() => {
    setRefreshKey((k) => k + 1);
    void loadSnapshot();
  }, [loadSnapshot]);

  useEffect(() => {
    void loadSnapshot();
    const unlisteners: Promise<() => void>[] = [
      listen<{ id: number; done: number; total: number }>("job://progress", (update) => {
        setSnap((cur) =>
          cur
            ? {
                ...cur,
                jobs: cur.jobs.map((job) =>
                  job.id === update.id
                    ? { ...job, done_bytes: update.done, total_bytes: update.total }
                    : job
                ),
              }
            : cur
        );
      }),
      listen<Job>("job://job", (job) => {
        setSnap((cur) => {
          if (!cur) return cur;
          const exists = cur.jobs.some((j) => j.id === job.id);
          return {
            ...cur,
            jobs: exists
              ? cur.jobs.map((j) => (j.id === job.id ? job : j))
              : [job, ...cur.jobs],
          };
        });
        if (job.state === "done") toast(`${job.label}: ${job.message || "done"}`);
        else if (job.state === "failed") toast(`${job.label} failed: ${job.message}`);
        setRefreshKey((k) => k + 1);
      }),
    ];
    return () => {
      void Promise.all(unlisteners).then((uns) => uns.forEach((fn) => fn()));
    };
  }, [loadSnapshot, toast]);

  const activeHost = useMemo(
    () => snap?.hosts.find((h) => h.id === snap.active) ?? null,
    [snap]
  );
  const status = snap?.status;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.altKey || e.metaKey || e.ctrlKey) return;
      const index = Number(e.key) - 1;
      if (index >= 0 && index < NAV.length) {
        e.preventDefault();
        setPage(NAV[index].id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!snap) {
    return (
      <div className="boot">
        <img className="boot-mark" src={logo} alt="" />
        <p>Connecting to GODsend Switch...</p>
      </div>
    );
  }

  const PageView = PAGES[page];

  return (
    <div className="shell">
      <div className="backdrop" aria-hidden="true">
        <span className="orb one" />
        <span className="orb two" />
        <span className="orb three" />
      </div>
      <a className="skip-link" href="#workspace">
        Skip to content
      </a>

      <header className="topbar">
        <div className="brand">
          <img className="mark" src={logo} alt="" />
          <span className="name">GODsend Switch</span>
        </div>
        <span className="topbar-title" role="status">
          {PAGE_TITLES[page]}
        </span>
        <div className="spacer" />
        <Chip tone={status?.mtp_ok ? "good" : "bad"} live={status?.mtp_ok}>
          {status?.mtp_ok ? "MTP" : "no MTP"}
        </Chip>
        <Chip tone={status?.tinfoil_ok ? "good" : "neutral"}>
          {status?.tinfoil_ok ? "Tinfoil USB" : "USB idle"}
        </Chip>
        <Chip tone={status?.ftp_ok ? "good" : "neutral"}>{status?.ftp_ok ? "FTP" : "no FTP"}</Chip>
      </header>

      <nav className="rail" aria-label="Sections">
        {NAV.map((item, index) => {
          const Icon = item.icon;
          const current = item.id === page;
          return (
            <button
              key={item.id}
              className={`rail-item ${current ? "current" : ""}`}
              aria-current={current ? "page" : undefined}
              onClick={() => setPage(item.id)}
              title={`${item.label} (Alt+${index + 1})`}
            >
              <Icon size={17} strokeWidth={1.8} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <main id="workspace" className="workspace with-inspector" key={`${page}-${refreshKey}`}>
        <PageView snap={snap} reload={reload} toast={toast} refreshKey={refreshKey} go={(p) => setPage(p as Page)} />
      </main>

      <aside className="inspector" aria-label="Device and queue">
        <div className="panel device-card">
          <div className="panel-head">
            <h2>Device</h2>
          </div>
          <div className="rows">
            <Row label="transport" value={status?.mtp_ok ? "MTP over USB" : status?.tinfoil_ok ? "Tinfoil USB" : status?.ftp_ok ? "FTP" : "offline"} />
            <Row label="console" value={status?.mtp_product || activeHost?.name || "-"} />
            <Row label="serial" value={status?.mtp_serial || "-"} />
            <Row label="sys-con" value={status?.syscon_ok ? `${snap.config.syscon_addr}:${snap.config.syscon_port}` : "not set"} />
            <Row
              label="sd free"
              value={fmtBytes(status?.storages.find((s) => s.kind === "sd")?.free_bytes ?? 0)}
            />
          </div>
        </div>
        <div className="panel queue-card">
          <div className="panel-head">
            <h2>Queue</h2>
            <div className="spacer" />
            {snap.jobs.length > 0 ? (
              <button
                className="btn small"
                onClick={async () => {
                  await api.jobsClear();
                  void loadSnapshot();
                }}
              >
                Clear
              </button>
            ) : null}
          </div>
          <div className="rows">
            {snap.jobs.length === 0 ? (
              <p style={{ margin: 0, fontSize: 12.5, color: "var(--text-faint)" }}>
                No jobs yet. Installs, uploads and conversions show up here.
              </p>
            ) : (
              snap.jobs.slice(0, 8).map((job) => (
                <JobRow
                  key={job.id}
                  job={job}
                  onCancel={async () => {
                    await api.jobCancel(job.id);
                    void loadSnapshot();
                  }}
                />
              ))
            )}
          </div>
        </div>
      </aside>

      <footer className="statusbar">
        <span>
          <span className={`status-dot ${status?.mtp_ok ? "ok" : status?.ftp_ok ? "idle" : "off"}`} />
          {status?.mtp_ok ? "USB connected" : status?.ftp_ok ? "FTP only" : "no console"}
        </span>
        <span>
          <span className={`status-dot ${status?.syscon_ok ? "ok" : "idle"}`} />
          sys-con {status?.syscon_ok ? "ready" : "off"}
        </span>
        <span className="grow" />
        <span>
          MCP {snap.mcp_enabled ? `:${snap.mcp_port}` : "off"}
        </span>
        <span>godsend-switch v{VERSION}</span>
      </footer>

      {toastMsg ? (
        <div className="toast" role="status" aria-live="polite">
          <span className="chip laser">
            <span className="dot" />
            Switch
          </span>
          {toastMsg}
        </div>
      ) : null}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="kv">
      <span>{label}</span>
      <span>{value}</span>
    </div>
  );
}

function JobRow({ job, onCancel }: { job: Job; onCancel: () => void }) {
  const running = job.state === "running";
  return (
    <div className="job-row">
      <div className="head">
        <span className="label" title={job.label}>
          {job.label}
        </span>
        {running ? (
          <button className="btn small" onClick={onCancel} title="Cancel job">
            Cancel
          </button>
        ) : (
          <Chip tone={job.state === "done" ? "good" : job.state === "failed" ? "bad" : "warn"}>
            {job.state}
          </Chip>
        )}
      </div>
      {running ? (
        <div style={{ marginTop: 6 }}>
          <div className="bar live">
            <div
              className="fill"
              style={{
                width: `${job.total_bytes ? Math.round((job.done_bytes / job.total_bytes) * 100) : 12}%`,
              }}
            />
          </div>
        </div>
      ) : (
        <div className="msg">{job.message}</div>
      )}
    </div>
  );
}
