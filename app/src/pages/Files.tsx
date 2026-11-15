import { useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { LocalEntry, RemoteEntry, StorageOut } from "../types";
import { Btn, Empty, fmtBytes } from "../ui";

export default function Files({ snap, toast, reload, refreshKey }: PageProps) {
  const [localDir, setLocalDir] = useState("");
  const [local, setLocal] = useState<LocalEntry[]>([]);
  const [storage, setStorage] = useState("sd");
  const [remoteDir, setRemoteDir] = useState("/");
  const [remote, setRemote] = useState<RemoteEntry[]>([]);
  const [selectedLocal, setSelectedLocal] = useState<LocalEntry | null>(null);

  useEffect(() => {
    api.localRoot().then((root) => {
      if (root) setLocalDir((cur) => cur || root);
    });
  }, []);

  useEffect(() => {
    if (!localDir) return;
    let alive = true;
    api
      .fmLocal(localDir)
      .then((rows) => alive && setLocal(rows))
      .catch((err) => toast(`local: ${String(err)}`));
    return () => {
      alive = false;
    };
  }, [localDir, toast, refreshKey]);

  useEffect(() => {
    if (!snap.status.mtp_ok) {
      setRemote([]);
      return;
    }
    let alive = true;
    api
      .listStorage(storage, remoteDir)
      .then((rows) => alive && setRemote(rows))
      .catch((err) => toast(`storage: ${String(err)}`));
    return () => {
      alive = false;
    };
  }, [storage, remoteDir, snap.status.mtp_ok, toast, refreshKey]);

  async function upload(file: LocalEntry) {
    try {
      await api.uploadLocal(file.path, storage, remoteDir);
      toast(`uploading ${file.name} to ${remoteDir}`);
    } catch (err) {
      toast(`upload failed: ${String(err)}`);
    }
  }

  async function pull(entry: RemoteEntry) {
    const exists = local.some((row) => !row.is_dir && row.name === entry.name);
    if (
      exists &&
      !confirm(
        `${entry.name} already exists in ${localDir}.\n\nOverwrite it?`
      )
    ) {
      return;
    }
    try {
      await api.downloadRemote(storage, entry.path, localDir);
      toast(`downloading ${entry.name}`);
    } catch (err) {
      toast(`download failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>Files</h2>
        <div className="spacer" />
        <Btn small onClick={reload}>
          Refresh
        </Btn>
      </div>

      <div className="file-columns">
        <div className="panel pane">
          <div className="panel-head">
            <h2>This computer</h2>
          </div>
          <div className="pathbar">
            <Btn small onClick={() => setLocalDir(localDir.split("/").slice(0, -1).join("/") || "/")}>
              Up
            </Btn>
            <span className="mono grow">{localDir}</span>
          </div>
          <div className="rows list">
            {local.map((row) => (
              <button
                key={row.path}
                className={`file-row selectable ${row.is_dir ? "dir" : ""} ${
                  selectedLocal?.path === row.path ? "selected" : ""
                }`}
                onClick={() => (row.is_dir ? setLocalDir(row.path) : setSelectedLocal(row))}
              >
                <span className="name">{row.name}</span>
                <span className="mono">{row.is_dir ? "folder" : fmtBytes(row.size)}</span>
              </button>
            ))}
            {local.length === 0 ? <Empty title="Empty folder" /> : null}
          </div>
          <div className="pane-actions">
            <Btn
              small
              variant="primary"
              disabled={!selectedLocal}
              onClick={() => selectedLocal && upload(selectedLocal)}
            >
              Upload to {remoteDir}
            </Btn>
          </div>
        </div>

        <div className="panel pane">
          <div className="panel-head">
            <h2>Console</h2>
          </div>
          <div className="pathbar">
            <select
              className="input"
              value={storage}
              onChange={(e) => {
                setStorage(e.target.value);
                setRemoteDir("/");
              }}
              aria-label="Storage unit"
            >
              {(snap.status.storages.length ? snap.status.storages : []).map((unit: StorageOut) => (
                <option key={unit.id} value={unit.kind}>
                  {unit.description}
                </option>
              ))}
              {snap.status.storages.length === 0 ? <option value="sd">SD card</option> : null}
            </select>
            <Btn small onClick={() => setRemoteDir(remoteDir.split("/").slice(0, -1).join("/") || "/")}>
              Up
            </Btn>
          </div>
          <div className="pathbar">
            <span className="mono grow">{remoteDir}</span>
          </div>
          <div className="rows list">
            {remote.map((row) => (
              <button
                key={row.path}
                className={`file-row selectable ${row.is_dir ? "dir" : ""}`}
                onClick={() => (row.is_dir ? setRemoteDir(row.path) : void pull(row))}
              >
                <span className="name">{row.name}</span>
                <span className="mono">{row.is_dir ? "folder" : fmtBytes(row.size)}</span>
              </button>
            ))}
            {remote.length === 0 ? (
              <Empty
                title={snap.status.mtp_ok ? "Empty folder" : "No MTP responder"}
                hint={snap.status.mtp_ok ? undefined : "Start DBI's MTP responder over USB."}
              />
            ) : null}
          </div>
          <div className="pane-actions">
            <span className="muted">Click a file to download it here.</span>
          </div>
        </div>
      </div>
    </div>
  );
}
