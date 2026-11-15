import { useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { NspInfo } from "../types";
import { Btn, Chip, Empty, fmtBytes } from "../ui";

export default function Convert({ snap, toast }: PageProps) {
  const [path, setPath] = useState("");
  const [info, setInfo] = useState<NspInfo | null>(null);
  const [out, setOut] = useState("");

  async function inspect() {
    if (!path.trim()) return;
    try {
      setInfo(await api.convertInspect(path.trim()));
    } catch (err) {
      setInfo(null);
      toast(`inspect failed: ${String(err)}`);
    }
  }

  async function split() {
    try {
      await api.convertSplit(path.trim());
      toast("splitting for FAT32");
    } catch (err) {
      toast(`split failed: ${String(err)}`);
    }
  }

  async function merge() {
    try {
      const base = path.trim().replace(/\.\d+$/, "");
      await api.convertMerge(base, out.trim() || base);
      toast("merging parts");
    } catch (err) {
      toast(`merge failed: ${String(err)}`);
    }
  }

  async function install() {
    try {
      await api.installPath(path.trim());
      toast(`sending to ${snap.config.install_target}`);
    } catch (err) {
      toast(`install failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Convert and install</h2>
          <div className="spacer" />
          <Chip tone="laser">local files</Chip>
        </div>
        <div className="form">
          <label className="wide">
            File or base path
            <input
              className="input"
              placeholder="/Users/you/Downloads/game.nsp"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void inspect()}
            />
          </label>
          <div className="actions">
            <Btn variant="primary" onClick={inspect}>
              Inspect
            </Btn>
            <Btn onClick={split} disabled={!path}>
              Split for FAT32
            </Btn>
            <Btn onClick={install} disabled={!path}>
              Send to Switch
            </Btn>
          </div>
          <label className="wide">
            Merge output
            <input
              className="input"
              placeholder="/Users/you/Downloads/game.nsp"
              value={out}
              onChange={(e) => setOut(e.target.value)}
            />
          </label>
          <div className="actions">
            <Btn onClick={merge} disabled={!path}>
              Merge parts
            </Btn>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Inspection</h2>
        </div>
        {info ? (
          <div className="rows">
            <div className="kv">
              <span>name</span>
              <span>{info.name}</span>
            </div>
            <div className="kv">
              <span>format</span>
              <span>{info.format}</span>
            </div>
            <div className="kv">
              <span>size</span>
              <span>{fmtBytes(info.size)}</span>
            </div>
            <div className="kv">
              <span>title id</span>
              <span>{info.title_id ?? "-"}</span>
            </div>
            <div className="kv">
              <span>split parts</span>
              <span>{info.parts || (info.is_part ? "part" : 0)}</span>
            </div>
          </div>
        ) : (
          <Empty title="Nothing inspected" hint="Paste a path and press Inspect." />
        )}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>What each action does</h2>
        </div>
        <div className="notes">
          <Note
            name="Split for FAT32"
            detail={`Writes <name>.00, .01 ... at ${Math.round(snap.config.split_bytes / 1024 / 1024)} MiB, the layout Tinfoil and DBI reassemble.`}
          />
          <Note
            name="Merge parts"
            detail="Joins a .00/.01 set back into one NSP or XCI."
          />
          <Note
            name="Send to Switch"
            detail="Uses the configured transport: SD install, NAND install, Tinfoil USB or the LAN installer."
          />
          <Note
            name="Not supported"
            detail="NSZ/XCZ decompression and XCI to NSP need console keys; this app identifies those files but does not transform them."
          />
        </div>
      </div>
    </div>
  );
}

function Note({ name, detail }: { name: string; detail: string }) {
  return (
    <div className="note">
      <div className="name">{name}</div>
      <div className="detail">{detail}</div>
    </div>
  );
}
