# GODsend Switch: project overview for agents

GODsend Switch is a desktop workshop for a homebrew-enabled Nintendo Switch.
It speaks MTP and the Tinfoil USB protocol over USB, the Awoo LAN installer,
and FTP, then wraps them in the GODsend family shell (Rust workspace plus
Tauri 2 plus React). The design system is inherited from GODsend PSP.

## Components

- **`core/` (`godsend-core`)**: framework-agnostic engine. No Tauri, no CLI
  parsing.
  - `mtp`: `mtp-rs` session wrapper. Opens the first responder (or a
    configured serial), lists storages, classifies them (`Kind`), resolves
    folder paths, uploads with progress and cancellation, downloads, mkdirs,
    deletes.
  - `tinfoil`: Tinfoil/Awoo USB bulk protocol (`TUL0` list, `TUC0`
    command/response headers, FILE_RANGE and FILE_RANGE_PADDED), served
    synchronously on a worker thread via `nusb`.
  - `netinstall`: Awoo/Tinfoil LAN install. TCP 2000 receives a
    length-prefixed URL list; a local HTTP server serves those files with
    Range support; the console acks with one 0x00 byte.
  - `ftp`: suppaftp wrapper for consoles without USB (ftpd, sys-ftpd).
    Tolerant listing (MLSD, Unix LIST, DOS LIST), upload, download.
  - `syscon`: sys-con network controller. 44-byte little-endian UDP packet,
    magic `0x504E4353`, button bit positions, sticks scaled to i16, repeated
    three times per state change.
  - `nsp`: PFS0 sniffing, XCI `HEAD` magic, zstd (NSZ/XCZ) detection, title
    ids from `[0100xxxxxxxxxxxx]` names, `.00` FAT32 split and merge.
  - `library`: installed titles from the MTP "Installed games" storage, NROs
    under `/switch`, local packages, cover URLs.
  - `fm`: local and MTP browsing, upload/download, FTP fallbacks.
  - `install`: transport selection (SD install, NAND install, Tinfoil,
    network), NRO routing to `/switch`.
  - `homebrew`: GitHub NRO catalog, latest release asset picking, install to
    `/switch` (MTP first, FTP fallback).
  - `sources`, `store`, `debrid`, `aria2`: game index and Internet Archive
    search, link selection, Real-Debrid and TorBox resolution, downloads.
  - `mcp`: JSON-RPC handler, tool specs, stdio loop and the app-hosted HTTP
    server (POST /mcp, GET /health).
  - `transfer`: JobManager with progress hook, cancellation and a journal.
- **`cli/` (`godsend-switch`)**: one command per feature area. Run with no
  arguments for the list.
- **`app/`**: Tauri 2 shell. `app/src-tauri/src/lib.rs` has the command layer
  and starts the MCP server; `app/src` is React 18 plus TypeScript with one
  hand-rolled `styles.css` (see DESIGN.md) and a mock bridge in `mock.ts`
  behind `?mock=1`.

## Invariants

- `core/` must stay UI-free. It returns `Result<T>`, and the Tauri error
  serializes as the Display string.
- DBI storage descriptions are the contract: "SD Card", "NAND USER",
  "NAND SYSTEM", "Installed games", "SD install", "NAND install", "Saves",
  "Album", "Gamecard". `mtp::kind_of` classifies them; check "installed"
  before "install".
- Tinfoil USB is the console pulling ranges. The host sends `TUL0` first,
  then serves `TUC0` FILE_RANGE (1 MiB chunks) and FILE_RANGE_PADDED
  (0xFF000 data plus 0x1000 zeros). `CMD_ID_EXIT` ends the loop.
- The LAN installer's URLs are `http://<host>:<port>/<index>/<name>`; the
  server must answer 206 with `Content-Range` and close the connection.
- sys-con packets are exactly 44 bytes. Bit positions, not masks: A is
  `1 << 2`, HOME is `1 << 26`. Sticks are `round(clamp(-1..1) * 32767)`.
- NSP split parts are `<name>.00`, `<name>.01`; the source file stays.
- MCP is stateless HTTP JSON-RPC. Notifications get no body (202). Tool
  failures are `isError: true` content, never a transport error.
- Destructive operations (delete a file or title, power actions) require an
  explicit confirmation in the UI.
- USB input from a PC is impossible: the console is the USB device. The
  virtual pad only exists through sys-con's UDP network controller.

## Building and testing

```sh
cargo test --workspace
cargo check --workspace
npm --prefix app install
npm --prefix app run build          # tsc --noEmit plus vite build
npm --prefix app run dev:mock       # browser UI, no hardware
npm --prefix app run test:ui        # Playwright against the mock bridge
```

`core/tests/fake_switch.rs` runs a fake FTP server and drives list, upload,
download, FAT32 splitting and NSP inspection. `core/tests/mcp_http.rs` starts
the real MCP HTTP server and checks initialize, tools/list and /health.
Extend these, not just unit tests, when a flow changes.

## Releases and copy

- Version lives in `Cargo.toml` (workspace), `app/package.json`,
  `app/src-tauri/tauri.conf.json`, `app/src/App.tsx` and the Settings about
  line. Use `scripts/bump-version.sh x.y.z "note"`; keep the newest
  CHANGELOG header at the top, no Unreleased section.
- Public-facing copy: no em dashes, no en dashes, ASCII hyphen only.
- Icons: the supplied gold emblem is `app/src-tauri/icons/icon.png`.
  Regenerate with `npx --prefix app tauri icon app/src-tauri/icons/icon.png`
  and copy the same file to `app/src/assets/logo.png` and `docs/icon.png`.

## Demo videos and guides

GODsend apps carry per-feature demo reels and a guides page, recorded from
the renderer's `?mock=1` bridge by `~/Code/demo-reels`. A new, changed or
removed feature updates its reel and its guides entry in the same change.
The Switch reels are not recorded yet; until they are, text guides live in
`docs/guides/`.
