//! Pure-Rust, `#![deny(unsafe_code)]`, `wasm32`-compatible LLVM Bitcode (`.bc`) reader
//! and textual LLVM/QIR IR reconstructor.
//!
//! Eliminates runtime dependence on external `llvm-dis` or Python `pyqir` when loading
//! binary QIR bitcode in browsers (WebAssembly), Cloudflare Workers, or minimal native setups.

mod bitstream;
mod ir;
mod render;
mod types_meta;

use std::collections::HashMap;

pub use bitstream::BitcodeError;
use bitstream::{
    decode_blockinfo_into, walk_block, AbbrevOp, BitCursor, BlockEvent, MAX_BLOCKS_DEPTH,
    MAX_CONTAINER_BYTES,
};
use types_meta::{ModuleDecoder, BLOCKINFO_BLOCK_ID, STRTAB_BLOCK_ID};

/// Raw LLVM bitcode stream magic (`"BC\xc0\xde"`).
pub const LLVM_BITCODE_MAGIC: [u8; 4] = [0x42, 0x43, 0xC0, 0xDE];
/// LLVM bitcode wrapper magic (`0x0B17C0DE` little-endian).
pub const LLVM_WRAPPER_MAGIC: [u8; 4] = [0xDE, 0xC0, 0x17, 0x0B];

/// Returns `true` if `bytes` begins with raw LLVM bitcode magic (`BC\xc0\xde`)
/// or an LLVM bitcode wrapper header (`0x0B17C0DE`).
pub fn is_llvm_bitcode(bytes: &[u8]) -> bool {
    (bytes.len() >= 4 && bytes[..4] == LLVM_BITCODE_MAGIC)
        || (bytes.len() >= 20 && bytes[..4] == LLVM_WRAPPER_MAGIC)
}

/// Unwraps raw LLVM bitcode bytes from either a bare bitstream or a Darwin/object wrapper.
fn unwrap_bitcode_payload(bytes: &[u8]) -> Result<&[u8], BitcodeError> {
    if bytes.len() > MAX_CONTAINER_BYTES {
        return Err(BitcodeError::new(format!(
            "Bitcode payload exceeds {} MiB safety limit",
            MAX_CONTAINER_BYTES / (1024 * 1024)
        )));
    }
    if bytes.len() >= 4 && bytes[..4] == LLVM_BITCODE_MAGIC {
        return Ok(&bytes[4..]);
    }
    if bytes.len() >= 20 && bytes[..4] == LLVM_WRAPPER_MAGIC {
        let offset = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let size = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let end = offset
            .checked_add(size)
            .ok_or_else(|| BitcodeError::new("Bitcode wrapper offset/size overflow"))?;
        if end > bytes.len() || size < 4 {
            return Err(BitcodeError::new(
                "Malformed LLVM bitcode wrapper header bounds",
            ));
        }
        let inner = &bytes[offset..end];
        if inner[..4] != LLVM_BITCODE_MAGIC {
            return Err(BitcodeError::new(
                "LLVM bitcode wrapper does not point to BC\\xc0\\xde stream",
            ));
        }
        return Ok(&inner[4..]);
    }
    Err(BitcodeError::new("Missing LLVM bitcode magic bytes"))
}

/// Pass 1: Extract top-level `STRTAB_BLOCK` (which LLVM places after `MODULE_BLOCK`).
fn extract_strtab_pass1(payload: &[u8], depth: usize) -> Result<Vec<u8>, BitcodeError> {
    if depth > MAX_BLOCKS_DEPTH {
        return Err(BitcodeError::new("Bitcode block nesting limit exceeded"));
    }
    let mut cursor = BitCursor::new(payload)?;
    let mut blockinfo: HashMap<u64, Vec<Vec<AbbrevOp>>> = HashMap::new();
    let mut strtab = Vec::new();

    while cursor.bit_pos < cursor.total_bits {
        match cursor.read_bits(2)? {
            0 => {
                cursor.align32();
                break;
            }
            1 => {
                let (bid, new_aw, end_bit) = cursor.enter_subblock()?;
                if bid == BLOCKINFO_BLOCK_ID {
                    decode_blockinfo_into(&mut cursor, new_aw, end_bit, &mut blockinfo)?;
                } else if bid == STRTAB_BLOCK_ID {
                    walk_block(
                        &mut cursor,
                        STRTAB_BLOCK_ID,
                        new_aw,
                        end_bit,
                        &blockinfo,
                        |ev| {
                            if let BlockEvent::Record(rec) = ev {
                                if rec.code == 1 {
                                    if !rec.blob.is_empty() {
                                        strtab.extend_from_slice(&rec.blob);
                                    } else {
                                        strtab.extend(rec.ops.iter().map(|&b| (b & 0xFF) as u8));
                                    }
                                }
                            }
                            Ok(())
                        },
                    )?;
                }
                cursor.bit_pos = end_bit;
            }
            _ => {
                return Err(BitcodeError::new(
                    "Invalid record at top level while scanning STRTAB",
                ));
            }
        }
    }
    Ok(strtab)
}

/// Losslessly converts a 64-bit bitcode index to `usize`, returning `usize::MAX`
/// on 32-bit targets (such as `wasm32`) if the value exceeds `usize::MAX` so
/// crafted inputs cannot silently wrap around into valid table indices.
#[inline]
pub(crate) fn to_idx(raw: u64) -> usize {
    usize::try_from(raw).unwrap_or(usize::MAX)
}

/// Saturating `u64` -> `u32` conversion to prevent silent modulo-$2^{32}$ truncation.
#[inline]
pub(crate) fn to_u32_sat(raw: u64) -> u32 {
    u32::try_from(raw).unwrap_or(u32::MAX)
}

/// Decodes a binary LLVM bitcode buffer (`.bc` or `0x0B17C0DE` wrapper) purely in Rust
/// and reconstructs textual LLVM / QIR IR suitable for [`crate::parser::QirParser`],
/// the browser IR editor, and downstream tooling.
pub fn decode_bitcode_to_ir(bytes: &[u8]) -> Result<String, BitcodeError> {
    let payload = unwrap_bitcode_payload(bytes)?;
    let strtab = extract_strtab_pass1(payload, 0)?;
    let mut decoder = ModuleDecoder::new(strtab);
    let mut cursor = BitCursor::new(payload)?;
    decoder.parse_top_level(&mut cursor)?;
    Ok(decoder.render_ir())
}
