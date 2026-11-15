# Installing games

## Where files land

The install target in Settings decides the route:

- **SD install** (MTP): the package is written to the responder's "SD install"
  storage and DBI installs it as soon as the transfer completes.
- **NAND install** (MTP): the same, into console storage.
- **Tinfoil USB**: the console pulls ranges over USB.
- **Network install**: the app serves the file over HTTP and hands the URLs
  to Awoo on TCP 2000.

NRO homebrew is different: it always goes to `/switch` on the SD card.

## From the app

Open Downloads, pick a package, press Send to Switch. The job queue shows the
upload. Or use Library -> Local -> Install. Convert -> Send to Switch does the
same for a path you paste.

## From the CLI

```sh
godsend-switch install ~/Downloads/game.nsp            # configured target
godsend-switch install ~/Downloads/game.nsp --via tinfoil
godsend-switch tinfoil game.nsp                        # USB bulk protocol
godsend-switch netinstall game.nsp                     # LAN, TCP 2000
```

## Split packages

FAT32 drives cannot hold a file above 4 GiB. Split first:

```sh
godsend-switch split game.nsp
```

That writes `game.nsp.00`, `.01`, ... and leaves the source. Copy the parts
next to each other; Tinfoil and DBI reassemble them. Use Convert -> Merge
parts to join them again.

## What cannot be done here

NSZ/XCZ decompression and XCI to NSP need console keys. The app identifies
those formats but does not transform them.
