//! LSB-first LLVM Bitstream reader, abbreviation engine, and shared block walker.

use std::collections::HashMap;
use std::fmt;

pub(crate) const MAX_CONTAINER_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_ABBREVS_PER_BLOCK: usize = 4096;
pub(crate) const MAX_RECORD_OPS: usize = 65536;
pub(crate) const MAX_BLOCKS_DEPTH: usize = 32;

/// Errors encountered while decoding an LLVM bitcode container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitcodeError {
    pub message: String,
}

impl BitcodeError {
    pub(crate) fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl fmt::Display for BitcodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LLVM bitcode error: {}", self.message)
    }
}

impl std::error::Error for BitcodeError {}

#[derive(Clone, Debug)]
pub(crate) enum AbbrevOp {
    Literal(u64),
    Fixed(u32),
    Vbr(u32),
    Array(Box<AbbrevOp>),
    Char6,
    Blob,
}

#[derive(Debug)]
pub(crate) struct Record {
    pub(crate) code: u64,
    pub(crate) ops: Vec<u64>,
    pub(crate) blob: Vec<u8>,
}

pub(crate) struct BitCursor<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) bit_pos: usize,
    pub(crate) total_bits: usize,
}

impl<'a> BitCursor<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Result<Self, BitcodeError> {
        let total_bits = data
            .len()
            .checked_mul(8)
            .ok_or_else(|| BitcodeError::new("Bitstream length overflow"))?;
        Ok(Self {
            data,
            bit_pos: 0,
            total_bits,
        })
    }

    #[inline]
    pub(crate) fn align32(&mut self) {
        let rem = self.bit_pos & 31;
        if rem != 0 {
            self.bit_pos = self.bit_pos.saturating_add(32 - rem).min(self.total_bits);
        }
    }

    pub(crate) fn read_bits(&mut self, n: u32) -> Result<u64, BitcodeError> {
        if n > 64 {
            return Err(BitcodeError::new("Fixed field width > 64 bits"));
        }
        let n_usize = n as usize;
        let end_bit = self
            .bit_pos
            .checked_add(n_usize)
            .ok_or_else(|| BitcodeError::new("Bitstream offset overflow"))?;
        if end_bit > self.total_bits {
            return Err(BitcodeError::new("Unexpected EOF in LLVM bitstream"));
        }
        if n == 0 {
            return Ok(0);
        }
        let mut out = 0u64;
        let mut remaining = n_usize;
        let mut out_shift = 0usize;
        while remaining > 0 {
            let byte_idx = self.bit_pos >> 3;
            let bit_off = self.bit_pos & 7;
            let take = remaining.min(8 - bit_off);
            let mask = ((1u16 << take) - 1) as u8;
            let chunk = ((self.data[byte_idx] >> bit_off) & mask) as u64;
            out |= chunk << out_shift;
            self.bit_pos += take;
            remaining -= take;
            out_shift += take;
        }
        Ok(out)
    }

    pub(crate) fn read_vbr(&mut self, width: u32) -> Result<u64, BitcodeError> {
        if !(2..=64).contains(&width) {
            return Err(BitcodeError::new(format!("Invalid VBR width {}", width)));
        }
        let payload_bits = width - 1;
        let payload_mask = (1u64 << payload_bits) - 1;
        let cont_bit = 1u64 << payload_bits;
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let chunk = self.read_bits(width)?;
            let low = chunk & payload_mask;
            if shift >= 64 {
                if low != 0 {
                    return Err(BitcodeError::new("VBR integer overflow > 64 bits"));
                }
            } else {
                let remaining = 64 - shift;
                if remaining < 64 && (low >> remaining) != 0 {
                    return Err(BitcodeError::new("VBR integer overflow > 64 bits"));
                }
                value |= low << shift;
            }
            if (chunk & cont_bit) == 0 {
                return Ok(value);
            }
            shift = shift.saturating_add(payload_bits);
            if shift > 70 {
                return Err(BitcodeError::new("Unterminated VBR value"));
            }
        }
    }

    fn read_char6(&mut self) -> Result<u8, BitcodeError> {
        const TABLE: &[u8; 64] =
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._";
        let idx = self.read_bits(6)? as usize;
        Ok(TABLE[idx])
    }

    fn read_abbrev_scalar(
        &mut self,
        op: &AbbrevOp,
        ops: &mut Vec<u64>,
        blob: &mut Vec<u8>,
    ) -> Result<(), BitcodeError> {
        match op {
            AbbrevOp::Literal(v) => ops.push(*v),
            AbbrevOp::Fixed(w) => ops.push(self.read_bits(*w)?),
            AbbrevOp::Vbr(w) => ops.push(self.read_vbr(*w)?),
            AbbrevOp::Char6 => ops.push(self.read_char6()? as u64),
            AbbrevOp::Array(elem) => {
                if matches!(elem.as_ref(), AbbrevOp::Literal(_) | AbbrevOp::Fixed(0)) {
                    return Err(BitcodeError::new(
                        "Zero-bit abbreviation array element is invalid",
                    ));
                }
                let raw_count = self.read_vbr(6)?;
                let count = usize::try_from(raw_count)
                    .map_err(|_| BitcodeError::new("Abbrev array length overflows usize"))?;
                if count > MAX_RECORD_OPS {
                    return Err(BitcodeError::new("Abbrev array length exceeds limit"));
                }
                for _ in 0..count {
                    self.read_abbrev_scalar(elem, ops, blob)?;
                }
            }
            AbbrevOp::Blob => {
                let raw_len = self.read_vbr(6)?;
                let len = usize::try_from(raw_len)
                    .map_err(|_| BitcodeError::new("Blob size overflow"))?;
                self.align32();
                let byte_start = self.bit_pos >> 3;
                let byte_end = byte_start
                    .checked_add(len)
                    .ok_or_else(|| BitcodeError::new("Blob size overflow"))?;
                if byte_end > self.data.len() {
                    return Err(BitcodeError::new("Blob extends past end of bitcode"));
                }
                *blob = self.data[byte_start..byte_end].to_vec();
                self.bit_pos = byte_end * 8;
                self.align32();
            }
        }
        Ok(())
    }

    fn parse_abbrev_op(&mut self, allow_aggregate: bool) -> Result<AbbrevOp, BitcodeError> {
        if self.read_bits(1)? == 1 {
            return Ok(AbbrevOp::Literal(self.read_vbr(8)?));
        }
        match self.read_bits(3)? {
            1 => {
                let w = u32::try_from(self.read_vbr(5)?)
                    .map_err(|_| BitcodeError::new("Fixed abbrev width > 64"))?;
                if w > 64 {
                    return Err(BitcodeError::new("Fixed abbrev width > 64"));
                }
                Ok(AbbrevOp::Fixed(w))
            }
            2 => {
                let w = u32::try_from(self.read_vbr(5)?)
                    .map_err(|_| BitcodeError::new("VBR abbrev width out of range"))?;
                if !(2..=64).contains(&w) {
                    return Err(BitcodeError::new("VBR abbrev width out of range"));
                }
                Ok(AbbrevOp::Vbr(w))
            }
            3 => {
                if !allow_aggregate {
                    return Err(BitcodeError::new("Nested array abbreviation is invalid"));
                }
                Ok(AbbrevOp::Array(Box::new(self.parse_abbrev_op(false)?)))
            }
            4 => Ok(AbbrevOp::Char6),
            5 => {
                if !allow_aggregate {
                    return Err(BitcodeError::new("Nested blob abbreviation is invalid"));
                }
                Ok(AbbrevOp::Blob)
            }
            other => Err(BitcodeError::new(format!(
                "Unknown abbreviation encoding {}",
                other
            ))),
        }
    }

    pub(crate) fn define_abbrev(&mut self) -> Result<Vec<AbbrevOp>, BitcodeError> {
        let num_ops = usize::try_from(self.read_vbr(5)?)
            .map_err(|_| BitcodeError::new("Invalid operand count in DEFINE_ABBREV"))?;
        if num_ops == 0 || num_ops > MAX_RECORD_OPS {
            return Err(BitcodeError::new("Invalid operand count in DEFINE_ABBREV"));
        }
        let mut ops = Vec::with_capacity(num_ops);
        let mut i = 0;
        while i < num_ops {
            let op = self.parse_abbrev_op(true)?;
            let is_terminal = matches!(op, AbbrevOp::Array(_) | AbbrevOp::Blob);
            if matches!(op, AbbrevOp::Array(_)) {
                i += 1;
                if i >= num_ops {
                    return Err(BitcodeError::new("Truncated Array abbreviation definition"));
                }
            }
            ops.push(op);
            i += 1;
            if is_terminal && i < num_ops {
                return Err(BitcodeError::new(
                    "Array/Blob must be final operand in abbreviation",
                ));
            }
        }
        Ok(ops)
    }

    pub(crate) fn read_record(&mut self, abbrev: &[AbbrevOp]) -> Result<Record, BitcodeError> {
        let mut raw = Vec::new();
        let mut blob = Vec::new();
        for op in abbrev {
            self.read_abbrev_scalar(op, &mut raw, &mut blob)?;
        }
        if raw.is_empty() {
            return Err(BitcodeError::new("Abbreviated record produced no code"));
        }
        Ok(Record {
            code: raw[0],
            ops: raw[1..].to_vec(),
            blob,
        })
    }

    pub(crate) fn enter_subblock(&mut self) -> Result<(u64, u32, usize), BitcodeError> {
        let block_id = self.read_vbr(8)?;
        let new_aw = u32::try_from(self.read_vbr(4)?)
            .map_err(|_| BitcodeError::new("Invalid subblock abbrev width"))?;
        if !(2..=32).contains(&new_aw) {
            return Err(BitcodeError::new(format!(
                "Invalid subblock abbrev width {}",
                new_aw
            )));
        }
        self.align32();
        let words = usize::try_from(self.read_bits(32)?)
            .map_err(|_| BitcodeError::new("Subblock size overflow"))?;
        let bit_len = words
            .checked_mul(32)
            .ok_or_else(|| BitcodeError::new("Subblock size overflow"))?;
        let end_bit = self
            .bit_pos
            .checked_add(bit_len)
            .ok_or_else(|| BitcodeError::new("Subblock end bit overflow"))?;
        if end_bit > self.total_bits {
            return Err(BitcodeError::new(format!(
                "Subblock {} extends past bitstream end",
                block_id
            )));
        }
        Ok((block_id, new_aw, end_bit))
    }

    pub(crate) fn read_unabbrev_record(&mut self) -> Result<Record, BitcodeError> {
        let code = self.read_vbr(6)?;
        let num_ops = usize::try_from(self.read_vbr(6)?)
            .map_err(|_| BitcodeError::new("UNABBREV_RECORD operand count out of bounds"))?;
        if num_ops > MAX_RECORD_OPS {
            return Err(BitcodeError::new(
                "UNABBREV_RECORD operand count out of bounds",
            ));
        }
        let mut ops = Vec::with_capacity(num_ops);
        for _ in 0..num_ops {
            ops.push(self.read_vbr(6)?);
        }
        Ok(Record {
            code,
            ops,
            blob: Vec::new(),
        })
    }
}

