# Connect your Switch

The app never installs drivers and never assumes the console is a mounted
disk. Pick one of three transports.

## MTP over USB (recommended)

1. Put DBI on the SD card and launch it from the homebrew menu. Pick "Run MTP
   responder" (or press X). Sphaira's MTP mode works the same way.
2. Connect the USB cable. On a dock, use the back port.
3. Open GODsend Switch. The Console page should list eight storages: SD Card,
   NAND USER, NAND SYSTEM, Installed games, SD install, NAND install, Saves,
   Album.

If the list is empty, press Refresh. If it stays empty, the responder is not
running or another MTP client holds the device.

## Tinfoil USB

Launch a Tinfoil/Awoo-family installer and choose USB install. The app's
Tinfoil chip turns green. Installs sent with the Tinfoil target stream from
the PC while the console requests byte ranges.

## FTP

Install ftpd or sys-ftpd on the console, note its address, then add a host on
the Devices page (default port 5000, user `anonymous`). FTP is a fallback: it
moves files but cannot install or drive the console.

## sys-con

Install sys-con, add `network_controller=1` to its config, and set the
console's address in Settings. The Controller page then drives the pad over
UDP. Keep both devices on the same network.

## Check the result

Run `godsend-switch status` or open the Console page. Preflight reports the
responder, install storage, Tinfoil, FTP, sys-con, downloads, aria2c, debrid
and the MCP server.
