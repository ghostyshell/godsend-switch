# Changelog

## [Unreleased]

### Changed

- Accessibility pass: search and filter inputs now have accessible names, and the site gains a visible focus ring, focus counterparts for hover styles, and reduced-motion support.
- Downloads are hosted on Cloudflare R2 under versioned, immutable keys, so a
  link stays valid until the next version. The weekly GoFile/file.kiwi re-upload
  is gone: the scheduled job now only checks the live feed for a version bump,
  and each publish prunes R2 versions older than the previous one.

## [0.1.0] - 2026-11-15

First release.

### Added

- USB transports: MTP responders (DBI, Sphaira) for SD/NAND, installed
  games, saves, album and the install folders; Tinfoil/Awoo bulk install
  (TUL0/TUC0); and the LAN installer on TCP 2000 with Range serving.
- FTP transport for consoles without USB (ftpd, sys-ftpd), with LAN
  discovery and multiple hosts.
- Library of installed titles and local NSP/NSZ/XCI/XCZ packages, title ids
  from filenames, cover art with monogram fallback.
- Installs to SD install, NAND install, Tinfoil USB or the network, with NRO
  routing to /switch.
- Two-pane file manager over MTP and the local disk, with upload, download,
  mkdir and delete.
- sys-con virtual controller: face and shoulder buttons, D-pad, both sticks,
  HOME and Capture, keyboard mapping, press-and-hold and detach.
- Game Index search, Internet Archive browsing, Real-Debrid and TorBox
  resolution, aria2c downloads with a single-connection fallback.
- Homebrew catalog of GitHub NRO projects with latest-release asset picking.
- NSP/NSZ/XCI format inspection, title ids, FAT32 split into .00/.01 parts
  and merge.
- MCP server hosted by the app on 127.0.0.1 with eleven tools, plus the same
  tools over stdio in the CLI.
- Preflight checks for the responder, install storage, Tinfoil, FTP,
  sys-con, download folder, aria2c, debrid and the MCP port.

### Safety and robustness

- The LAN installer keeps its HTTP server up until the console closes the
  link, so the ack byte no longer tears the server down mid-install.
- The MCP endpoint accepts only loopback Host/Origin and JSON content type,
  and restarts when its port or enabled flag changes.
- MTP sessions are serialized, cancelled uploads delete their partial object,
  short downloads fail and are cleaned up, and split parts install through
  their base.
- Settings saves are atomic; FTP connects time out; folder uploads skip
  symlinks; remote download names cannot escape the download folder.
- macOS uses an 84px bar padded 22px from the top, so the brand sits below
  the traffic lights and stays left-aligned, the same fix as GODsend PSP.

### Notes

- NSZ/XCZ decompression and XCI to NSP need console keys and are not
  implemented.
- USB input from a PC is not possible; the virtual pad uses sys-con's UDP
  network controller.
