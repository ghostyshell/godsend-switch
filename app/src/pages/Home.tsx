import { useMemo } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import { Btn, Chip, fmtBytes } from "../ui";

export default function Home({ snap, reload, go }: PageProps) {
  const status = snap.status;
  const active = useMemo(
    () => status.storages.find((s) => s.kind === "sd"),
    [status]
  );

  const checks = snap.doctor ?? [];

  return (
    <div className="page">
      <section className="hero">
        <div className="top">
          <div>
            <div className="eyebrow">CONSOLE</div>
            <h1>{status.mtp_ok ? status.mtp_product : status.tinfoil_ok ? "Tinfoil USB mode" : "No console yet"}</h1>
            <div className="sub">
              {status.mtp_ok
                ? `${status.mtp_serial || "no serial"} - ${status.storages.length} storages over MTP`
                : status.tinfoil_ok
                ? "ready for a Tinfoil USB install"
                : status.ftp_ok
                ? "FTP is up; USB is not attached"
                : "Start DBI's MTP responder and connect USB, or add an FTP host."}
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "flex-start" }}>
            <Chip tone={status.mtp_ok ? "good" : "bad"} live={status.mtp_ok}>
              {status.mtp_ok ? "MTP online" : "MTP offline"}
            </Chip>
            <Chip tone={status.tinfoil_ok ? "good" : "neutral"}>
              {status.tinfoil_ok ? "USB install" : "no USB install"}
            </Chip>
          </div>
        </div>

        <div className="stats">
          <div>
            <b>{active ? fmtBytes(active.free_bytes) : "-"}</b>
            <span>SD FREE</span>
          </div>
          <div>
            <b>{active ? fmtBytes(active.total_bytes) : "-"}</b>
            <span>SD SIZE</span>
          </div>
          <div>
            <b>{status.ftp_ok ? `${status.ftp_ms} ms` : "-"}</b>
            <span>FTP</span>
          </div>
          <div>
            <b>{snap.config.syscon_addr || "-"}</b>
            <span>SYSCON</span>
          </div>
        </div>

        <div className="actions">
          <Btn variant="primary" onClick={() => go("library")}>
            Open library
          </Btn>
          <Btn onClick={() => go("controller")}>Virtual pad</Btn>
          <Btn onClick={() => go("files")}>Browse files</Btn>
          <Btn onClick={reload}>Rescan</Btn>
        </div>
      </section>

      <div className="panel">
        <div className="panel-head">
          <h2>Storages</h2>
          <div className="spacer" />
          <Btn small onClick={reload}>
            Refresh
          </Btn>
        </div>
        <div className="storages">
          {status.storages.length === 0 ? (
            <p className="muted" style={{ padding: 14 }}>
              No MTP storages. Start DBI's MTP responder, then press Refresh.
            </p>
          ) : (
            status.storages.map((storage) => (
              <div className="storage-row" key={storage.id}>
                <div>
                  <div className="name">{storage.description}</div>
                  <div className="mono">{storage.kind}</div>
                </div>
                <div className="bar">
                  <div
                    className="fill"
                    style={{
                      width: storage.total_bytes
                        ? `${Math.max(2, Math.round(((storage.total_bytes - storage.free_bytes) / storage.total_bytes) * 100))}%`
                        : "0%",
                    }}
                  />
                </div>
                <div className="mono free">{fmtBytes(storage.free_bytes)} free</div>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Preflight</h2>
          <div className="spacer" />
          <Btn
            small
            onClick={async () => {
              try {
                await api.doctorRun();
              } finally {
                reload();
              }
            }}
          >
            Re-check
          </Btn>
        </div>
        <div className="checks">
          {checks.map((check) => (
            <div className={`check ${check.ok ? "ok" : "off"}`} key={check.name}>
              <div className="head">
                <span>{check.name}</span>
                <Chip tone={check.ok ? "good" : "warn"}>{check.ok ? "ok" : "optional"}</Chip>
              </div>
              <div className="detail">{check.detail}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
