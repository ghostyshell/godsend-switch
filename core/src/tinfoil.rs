//! Tinfoil/Awoo USB install protocol (magic `TUC0` / `TUL0`).
//!
//! The console runs the client: it requests byte ranges of the files the host
//! announced, and the host streams them over two bulk endpoints. This is the
//! protocol NS-USBloader speaks. It installs only; browsing happens over MTP.

use crate::error::{Error, Result};
use crate::transfer::JobContext;
use nusb::transfer::{Bulk, Buffer, Completion, In, Out, TransferError};
use nusb::MaybeFuture;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VENDOR_ID: u16 = 0x057E;
pub const PRODUCT_ID: u16 = 0x3000;
pub const CMD_EXIT: u32 = 0;
pub const CMD_FILE_RANGE: u32 = 1;
pub const CMD_FILE_RANGE_PADDED: u32 = 2;
pub const TYPE_RESPONSE: u8 = 1;
pub const CHUNK: usize = 0x100000;
pub const PADDING: usize = 0x1000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsbInfo {
    pub product: String,
    pub serial: String,
}

/// Is a console sitting in Tinfoil's USB install mode right now?
pub fn attached() -> Option<UsbInfo> {
    let devices = nusb::list_devices().wait().ok()?;
    devices
        .filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        .map(|d| UsbInfo {
            product: d.product_string().unwrap_or("Nintendo Switch").to_string(),
            serial: d.serial_number().unwrap_or("").to_string(),
        })
        .next()
}

/// 16-byte `TUL0` title-list header.
pub fn list_header(len: u32) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(b"TUL0");
    out[4..8].copy_from_slice(&len.to_le_bytes());
    out
}

/// 32-byte `TUC0` command/response header.
pub fn cmd_header(cmd_type: u8, cmd_id: u32, data_size: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0..4].copy_from_slice(b"TUC0");
    out[4] = cmd_type;
    out[8..12].copy_from_slice(&cmd_id.to_le_bytes());
    out[12..20].copy_from_slice(&data_size.to_le_bytes());
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeRequest {
    pub size: u64,
    pub offset: u64,
    pub name_len: u64,
}

pub fn parse_range_request(header: &[u8]) -> RangeRequest {
    let size = u64::from_le_bytes(header[0..8].try_into().unwrap_or([0; 8]));
    let offset = u64::from_le_bytes(header[8..16].try_into().unwrap_or([0; 8]));
    let name_len = u64::from_le_bytes(header[16..24].try_into().unwrap_or([0; 8]));
    RangeRequest { size, offset, name_len }
}

struct Endpoints {
    out: nusb::Endpoint<Bulk, Out>,
    inp: nusb::Endpoint<Bulk, In>,
}

impl Endpoints {
    fn write(&mut self, bytes: Vec<u8>) -> Result<()> {
        let done = self.out.transfer_blocking(Buffer::from(bytes), Duration::from_secs(30));
        done.status.map_err(|e| Error::Other(format!("USB write: {e}")))
    }

    fn read_exact(&mut self, len: usize, ctx: &JobContext) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            ctx.check_cancel()?;
            let want = len - out.len();
            let done = self.inp.transfer_blocking(Buffer::new(want), Duration::from_secs(1));
            match done.status {
                Ok(()) => {
                    let bytes = done.buffer.into_vec();
                    let got = bytes.len().min(want);
                    out.extend_from_slice(&bytes[..got]);
                    if got == 0 {
                        std::thread::sleep(Duration::from_millis(40));
                    }
                }
                Err(TransferError::Cancelled) => continue,
                Err(TransferError::Disconnected) => {
                    return Err(Error::Other("console disconnected".into()))
                }
                Err(e) => return Err(Error::Other(format!("USB read: {e}"))),
            }
        }
        Ok(out)
    }
}

