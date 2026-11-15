import { useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { Host } from "../types";
import { Btn, Chip, confirmDialog, Empty } from "../ui";

const EMPTY: Host = {
  id: "",
  name: "",
  addr: "",
  ftp_port: 5000,
  ftp_user: "anonymous",
  ftp_pass: "",
};

export default function Devices({ snap, toast, reload }: PageProps) {
  const [draft, setDraft] = useState<Host>(EMPTY);
  const [editing, setEditing] = useState(false);
  const [subnet, setSubnet] = useState("192.168.1");

  async function save() {
    if (!draft.addr.trim()) {
      toast("enter the console's IP address");
      return;
    }
    try {
      await api.hostAdd(draft);
      toast(`saved ${draft.name || draft.addr}`);
      setDraft(EMPTY);
      setEditing(false);
      reload();
    } catch (err) {
      toast(`save failed: ${String(err)}`);
    }
  }

  async function remove(host: Host) {
    if (!confirmDialog(`Remove ${host.name}?`, "The console keeps its own settings.")) return;
    await api.hostRemove(host.id);
    reload();
  }

  async function discover() {
    try {
      toast(`scanning ${subnet}.1-254 on port 5000`);
      const found = await api.discover(subnet, 5000);
      if (found.length === 0) {
        toast("no FTP servers answered on that subnet");
        return;
      }
      for (const host of found) {
        await api.hostAdd(host);
      }
      toast(`found ${found.length} console${found.length === 1 ? "" : "s"}`);
      reload();
    } catch (err) {
      toast(`discovery failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>USB</h2>
          <div className="spacer" />
          <Chip tone={snap.status.mtp_ok ? "good" : "bad"}>
            {snap.status.mtp_ok ? "MTP attached" : "no MTP"}
          </Chip>
          <Chip tone={snap.status.tinfoil_ok ? "good" : "neutral"}>
            {snap.status.tinfoil_ok ? "Tinfoil ready" : "Tinfoil idle"}
          </Chip>
        </div>
        <p className="muted" style={{ padding: "0 14px 12px" }}>
          USB is driven by a responder on the console: DBI or Sphaira in MTP mode gives the
          library, SD/NAND, saves, album and the install folders. Tinfoil-family installers
          expose the bulk install protocol instead.
        </p>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Consoles (FTP)</h2>
          <div className="spacer" />
          <Btn
            variant="primary"
            onClick={() => {
              setDraft(EMPTY);
              setEditing(true);
            }}
          >
            Add console
          </Btn>
        </div>
        <div className="rows">
          {snap.hosts.length === 0 ? (
            <Empty title="No FTP hosts yet" hint="Optional: add one for file transfers over the LAN." />
          ) : (
            snap.hosts.map((host) => (
              <div className={`host-card ${snap.active === host.id ? "current" : ""}`} key={host.id}>
                <div>
                  <div className="name">{host.name}</div>
                  <div className="mono">
                    {host.addr}:{host.ftp_port} - {host.ftp_user}
                  </div>
                </div>
                <div className="host-actions">
                  {snap.active === host.id ? (
                    <Chip tone="good">active</Chip>
                  ) : (
                    <Btn
                      small
                      onClick={async () => {
                        await api.hostSetActive(host.id);
                        reload();
                      }}
                    >
                      Use
                    </Btn>
                  )}
                  <Btn small variant="danger" onClick={() => remove(host)}>
                    Remove
                  </Btn>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="discover">
          <input
            className="input"
            value={subnet}
            onChange={(e) => setSubnet(e.target.value)}
            aria-label="Subnet to scan"
          />
          <Btn onClick={discover}>Scan subnet</Btn>
        </div>
      </div>

      {editing ? (
        <div className="panel">
          <div className="panel-head">
            <h2>{draft.id ? "Edit console" : "New console"}</h2>
          </div>
          <div className="form">
            <label>
              Name
              <input
                className="input"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                placeholder="Living room Switch"
              />
            </label>
            <label>
              Address
              <input
                className="input"
                value={draft.addr}
                onChange={(e) => setDraft({ ...draft, addr: e.target.value })}
                placeholder="192.168.1.50"
              />
            </label>
            <label>
              FTP port
              <input
                className="input"
                type="number"
                value={draft.ftp_port}
                onChange={(e) => setDraft({ ...draft, ftp_port: Number(e.target.value) })}
              />
            </label>
            <label>
              User
              <input
                className="input"
                value={draft.ftp_user}
                onChange={(e) => setDraft({ ...draft, ftp_user: e.target.value })}
              />
            </label>
            <label>
              Password
              <input
                className="input"
                type="password"
                value={draft.ftp_pass}
                onChange={(e) => setDraft({ ...draft, ftp_pass: e.target.value })}
              />
            </label>
          </div>
          <div className="actions">
            <Btn variant="primary" onClick={save}>
              Save
            </Btn>
            <Btn onClick={() => setEditing(false)}>Cancel</Btn>
          </div>
        </div>
      ) : null}
    </div>
  );
}
