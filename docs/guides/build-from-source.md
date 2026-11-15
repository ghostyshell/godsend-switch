# Build from source

## Requirements

- Rust 1.85 or newer.
- Node 20 or newer.
- Tauri 2 prerequisites: Xcode Command Line Tools on macOS, MSVC plus
  WebView2 on Windows, webkit2gtk-4.1 and its dev headers on Linux.

## Desktop app

```sh
npm install --prefix app
npx --prefix app tauri icon app/src-tauri/icons/icon.png   # once per icon change
npm run --prefix app build                                  # typecheck plus vite
npx --prefix app tauri build                                # bundles
```

The app bundle lands under `target/release/bundle`. On macOS it is ad-hoc
signed on CI; local builds need no signing.

## CLI

```sh
cargo build --release -p godsend-switch-cli
./target/release/godsend-switch --help
```

## Tests

```sh
cargo test --workspace        # core unit, fake FTP, MCP HTTP
npm run --prefix app test:ui  # Playwright against the mock bridge
```

The UI tests start Vite on port 1420 and use `?mock=1`, so no console is
needed. The fake FTP test binds a local server and exercises list, upload,
download and FAT32 splitting.

## Mock mode

```sh
npm run --prefix app dev:mock
```

Fixture data covers a Switch on MTP with eight storages, four installed
titles, two local packages, a homebrew catalog, store results and a queue.