/// Serve the console's range requests until it sends the exit command.
pub fn install(files: &[PathBuf], ctx: &JobContext) -> Result<()> {
    if files.is_empty() {
        return Err(Error::Invalid("no files selected".into()));
    }
    let info = nusb::list_devices()
        .wait()
        .map_err(|e| Error::Other(format!("USB enumeration: {e}")))?
        .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        .ok_or_else(|| Error::device("no console in Tinfoil USB install mode; open the installer and pick USB"))?;
    let device = info.open().wait().map_err(|e| Error::Other(format!("USB open: {e}")))?;
    let interface = device
        .claim_interface(0)
        .wait()
        .map_err(|e| Error::Other(format!("USB claim: {e}")))?;
    let (out_addr, in_addr) = endpoints(&interface)?;
    let mut endpoints = Endpoints {
        out: interface
            .endpoint::<Bulk, Out>(out_addr)
            .map_err(|e| Error::Other(format!("USB out endpoint: {e}")))?,
        inp: interface
            .endpoint::<Bulk, In>(in_addr)
            .map_err(|e| Error::Other(format!("USB in endpoint: {e}")))?,
    };

    let mut names: HashMap<String, PathBuf> = HashMap::new();
    let mut list = Vec::new();
    for path in files {
        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let name = absolute.to_string_lossy().to_string();
        list.extend_from_slice(name.as_bytes());
        list.push(b'\n');
        names.insert(name, absolute);
    }
    let total: u64 = names
        .values()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    ctx.message("sending the title list");
    endpoints.write(list_header(list.len() as u32).to_vec())?;
    endpoints.write(list)?;

    let mut served = 0u64;
    ctx.progress(0, total);
    loop {
        ctx.check_cancel()?;
        let header = match endpoints.read_exact(0x20, ctx) {
            Ok(header) => header,
            Err(Error::Cancelled(_)) => return Err(Error::Cancelled(ctx.job_id())),
            Err(e) => return Err(e),
        };
        if &header[0..4] != b"TUC0" {
            continue;
        }
        let cmd_id = u32::from_le_bytes(header[8..12].try_into().unwrap_or([0; 4]));
        if cmd_id == CMD_EXIT {
            ctx.progress(total, total);
            return Ok(());
        }
        if cmd_id != CMD_FILE_RANGE && cmd_id != CMD_FILE_RANGE_PADDED {
            continue;
        }
        let request = parse_range_request(&endpoints.read_exact(0x20, ctx)?);
        let name = String::from_utf8(endpoints.read_exact(request.name_len as usize, ctx)?)
            .map_err(|_| Error::Invalid("title name is not UTF-8".into()))?;
        let path = names
            .get(&name)
            .ok_or_else(|| Error::NotFound(format!("console asked for unknown file {name}")))?
            .clone();
        ctx.message(format!(
            "serving {} at {}+{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            request.offset,
            request.size
        ));
        endpoints.write(cmd_header(TYPE_RESPONSE, cmd_id, request.size).to_vec())?;
        served += serve_range(
            &mut endpoints,
            &path,
            request,
            cmd_id == CMD_FILE_RANGE_PADDED,
            ctx,
        )?;
        ctx.progress(served.min(total), total);
    }
}

fn serve_range(
    endpoints: &mut Endpoints,
    path: &Path,
    request: RangeRequest,
    padded: bool,
    ctx: &JobContext,
) -> Result<u64> {
    let file_len = std::fs::metadata(path)?.len();
    if request.offset >= file_len {
        return Err(Error::Invalid(format!(
            "console asked for offset {} past the {} byte file",
            request.offset, file_len
        )));
    }
    let size = request.size.min(file_len - request.offset);
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(request.offset))?;
    let mut left = size;
    let mut sent = 0u64;
    let payload = if padded { CHUNK - PADDING } else { CHUNK };
    let zeros = vec![0u8; PADDING];
    let mut buf = vec![0u8; payload];
    while left > 0 {
        ctx.check_cancel()?;
        let want = (left as usize).min(payload);
        file.read_exact(&mut buf[..want])?;
        if padded {
            endpoints.write(zeros.clone())?;
        }
        endpoints.write(buf[..want].to_vec())?;
        left -= want as u64;
        sent += want as u64;
    }
    Ok(sent)
}

/// Bulk OUT/IN endpoint addresses from the claimed interface descriptor.
fn endpoints(interface: &nusb::Interface) -> Result<(u8, u8)> {
    use nusb::transfer::Direction;
    let descriptor = interface
        .descriptor()
        .ok_or_else(|| Error::Other("no interface descriptor".into()))?;
    let mut out = None;
    let mut inp = None;
    for endpoint in descriptor.endpoints() {
        let address = endpoint.address();
        let is_in = address & 0x80 != 0;
        let _ = Direction::In; // keep the import meaningful across platforms
        if is_in {
            inp = Some(address);
        } else {
            out = Some(address);
        }
    }
    match (out, inp) {
        (Some(out), Some(inp)) => Ok((out, inp)),
        _ => Err(Error::Other("console has no bulk endpoint pair".into())),
    }
}

#[allow(dead_code)]
fn ignore_completion(_: Completion) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_list_header_layout() {
        let header = list_header(0x1234);
        assert_eq!(&header[0..4], b"TUL0");
        assert_eq!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 0x1234);
        assert_eq!(&header[8..16], &[0u8; 8]);
    }

    #[test]
    fn command_header_layout() {
        let header = cmd_header(TYPE_RESPONSE, CMD_FILE_RANGE, 0x100000);
        assert_eq!(&header[0..4], b"TUC0");
        assert_eq!(header[4], 1);
        assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), CMD_FILE_RANGE);
        assert_eq!(u64::from_le_bytes(header[12..20].try_into().unwrap()), 0x100000);
        assert_eq!(&header[20..32], &[0u8; 12]);
    }

    #[test]
    fn range_request_layout() {
        let mut header = [0u8; 32];
        header[0..8].copy_from_slice(&0x1000u64.to_le_bytes());
        header[8..16].copy_from_slice(&0x2000u64.to_le_bytes());
        header[16..24].copy_from_slice(&7u64.to_le_bytes());
        let request = parse_range_request(&header);
        assert_eq!(request.size, 0x1000);
        assert_eq!(request.offset, 0x2000);
        assert_eq!(request.name_len, 7);
    }

    #[test]
    fn padded_chunks_keep_the_payload_under_a_megabyte() {
        assert_eq!(CHUNK - PADDING, 0xFF000);
        assert_eq!(PADDING, 0x1000);
    }
}
