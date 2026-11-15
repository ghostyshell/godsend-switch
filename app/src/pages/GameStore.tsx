import { useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { GameHit, IaFile, IaItem } from "../types";
import { Btn, Chip, Empty, RemoteCover, Seg, fmtBytes } from "../ui";

export default function GameStore({ snap, toast }: PageProps) {
  const [source, setSource] = useState<"index" | "archive">("index");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<GameHit[]>([]);
  const [items, setItems] = useState<IaItem[]>([]);
  const [files, setFiles] = useState<Record<string, IaFile[]>>({});
  const [busy, setBusy] = useState(false);
  const debrid = Boolean(snap.config.rd_api_key || snap.config.tb_api_key);

  async function search() {
    if (!query.trim()) return;
    setBusy(true);
    try {
      if (source === "index") setHits(await api.storeSearch(query.trim()));
      else setItems(await api.iaSearch(query.trim()));
    } catch (err) {
      toast(`search failed: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function openItem(item: IaItem) {
    try {
      const rows = await api.iaFiles(item.id);
      setFiles((cur) => ({ ...cur, [item.id]: rows }));
    } catch (err) {
      toast(`files: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>Game Index</h2>
        <div className="spacer" />
        <Chip tone={debrid ? "good" : "warn"}>{debrid ? "debrid connected" : "no debrid key"}</Chip>
      </div>

      <div className="toolbar">
        <Seg
          value={source}
          ariaLabel="Search source"
          options={[
            { value: "index", label: "Game index" },
            { value: "archive", label: "Internet Archive" },
          ]}
          onChange={(value) => setSource(value as "index" | "archive")}
        />
        <input
          className="input grow"
          placeholder={source === "index" ? "Search Switch games" : "Search archive.org"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void search()}
        />
        <Btn variant="primary" onClick={search} disabled={busy}>
          {busy ? "Searching..." : "Search"}
        </Btn>
      </div>

      {source === "index" ? (
        hits.length === 0 ? (
          <Empty
            title="Search the game index"
            hint={
              snap.config.gamesearch_base
                ? `Backend: ${snap.config.gamesearch_base}`
                : "Set the game index backend in Settings."
            }
          />
        ) : (
          <div className="store-grid">
            {hits.map((hit) => (
              <article className="store-card" key={hit.id || hit.gameSlug}>
                <div className="store-art">
                  <RemoteCover url={hit.cover || hit.rawgCover} title={hit.title} />
                </div>
                <div className="name">{hit.title}</div>
                <div className="tag">{hit.platform || "switch"}</div>
                <div className="meta">
                  {hit.linkCount} links
                  {hit.size ? ` - ${fmtBytes(hit.size)}` : ""}
                  {hit.rawgReleased ? ` - ${hit.rawgReleased.slice(0, 4)}` : ""}
                </div>
                <div className="actions">
                  <Btn
                    small
                    variant="primary"
                    onClick={async () => {
                      try {
                        await api.storeDownload(hit.gameSlug);
                        toast(`downloading ${hit.title}`);
                      } catch (err) {
                        toast(`download failed: ${String(err)}`);
                      }
                    }}
                  >
                    Download
                  </Btn>
                </div>
              </article>
            ))}
          </div>
        )
      ) : items.length === 0 ? (
        <Empty title="Search the Internet Archive" hint="Direct NSP, XCI and homebrew packs." />
      ) : (
        <div className="store-grid">
          {items.map((item) => (
            <article className="store-card" key={item.id}>
              <div className="name">{item.title}</div>
              <div className="meta">
                {item.files_count} files - {fmtBytes(item.size_bytes)}
              </div>
              <div className="actions">
                <Btn small onClick={() => openItem(item)}>
                  Files
                </Btn>
              </div>
              {(files[item.id] ?? []).map((file) => (
                <div className="file-row" key={file.url}>
                  <span className="mono">{file.name}</span>
                  <span className="mono">{fmtBytes(file.size_bytes)}</span>
                  <Btn
                    small
                    variant="primary"
                    onClick={async () => {
                      try {
                        await api.downloadUrl(file.url, file.name);
                        toast(`downloading ${file.name}`);
                      } catch (err) {
                        toast(`download failed: ${String(err)}`);
                      }
                    }}
                  >
                    Get
                  </Btn>
                </div>
              ))}
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
