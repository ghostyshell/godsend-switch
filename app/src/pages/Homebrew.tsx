import { useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { CatalogEntry } from "../types";
import { Btn, Chip, Empty, fmtBytes } from "../ui";

export default function Homebrew({ toast, refreshKey }: PageProps) {
  const [rows, setRows] = useState<CatalogEntry[]>([]);
  const [busy, setBusy] = useState(false);

  async function load(refresh: boolean) {
    setBusy(true);
    try {
      setRows(await api.homebrewCatalog(refresh));
    } catch (err) {
      toast(`catalog: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void load(false);
  }, [refreshKey]);

  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>Homebrew</h2>
        <div className="spacer" />
        <Btn small onClick={() => load(true)} disabled={busy}>
          {busy ? "Checking GitHub..." : "Refresh"}
        </Btn>
      </div>

      {rows.length === 0 ? (
        <Empty title={busy ? "Loading catalog..." : "No catalog"} />
      ) : (
        <div className="store-grid">
          {rows.map((row) => (
            <article className="store-card" key={row.id}>
              <div className="name">{row.name}</div>
              <div className="tag">{row.category}</div>
              <p>{row.description}</p>
              <div className="meta mono">{row.repo}</div>
              <div className="meta">
                {row.latest_tag ?? "no release"} {row.size_bytes ? `- ${fmtBytes(row.size_bytes)}` : ""}
              </div>
              <div className="actions">
                {row.nro_url ? (
                  <Btn
                    small
                    variant="primary"
                    onClick={async () => {
                      try {
                        await api.homebrewInstall(row.id);
                        toast(`installing ${row.name} to /switch`);
                      } catch (err) {
                        toast(`install failed: ${String(err)}`);
                      }
                    }}
                  >
                    Install
                  </Btn>
                ) : (
                  <Chip tone="warn">unavailable</Chip>
                )}
              </div>
            </article>
          ))}
        </div>
      )}

      <div className="panel">
        <div className="panel-head">
          <h2>Where it goes</h2>
        </div>
        <p className="muted" style={{ padding: "0 14px 14px" }}>
          NROs are pushed to <span className="mono">/switch</span> on the SD card (MTP first, FTP
          as the fallback), so they appear in the homebrew menu. sys-con is the one the virtual
          pad needs; DBI and Awoo are the two installers the USB paths talk to.
        </p>
      </div>
    </div>
  );
}