pub(crate) enum BlockEvent<'c, 'd> {
    Subblock {
        cursor: &'c mut BitCursor<'d>,
        block_id: u64,
        abbrev_width: u32,
        end_bit: usize,
    },
    Record(Record),
}

/// Shared block iteration helper eliminating repeated END/SUBBLOCK/ABBREV/RECORD decoding.
pub(crate) fn walk_block<F>(
    cursor: &mut BitCursor<'_>,
    block_id: u64,
    aw: u32,
    end_bit: usize,
    blockinfo: &HashMap<u64, Vec<Vec<AbbrevOp>>>,
    mut on_event: F,
) -> Result<(), BitcodeError>
where
    F: FnMut(BlockEvent<'_, '_>) -> Result<(), BitcodeError>,
{
    let mut abbrevs = blockinfo.get(&block_id).cloned().unwrap_or_default();
    while cursor.bit_pos < end_bit {
        let aid = cursor.read_bits(aw)?;
        match aid {
            0 => {
                cursor.align32();
                break;
            }
            1 => {
                let (sub_id, sub_aw, sub_end) = cursor.enter_subblock()?;
                on_event(BlockEvent::Subblock {
                    cursor,
                    block_id: sub_id,
                    abbrev_width: sub_aw,
                    end_bit: sub_end,
                })?;
                cursor.bit_pos = sub_end;
            }
            2 => {
                let ab = cursor.define_abbrev()?;
                if abbrevs.len() >= MAX_ABBREVS_PER_BLOCK {
                    return Err(BitcodeError::new("Local block abbreviation limit exceeded"));
                }
                abbrevs.push(ab);
            }
            3 => {
                let rec = cursor.read_unabbrev_record()?;
                on_event(BlockEvent::Record(rec))?;
            }
            id => {
                let idx = usize::try_from(id)
                    .ok()
                    .and_then(|v| v.checked_sub(4))
                    .ok_or_else(|| BitcodeError::new("Invalid abbrev ID"))?;
                let ab = abbrevs.get(idx).cloned().ok_or_else(|| {
                    BitcodeError::new(format!("Undefined abbrev ID {} in block {}", id, block_id))
                })?;
                let rec = cursor.read_record(&ab)?;
                on_event(BlockEvent::Record(rec))?;
            }
        }
    }
    Ok(())
}

