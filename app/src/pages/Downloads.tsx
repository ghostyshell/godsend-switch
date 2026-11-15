import { useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { LocalEntry } from "../types";
import { Btn, Empty, confirmDialog, fmtBytes } from "../ui";

export default function Downloads({ snap, toast, reload, refreshKey }: PageProps) {
  const [files, setFiles] = useState<LocalEntry[]>([]);

  useEffect(() => {
    let alive = true;
    api
      .downloadsList()
      .then((rows) => alive && setFiles(rows.filter((row) => !row.is_dir)))
      .catch((err) => toast(`downloads: ${String(err)}`));
    return () => {
      alive = false;
    };
  }, [refreshKey, toast]);

  async function send(file: LocalEntry) {
    try {
      await api.downloadPush(file.path);
      toast(`sending ${file.name}`);
    } catch (err) {
      toast(`send failed: ${String(err)}`);
    }
  }

  async function remove(file: LocalEntry) {
    if (!confirmDialog(`Delete ${file.name}?`, "The file is removed from the download folder.")) return;
    try {
      await api.downloadDelete(file.path);
      reload();
    } catch (err) {
      toast(`delete failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>Downloads</h2>
        <div className="spacer" />
        <Btn
          small
          onClick={async () => {
            const root = await api.localRoot();
            if (root) await api.reveal(root);
          }}
        >
          Open folder
        </Btn>
        <Btn small onClick={reload}>
          Refresh
        </Btn>
      </div>

      <div className="panel">
        {files.length === 0 ? (
          <Empty
            title="No local packages"
            hint={`Downloads land in ${snap.config.download_dir || "Downloads/godsend-switch"}.`}
          />
        ) : (
          <div className="rows">
            {files.map((file) => (
              <div className="file-row" key={file.path}>
                <span className="name" title={file.name}>
                  {file.name}
                </span>
                <span className="mono">{fmtBytes(file.size)}</span>
                <div className="inline-actions">
                  <Btn small variant="primary" onClick={() => send(file)}>
                    Send to Switch
                  </Btn>
                  <Btn small onClick={() => api.downloadOpen(file.path)}>
                    Show
                  </Btn>
                  <Btn small variant="danger" onClick={() => remove(file)}>
                    Delete
                  </Btn>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Install target</h2>
        </div>
        <p className="muted" style={{ padding: "0 14px 14px" }}>
          Sending a package uses{" "}
          <span className="mono">
            {snap.config.install_target === "sd"
              ? "SD install (MTP)"
              : snap.config.install_target === "nand"
              ? "NAND install (MTP)"
              : snap.config.install_target === "tinfoil"
              ? "Tinfoil USB"
              : "network install"}
          </span>
          . NRO files always go to <span className="mono">/switch</span> on the SD card.
        </p>
      </div>
    </div>
  );
}
