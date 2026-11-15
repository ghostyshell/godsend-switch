//! sys-con network controller.
//!
//! The console's sys-con sysmodule can expose a virtual pad driven over UDP
//! (config: `[global] network_controller=1`, default port 56789). Packets are
//! one-way: the pad appears on the first packet and detaches when a packet
//! arrives with `connected = 0`. There is no handshake.

use crate::error::{Error, Result};
use std::net::UdpSocket;

pub const DEFAULT_PORT: u16 = 56789;
pub const MAGIC: u32 = 0x504E_4353; // "SCNP"
pub const VERSION: u8 = 1;
pub const PACKET_LEN: usize = 44;
/// UDP is lossy; state changes are sent a few times.
pub const REPEAT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    X,
    A,
    B,
    Y,
    LStick,
    RStick,
    L,
    R,
    Zl,
    Zr,
    Minus,
    Plus,
    DpadUp,
    DpadRight,
    DpadDown,
    DpadLeft,
    Capture,
    Home,
}

impl Button {
    /// Bit position used by sys-con, not the mask itself.
    pub fn bit(self) -> u32 {
        match self {
            Button::X => 1,
            Button::A => 2,
            Button::B => 3,
            Button::Y => 4,
            Button::LStick => 5,
            Button::RStick => 10,
            Button::L => 15,
            Button::R => 16,
            Button::Zl => 17,
            Button::Zr => 18,
            Button::Minus => 19,
            Button::Plus => 20,
            Button::DpadUp => 21,
            Button::DpadRight => 22,
            Button::DpadDown => 23,
            Button::DpadLeft => 24,
            Button::Capture => 25,
            Button::Home => 26,
        }
    }

    pub fn mask(self) -> u32 {
        1 << self.bit()
    }

