# Troubleshooting

## No storages on the Console page

- Is a responder running? DBI: launch it, choose "Run MTP responder".
- macOS and Windows mount MTP devices in Finder/Explorer too; close those
  windows, they hold the device.
- Try another cable or the dock's rear USB port.
- Press Refresh. Run `godsend-switch status` for the exact error.

## Installs fail immediately

- Check the target: SD install needs an SD card, NAND install needs free
  console space.
- "no SD install storage" means the responder hides it (DBI's MTP storage
  settings).
- Tinfoil target: the console must be in USB install mode, not MTP.
- Network target: Awoo must be listening; both devices must reach each other
  on TCP 2000 and on the HTTP port the app picked.

## Pad does nothing

- sys-con installed and launched at least once?
- `network_controller=1` in its config, then rebooted?
- Address correct in Settings, same subnet?
- Try `godsend-switch pad a` and watch the console.

## Downloads are slow

- Install aria2c (`brew install aria2`). Without it the app streams on one
  connection.
- Debrid keys unlock links that need them; check Preflight.

## FTP host will not connect

- ftpd defaults: port 5000, user `anonymous`, empty password.
- sys-ftpd runs in the background; ftpd needs focus.
- Discovery only finds FTP; add MTP consoles by cable.

## Where settings live

`~/Library/Application Support/click.godsendswitch.godsend-switch` on macOS,
`%APPDATA%` on Windows, `~/.local/share` on Linux. Settings -> Open data
folder.
