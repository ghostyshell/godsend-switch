# AI guidelines for godsend-psp

Read `AGENTS.md` first for the architecture and invariants, and `DESIGN.md`
before touching any UI code.

## Stack

Rust workspace (`core` engine, `cli`, `app/src-tauri` Tauri 2 shell) plus a
React 18 + TypeScript + Vite frontend with one hand-rolled stylesheet.
No CSS framework; design tokens live on `:root` in `app/src/styles.css`
and raw hex in components is banned.

## Rules

1. Keep `core/` framework-agnostic: no tauri, no clap, no UI types. The CLI
   and the desktop app are thin layers over the engine.
2. Frontend types in `app/src/types.ts` must stay in sync with the serde
   shapes in core; the mock bridge (`app/src/mock.ts`) must mirror real
   fixture shapes so `?mock=1` stays honest.
3. Long operations are jobs in `godsend_core::transfer::JobManager` with
   progress + cancellation; never block the UI thread with I/O.
4. Destructive stick operations require explicit confirmation in the UI and
   must path-escape-guard anything joined from user input or archive names.
5. Copy follows the no-em-dashes rule (`no-em-dashes.mdc`). Every product
   change bumps the version and writes a `CHANGELOG.md` entry in the same
   commit: `scripts/bump-version.sh x.y.z "Fixed: ..."`. No `[Unreleased]`
   section; newest dated header wins. `scripts/install-hooks.sh` enables the
   pre-commit check (`scripts/check-changelog.sh`). See
   `docs/agents/skills/changelog.md`.
6. Design: follow DESIGN.md tokens and motion rules; respect the
   reduced-motion kill switch; keep state chips dot+word so nothing is
   color-only.
7. Ship complete: every change bumps the version, refreshes the site
   screenshots (`node scripts/capture-screens.mjs` with the mock dev server),
   and runs `scripts/publish-release.sh` (build, GoFile + file.kiwi upload,
   README/site repoint, live-feed check) before it counts as done.
8. Releases: after `scripts/publish-release.sh`, verify the live site feed
   (`downloads.json?v=<timestamp>`) reports the new version. Local files or
   a pushed commit are not proof; GitHub Pages caches for ten minutes and
   the site must fetch the feed cache-busted.
8. Tests: `cargo test --workspace` covers the binary format parsers and the
   job manager; `npx playwright test` covers the UI via the mock bridge.
   Both must pass before any commit that touches product code.

## PSP domain notes

- A PSP in USB mode is plain mass storage: detection is filesystem based
  (root `PSP/` directory). No libusb, no drivers.
- Game folders live in PSP/GAME; disc images in the ISO folder at stick
  root; saves in PSP/SAVEDATA (minus ARK_01234); music PSP/MUSIC; video
  PSP/VIDEO; themes PSP/THEME; plugin lists in seplugins/*.txt.
- ARK-4 install layout: ARK_01234 to PSP/SAVEDATA, loaders to PSP/GAME.
- PSP video target: AVC Main 3.0, 480x272, yuv420p, AAC 128k, MP4 with
  faststart. Music: MP3 192k or AAC in M4A.