/// Decodes a `BLOCKINFO_BLOCK` (`block_id = 0`), storing abbreviated record templates per target block.
pub(crate) fn decode_blockinfo_into(
    cursor: &mut BitCursor<'_>,
    aw: u32,
    end_bit: usize,
    blockinfo: &mut HashMap<u64, Vec<Vec<AbbrevOp>>>,
) -> Result<(), BitcodeError> {
    let mut target_bid: u64 = 0;
    while cursor.bit_pos < end_bit {
        match cursor.read_bits(aw)? {
            0 => {
                cursor.align32();
                break;
            }
            1 => {
                let (_, _, sub_end) = cursor.enter_subblock()?;
                cursor.bit_pos = sub_end;
            }
            2 => {
                let abbrev = cursor.define_abbrev()?;
                let list = blockinfo.entry(target_bid).or_default();
                if list.len() >= MAX_ABBREVS_PER_BLOCK {
                    return Err(BitcodeError::new("BLOCKINFO abbreviation limit exceeded"));
                }
                list.push(abbrev);
            }
            3 => {
                let rec = cursor.read_unabbrev_record()?;
                if rec.code == 1 && !rec.ops.is_empty() {
                    target_bid = rec.ops[0];
                }
            }
            id => {
                return Err(BitcodeError::new(format!(
                    "Unexpected abbrev ID {} inside BLOCKINFO_BLOCK",
                    id
                )));
            }
        }
    }
    Ok(())
}

#[inline]
pub(crate) fn decode_sign_rotated(v: u64) -> i64 {
    if (v & 1) == 0 {
        (v >> 1) as i64
    } else if v == 1 {
        i64::MIN
    } else {
        -((v >> 1) as i64)
    }
}

pub(crate) fn ops_to_string(ops: &[u64]) -> String {
    let bytes: Vec<u8> = ops.iter().map(|&b| (b & 0xFF) as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
