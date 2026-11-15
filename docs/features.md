# Feature reference

## Transports

| Transport | Needs on the console | What it can do |
| --- | --- | --- |
| MTP over USB | DBI "Run MTP responder", or Sphaira | Browse SD/NAND, installed games, saves, album, gamecard; install from SD install / NAND install |
| Tinfoil USB | A Tinfoil/Awoo-family installer in USB mode | Install NSP/NSZ/XCI/XCZ only |
| LAN install | Awoo Installer listening on TCP 2000 | Install NSP/NSZ/XCI/XCZ over the network |
| FTP | ftpd or sys-ftpd | File transfers, NRO pushes, no install handoff |

The app treats the first responder as the console unless a serial is set in
Settings. Discovery scans a /24 for FTP only; MTP is enumerated automatically.

## MTP storages

DBI exposes the storage descriptions the app keys on:

- `SD Card`: the full microSD card, writable.
- `NAND USER`, `NAND SYSTEM`: console storage, NAND USER writable.
- `Installed games`: the titles the console reports, with sizes.
- `SD install`, `NAND install`: dropping a package here installs it.
- `Saves`, `Album`, `Gamecard`: save data, screenshots, the inserted card.

`mtp::kind_of` maps descriptions to kinds. A responder that renames its
storages shows them under Other and installs report the missing unit.

## Library

- Installed titles come from the `Installed games` storage. Title ids are
  parsed from `[0100xxxxxxxxxxxx]` in the name.
- Local packages are the NSP/NSZ/XCI/XCZ/NRO files in the download folder.
- Box art is fetched from `<covers_base>/switch/repo/COV/<TITLEID>.JPG`;
  without art a two-letter monogram is drawn.
- Split sets list once, through their base name.

## Installs

- Package files go to the selected install storage, Tinfoil USB or the LAN
  installer.
- NRO files always go to `/switch` on the SD card, over MTP, with FTP as the
  fallback.
- The job queue reports upload progress and can cancel mid-transfer. An MTP
  upload that fails leaves a partial object; delete it from Files.

## Files

Two panes: the local disk and an MTP storage. Click a folder to open it,
select a local file and press Upload, click a remote file to download it.
New folders and deletes are available for the remote side.

## Virtual controller

sys-con's network controller over UDP. The pad sends a full state on every
change: buttons held, both sticks, connection flag. Buttons stay held until
clicked again, a tap releases them after 90 ms, and Detach sends a packet
with `connected = 0`. Keyboard: arrows, Z/X/A/S for B/A/Y/X, Q/E for L/R,
Enter/Shift for Plus/Minus.

## Game Index and downloads

- Game index search uses `gamesearch_base` and the Switch platform.
- Internet Archive search lists items, their files and direct downloads.
- Link resolution prefers torrents, then DDL links, honoring `supported_by`
  against the configured debrid keys.
- Downloads use aria2c when present (16 connections), otherwise a streamed
  single connection. Every download is a cancellable job.

## Homebrew

Catalog entries point at GitHub repositories and an NRO asset fragment. A
refresh queries each latest release, records the tag, date and size, and
caches the result. Installing downloads the NRO and pushes it to `/switch`.

## Conversions

- `inspect`: format (NSP/NSZ/XCI/XCZ/NRO), size, title id, split parts.
- `split`: writes `.00`, `.01` parts at the configured size (default 4095
  MiB), leaving the source in place.
- `merge`: joins a `.00` set back into one file.
- Not implemented: NSZ/XCZ decompression, XCI to NSP, signing. Those need
  console keys.

## MCP

The desktop app starts a JSON-RPC MCP server on `127.0.0.1:<mcp_port>`
(default 3939) while it runs. `POST /mcp` handles the protocol, `GET /health`
answers a liveness JSON. Tools: `switch_status`, `doctor`, `list_storages`,
`list_storage`, `library`, `install_file`, `pad`, `search_games`,
`homebrew`, `nsp_split`, `nsp_merge`. The CLI's `mcp` command serves the same
tools over stdio.

## Jobs

Every transfer, install, split, merge and download is a job with bytes,
state, message and cancellation. The queue lives in the inspector and is
trimmed to the newest 50 rows in `jobs.json`.

## Preflight and settings

Preflight checks the responder, install storage, Tinfoil mode, FTP host,
sys-con address, download folder writability, aria2c, debrid keys and the MCP
port. Settings covers the install target, MTP serial, sys-con address and
port, split size, download folder, service bases, debrid keys and MCP.