    pub fn name(self) -> &'static str {
        match self {
            Button::X => "x",
            Button::A => "a",
            Button::B => "b",
            Button::Y => "y",
            Button::LStick => "lstick",
            Button::RStick => "rstick",
            Button::L => "l",
            Button::R => "r",
            Button::Zl => "zl",
            Button::Zr => "zr",
            Button::Minus => "minus",
            Button::Plus => "plus",
            Button::DpadUp => "up",
            Button::DpadRight => "right",
            Button::DpadDown => "down",
            Button::DpadLeft => "left",
            Button::Capture => "capture",
            Button::Home => "home",
        }
    }

    pub fn parse(name: &str) -> Result<Button> {
        Ok(match name.trim().to_ascii_lowercase().as_str() {
            "x" => Button::X,
            "a" => Button::A,
            "b" => Button::B,
            "y" => Button::Y,
            "lstick" | "ls" => Button::LStick,
            "rstick" | "rs" => Button::RStick,
            "l" => Button::L,
            "r" => Button::R,
            "zl" => Button::Zl,
            "zr" => Button::Zr,
            "minus" | "-" => Button::Minus,
            "plus" | "+" => Button::Plus,
            "up" => Button::DpadUp,
            "right" => Button::DpadRight,
            "down" => Button::DpadDown,
            "left" => Button::DpadLeft,
            "capture" => Button::Capture,
            "home" => Button::Home,
            other => return Err(Error::Invalid(format!("unknown button '{other}'"))),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadState {
    pub buttons: u32,
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
    pub connected: bool,
}

impl Default for PadState {
    fn default() -> Self {
        Self { buttons: 0, lx: 0, ly: 0, rx: 0, ry: 0, connected: true }
    }
}

impl PadState {
    pub fn with(buttons: &[Button]) -> Self {
        let mut state = PadState::default();
        for button in buttons {
            state.buttons |= button.mask();
        }
        state
    }

    /// Set the left or right stick from normalized -1.0..1.0 values.
    pub fn with_stick(mut self, side: &str, x: f32, y: f32) -> Result<Self> {
        let (x, y) = (axis(x), axis(y));
        match side.to_ascii_lowercase().as_str() {
            "l" => {
                self.lx = x;
                self.ly = y;
            }
            "r" => {
                self.rx = x;
                self.ry = y;
            }
            other => return Err(Error::Invalid(format!("unknown stick '{other}'"))),
        }
        Ok(self)
    }

    /// Wire layout from sys-con's `tools/networkpad.py`: 44 bytes, LE.
    pub fn to_packet(&self) -> [u8; PACKET_LEN] {
        let mut out = [0u8; PACKET_LEN];
        out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        out[4] = VERSION;
        out[5] = 0; // pad index
        out[6] = u8::from(self.connected);
        out[7] = 0; // reserved
        out[8..12].copy_from_slice(&self.buttons.to_le_bytes());
        out[12..14].copy_from_slice(&self.lx.to_le_bytes());
        out[14..16].copy_from_slice(&self.ly.to_le_bytes());
        out[16..18].copy_from_slice(&self.rx.to_le_bytes());
        out[18..20].copy_from_slice(&self.ry.to_le_bytes());
        // Rest pose: SDL convention, +Y up, one g of gravity.
        for (index, value) in [0.0f32, 9.80665, 0.0, 0.0, 0.0, 0.0].iter().enumerate() {
            let at = 20 + index * 4;
            out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }
}

fn axis(value: f32) -> i16 {
    let scaled = (value.clamp(-1.0, 1.0) * 32767.0).round();
    scaled.clamp(-32768.0, 32767.0) as i16
}

fn socket() -> Result<UdpSocket> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    Ok(sock)
}

/// Send one state, repeated `REPEAT` times.
pub fn send(addr: &str, port: u16, state: &PadState) -> Result<()> {
    if addr.trim().is_empty() {
        return Err(Error::device("sys-con address is not set in Settings"));
    }
    let target = format!("{}:{}", addr.trim(), port);
    let packet = state.to_packet();
    let sock = socket()?;
    for _ in 0..REPEAT {
        sock.send_to(&packet, &target)
            .map_err(|e| Error::Other(format!("sys-con {target}: {e}")))?;
    }
    Ok(())
}

/// Detach the virtual pad.
pub fn disconnect(addr: &str, port: u16) -> Result<()> {
    send(addr, port, &PadState { connected: false, ..Default::default() })
}

/// Press a button, hold it, then release every button.
pub fn tap(addr: &str, port: u16, button: Button, hold_ms: u64) -> Result<()> {
    send(addr, port, &PadState::with(&[button]))?;
    std::thread::sleep(std::time::Duration::from_millis(hold_ms.clamp(30, 5000)));
    send(addr, port, &PadState::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_is_44_bytes_with_magic_and_version() {
        let bytes = PadState::default().to_packet();
        assert_eq!(bytes.len(), 44);
        assert_eq!(&bytes[0..4], &MAGIC.to_le_bytes());
        assert_eq!(bytes[4], VERSION);
        assert_eq!(bytes[6], 1);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0);
        let accel_y = f32::from_le_bytes(bytes[24..28].try_into().unwrap());
        assert!((accel_y - 9.80665).abs() < 1e-4);
    }

    #[test]
    fn buttons_use_bit_positions() {
        let state = PadState::with(&[Button::A, Button::Plus]);
        assert_eq!(state.buttons, (1 << 2) | (1 << 20));
        assert_ne!(state.buttons & Button::A.mask(), 0);
        assert_eq!(Button::Home.mask(), 1 << 26);
    }

    #[test]
    fn sticks_clamp_and_scale() {
        assert_eq!(axis(0.0), 0);
        assert_eq!(axis(1.0), 32767);
        assert_eq!(axis(-1.0), -32767);
        assert_eq!(axis(9.0), 32767);
        let state = PadState::default().with_stick("L", 0.5, -1.0).unwrap();
        assert_eq!(state.lx, 16384);
        assert_eq!(state.ly, -32767);
        assert!(state.with_stick("z", 0.0, 0.0).is_err());
    }

    #[test]
    fn button_names_round_trip() {
        for name in ["x", "a", "b", "y", "l", "r", "zl", "zr", "minus", "plus", "up", "down", "left", "right", "home", "capture"] {
            let button = Button::parse(name).unwrap();
            assert_eq!(Button::parse(button.name()).unwrap(), button);
        }
        assert!(Button::parse("turbo").is_err());
    }

    #[test]
    fn empty_address_is_reported() {
        assert!(send("", DEFAULT_PORT, &PadState::default()).is_err());
    }
}
