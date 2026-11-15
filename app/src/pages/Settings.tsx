import { useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import { Btn } from "../ui";

export default function Settings({ snap, toast, reload }: PageProps) {
  const [config, setConfig] = useState(snap.config);
  const [saving, setSaving] = useState(false);

  function set<K extends keyof typeof config>(key: K, value: (typeof config)[K]) {
    setConfig({ ...config, [key]: value });
  }

  async function save() {
    setSaving(true);
    try {
      const patch: Record<string, unknown> = {
        install_target: config.install_target,
        mtp_serial: config.mtp_serial,
        syscon_addr: config.syscon_addr,
        syscon_port: config.syscon_port,
        download_dir: config.download_dir,
        rd_api_key: config.rd_api_key,
        tb_api_key: config.tb_api_key,
        gamesearch_base: config.gamesearch_base,
        covers_base: config.covers_base,
        aria2_path: config.aria2_path,
        split_bytes: Math.round(config.split_bytes / 1024 / 1024) * 1024 * 1024,
        mcp_port: config.mcp_port,
        mcp_enabled: config.mcp_enabled,
      };
      await api.configSet(patch);
      toast("settings saved");
      reload();
    } catch (err) {
      toast(`save failed: ${String(err)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Transports</h2>
        </div>
        <div className="form">
          <label>
            Install target
            <select
              className="input"
              value={config.install_target}
              onChange={(e) => set("install_target", e.target.value as typeof config.install_target)}
            >
              <option value="sd">SD install (MTP)</option>
              <option value="nand">NAND install (MTP)</option>
              <option value="tinfoil">Tinfoil USB</option>
              <option value="network">Network install (TCP 2000)</option>
            </select>
          </label>
          <label>
            MTP serial (blank = first responder)
            <input
              className="input"
              value={config.mtp_serial}
              onChange={(e) => set("mtp_serial", e.target.value)}
              placeholder="XAW10000000000"
            />
          </label>
          <label>
            sys-con address
            <input
              className="input"
              value={config.syscon_addr}
              onChange={(e) => set("syscon_addr", e.target.value)}
              placeholder="192.168.1.50"
            />
          </label>
          <label>
            sys-con port
            <input
              className="input"
              type="number"
              value={config.syscon_port}
              onChange={(e) => set("syscon_port", Number(e.target.value))}
            />
          </label>
          <label>
            FAT32 split size (MiB)
            <input
              className="input"
              type="number"
              value={Math.round(config.split_bytes / 1024 / 1024)}
              onChange={(e) => set("split_bytes", Number(e.target.value) * 1024 * 1024)}
            />
          </label>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Downloads and index</h2>
        </div>
        <div className="form">
          <label>
            Download folder
            <input
              className="input"
              value={config.download_dir}
              onChange={(e) => set("download_dir", e.target.value)}
              placeholder="Downloads/godsend-switch"
            />
          </label>
          <label>
            aria2c path (blank = auto)
            <input
              className="input"
              value={config.aria2_path}
              onChange={(e) => set("aria2_path", e.target.value)}
            />
          </label>
          <label>
            Game index backend
            <input
              className="input"
              value={config.gamesearch_base}
              onChange={(e) => set("gamesearch_base", e.target.value)}
            />
          </label>
          <label>
            Box art base
            <input
              className="input"
              value={config.covers_base}
              onChange={(e) => set("covers_base", e.target.value)}
            />
          </label>
          <label>
            Real-Debrid key
            <input
              className="input"
              type="password"
              value={config.rd_api_key}
              onChange={(e) => set("rd_api_key", e.target.value)}
            />
          </label>
          <label>
            TorBox key
            <input
              className="input"
              type="password"
              value={config.tb_api_key}
              onChange={(e) => set("tb_api_key", e.target.value)}
            />
          </label>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>MCP server</h2>
        </div>
        <div className="form">
          <label className="wide">
            <span>
              Starts with the app on <span className="mono">127.0.0.1</span>; a port
              change or toggle applies on Save. Add it to an agent with
              <span className="mono">claude mcp add --transport http godsend-switch
              http://127.0.0.1:{config.mcp_port}/mcp</span>.
            </span>
          </label>
          <label>
            Port
            <input
              className="input"
              type="number"
              value={config.mcp_port}
              onChange={(e) => set("mcp_port", Number(e.target.value))}
            />
          </label>
          <label className="inline">
            <input
              type="checkbox"
              checked={config.mcp_enabled}
              onChange={(e) => set("mcp_enabled", e.target.checked)}
            />
            Start the MCP server with the app
          </label>
        </div>
      </div>

      <div className="actions">
        <Btn variant="primary" onClick={save} disabled={saving}>
          {saving ? "Saving..." : "Save settings"}
        </Btn>
        <Btn onClick={() => api.openDataDir()}>Open data folder</Btn>
      </div>
    </div>
  );
}
