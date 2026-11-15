# Homebrew and sys-con

## The catalog

Homebrew is a bundled list of GitHub projects, each with the NRO asset name
to pick from its latest release:

| Entry | Why you want it |
| --- | --- |
| Homebrew App Store | Install more homebrew on the console |
| ftpd / sys-ftpd | FTP transport, foreground or background |
| sys-con | Network controller for the virtual pad |
| NX-Shell | File manager on the console |
| DBI | MTP responder and installer (the main USB path) |
| Awoo Installer | USB and LAN installs |
| NXThemes Installer | Custom themes |

Press Refresh to query GitHub. "unavailable" means the release has no NRO
asset matching the entry.

## Installing

Install downloads the NRO and pushes it to `/switch` on the SD card, over MTP
when a responder is attached and over FTP otherwise. The homebrew menu picks
it up on the next launch.

CLI: `godsend-switch homebrew --refresh` and
`godsend-switch homebrew-install sys-con`.

## Making the pad work

sys-con needs two things:

1. The NRO installed in `/switch` and launched once so it writes its config.
2. `network_controller=1` under `[global]` in `config/sys-con/config.ini` on
   the SD card. The default port is 56789.

Reboot the console after enabling it. The app's Controller page then sends
UDP packets to that port.
