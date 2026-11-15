import { useEffect, useMemo, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { Entry } from "../types";
import { Btn, Empty, Monogram, RemoteCover, Seg, fmtBytes } from "../ui";

export default function Library({ snap, toast, reload, refreshKey }: PageProps) {
  const [source, setSource] = useState<"installed" | "local">(snap.status.mtp_ok ? "installed" : "local");
  const [entries, setEntries] = useState<Entry[]>([]);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    setBusy(true);
    api
      .library(source)
      .then((rows) => alive && setEntries(rows))
      .catch((err) => toast(`library: ${String(err)}`))
      .finally(() => alive && setBusy(false));
    return () => {
      alive = false;
    };
  }, [source, refreshKey, toast]);

  const shown = useMemo(
    () =>
      entries.filter((entry) =>
        entry.name.toLowerCase().includes(query.trim().toLowerCase())
      ),
    [entries, query]
  );

  async function install(entry: Entry) {
    try {
      await api.installPath(entry.path);
      toast(`installing ${entry.name}`);
    } catch (err) {
      toast(`install failed: ${String(err)}`);
    }
  }

  async function download(entry: Entry) {
    try {
      const dir = await api.localRoot();
      await api.downloadRemote("installed", entry.path, dir);
      toast(`saving ${entry.name}`);
    } catch (err) {
      toast(`download failed: ${String(err)}`);
    }
  }



  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>{source === "installed" ? "Installed library" : "Local packages"}</h2>
        <div className="spacer" />
        <input
          className="input"
          placeholder="Filter by title"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <Btn onClick={reload}>Rescan</Btn>
      </div>

      <div className="toolbar">
        <Seg
          value={source}
          ariaLabel="Library source"
          options={[
            { value: "installed", label: "Installed" },
            { value: "local", label: "Local" },
          ]}
          onChange={(value) => setSource(value as "installed" | "local")}
        />
        <span className="muted">
          {shown.length} {shown.length === 1 ? "title" : "titles"}
        </span>
      </div>

      {shown.length === 0 ? (
        <Empty
          title={busy ? "Loading..." : source === "installed" ? "Nothing installed" : "No local packages"}
          hint={
            source === "installed"
              ? "Start the MTP responder, or switch to Local."
              : "Download a game or drop an NSP in the download folder."
          }
        />
      ) : (
        <div className="tiles">
          {shown.map((entry) => (
            <article className="tile" key={entry.path}>
              <div className="cover">
                <RemoteCover url={entry.cover} title={entry.name} />
              </div>
              <div className="body">
                <div className="t" title={entry.name}>
                  {entry.name}
                </div>
                <div className="s">
                  <span>
                    {entry.format.toUpperCase()} - {fmtBytes(entry.size)}
                  </span>
                  <span>{entry.title_id ?? ""}</span>
                </div>
              </div>
              <div className="actions">
                {source === "installed" ? (
                  <Btn small onClick={() => download(entry)}>
                    Save
                  </Btn>
                ) : (
                  <>
                    <Btn small variant="primary" onClick={() => install(entry)}>
                      Install
                    </Btn>
                    <Btn small onClick={() => api.reveal(entry.path)}>
                      Show
                    </Btn>
                  </>
                )}
              </div>
            </article>
          ))}
        </div>
      )}

      <div className="panel">
        <div className="panel-head">
          <h2>Title art</h2>
        </div>
        <p className="muted" style={{ padding: "0 14px 14px" }}>
          Switch box art is served by <span className="mono">{snap.config.covers_base}</span> under{" "}
          <span className="mono">/switch/repo/COV/[TITLEID].JPG</span>. Titles without art show a
          monogram.
        </p>
      </div>
    </div>
  );
}

export function TileFallback({ name }: { name: string }) {
  return <Monogram title={name} />;
}
