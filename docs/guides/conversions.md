# Conversions

## Inspect

Paste a path in Convert and press Inspect, or run
`godsend-switch inspect game.nsp`. The app reports the container (PFS0 for
NSP, zstd for NSZ/XCZ, HEAD at 0x100 for XCI), the size, the title id from
the filename and the split part count.

## Split

`godsend-switch split game.nsp` writes `game.nsp.00`, `.01`, ... at the
configured chunk (default 4095 MiB). The source file stays. Tinfoil, DBI and
the console's own file browser reassemble parts that sit next to each other.

Change the chunk in Settings or with `--chunk-mb`.

## Merge

`godsend-switch merge game.nsp out/game.nsp` joins `game.nsp.00`, `.01`, ...
until the first gap. In the app, paste the base path, set the output and press
Merge parts.

## Not implemented

- NSZ and XCZ decompression (needs the console keys and a decompressor).
- XCI to NSP conversion (needs keys and re-wrapping).
- Signing or creating installable packages.

The app will not guess: it reports these formats and stops.
