# Virtual controller

The pad is sys-con's network controller. The app sends a 44-byte UDP packet
to the console's configured address and port 56789.

## Using it

- Click a button to hold it, click again to release.
- Presets tap HOME, Capture, L or R once.
- Sticks are sliders from -1 to 1; they send on every change.
- Keyboard: arrows for the D-pad, Z/X for B/A, A/S for Y/X, Q/E for L/R,
  Enter for Plus, Shift for Minus.
- Detach sends `connected = 0`, which removes the virtual pad.

## What the packet contains

Magic `SCNP`, version 1, pad index 0, a connection flag, a 32-bit button mask
(bit positions: A=2, B=3, X=1, Y=4, L=15, R=16, ZL=17, ZR=18, MINUS=19,
PLUS=20, D-pad 21-24, Capture=25, HOME=26), four int16 stick axes, and six
floats for motion. At rest the accelerometer reports one g on Y.

## Limits

- The console is the USB device, so the PC cannot send USB input. UDP is the
  only path.
- A game may ignore input while the HOME menu is open, and some titles ignore
  the virtual pad entirely.
- Input is not authenticated. Keep the console on a trusted network.
