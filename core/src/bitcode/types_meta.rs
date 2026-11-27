//! LLVM module types, attributes, constants, symbol tables, and metadata block decoders.

use std::collections::HashMap;
use std::fmt::Write;

use super::bitstream::{
    decode_sign_rotated, ops_to_string, walk_block, AbbrevOp, BitCursor, BitcodeError, BlockEvent,
    Record,
};

pub(crate) const BLOCKINFO_BLOCK_ID: u64 = 0;
pub(crate) const MODULE_BLOCK_ID: u64 = 8;
pub(crate) const PARAMATTR_BLOCK_ID: u64 = 9;
pub(crate) const PARAMATTR_GROUP_BLOCK_ID: u64 = 10;
pub(crate) const CONSTANTS_BLOCK_ID: u64 = 11;
pub(crate) const FUNCTION_BLOCK_ID: u64 = 12;
pub(crate) const VALUE_SYMTAB_BLOCK_ID: u64 = 14;
pub(crate) const METADATA_BLOCK_ID: u64 = 15;
pub(crate) const TYPE_BLOCK_ID_NEW: u64 = 17;
pub(crate) const STRTAB_BLOCK_ID: u64 = 23;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LlvmType {
    Void,
    Half,
    BFloat,
    Float,
    Double,
    X86Fp80,
    Fp128,
    PpcFp128,
    Label,
    Metadata,
    Token,
    Opaque,
    Integer(u32),
    Pointer {
        pointee: Option<usize>,
        #[allow(dead_code)]
        addrspace: u32,
    },
    Array {
        len: u64,
        elem: usize,
    },
    Vector {
        len: u64,
        elem: usize,
        scalable: bool,
    },
    AnonStruct {
        is_packed: bool,
        fields: Vec<usize>,
    },
    NamedStruct {
        name: String,
        is_packed: bool,
        fields: Vec<usize>,
    },
    Function {
        is_vararg: bool,
        ret: usize,
        params: Vec<usize>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ValueEntry {
    pub(crate) ty_idx: usize,
    pub(crate) repr: String,
}

#[derive(Clone, Debug)]
pub(crate) struct GlobalVarDecl {
    pub(crate) val_id: usize,
    pub(crate) name: String,
    pub(crate) is_const: bool,
    pub(crate) val_ty_idx: usize,
    pub(crate) init_id: usize,
    pub(crate) linkage: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct FunctionDecl {
    pub(crate) val_id: usize,
    pub(crate) name: String,
    pub(crate) fn_ty_idx: usize,
    pub(crate) is_proto: bool,
    pub(crate) paramattr_id: usize,
    pub(crate) linkage: u64,
    pub(crate) personality_val_id: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum AttrEntry {
    Enum(u64),
    EnumInt(u64, u64),
    StrKey(String),
    StrKeyVal(String, String),
}

#[derive(Clone, Debug)]
pub(crate) struct AttrGroup {
    pub(crate) attr_idx: u64,
    pub(crate) entries: Vec<AttrEntry>,
}

#[derive(Clone, Debug)]
pub(crate) struct FunctionBodyIr {
    pub(crate) func_idx: usize,
    pub(crate) param_names: Vec<String>,
    pub(crate) instructions: Vec<(usize, String)>,
    pub(crate) bb_names: HashMap<usize, String>,
    pub(crate) num_blocks: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum MetadataItem {
    Str(String),
    Value { ty_idx: usize, val_repr: String },
    Node(Vec<Option<usize>>),
    DistinctNode(Vec<Option<usize>>),
    Other,
}

pub(crate) struct ModuleDecoder {
    pub(crate) strtab: Vec<u8>,
    pub(crate) blockinfo: HashMap<u64, Vec<Vec<AbbrevOp>>>,
    pub(crate) version: u64,
    pub(crate) source_filename: Option<String>,
    pub(crate) target_triple: Option<String>,
    pub(crate) target_datalayout: Option<String>,
    pub(crate) types: Vec<LlvmType>,
    pub(crate) pending_struct_name: Option<String>,
    pub(crate) attr_groups: HashMap<u64, AttrGroup>,
    pub(crate) attr_lists: Vec<Vec<u64>>,
    pub(crate) globals: Vec<GlobalVarDecl>,
    pub(crate) functions: Vec<FunctionDecl>,
    pub(crate) module_values: Vec<ValueEntry>,
    pub(crate) vst_global_names: HashMap<usize, String>,
    pub(crate) bodies: Vec<FunctionBodyIr>,
    pub(crate) next_body_cursor: usize,
    pub(crate) metadata: Vec<MetadataItem>,
    pub(crate) named_metadata: Vec<(String, Vec<usize>)>,
}

impl ModuleDecoder {
    pub(crate) fn new(strtab: Vec<u8>) -> Self {
        Self {
            strtab,
            blockinfo: HashMap::new(),
            version: 0,
            source_filename: None,
            target_triple: None,
            target_datalayout: None,
            types: Vec::new(),
            pending_struct_name: None,
            attr_groups: HashMap::new(),
            attr_lists: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            module_values: Vec::new(),
            vst_global_names: HashMap::new(),
            bodies: Vec::new(),
            next_body_cursor: 0,
            metadata: Vec::new(),
            named_metadata: Vec::new(),
        }
    }

    pub(crate) fn strtab_slice(&self, off: u64, len: u64) -> String {
        let (Ok(o), Ok(l)) = (usize::try_from(off), usize::try_from(len)) else {
            return String::new();
        };
        if let Some(end) = o.checked_add(l) {
            if end <= self.strtab.len() {
                return String::from_utf8_lossy(&self.strtab[o..end]).into_owned();
            }
        }
        String::new()
    }

    pub(crate) fn format_type(&self, ty_idx: usize) -> String {
        self.format_type_depth(ty_idx, 0)
    }

    fn format_type_depth(&self, ty_idx: usize, depth: usize) -> String {
        const MAX_TYPE_DEPTH: usize = 32;
        if depth >= MAX_TYPE_DEPTH {
            return "ptr".to_string();
        }
        let next = depth + 1;
        match self.types.get(ty_idx) {
            Some(LlvmType::Void) => "void".to_string(),
            Some(LlvmType::Half) => "half".to_string(),
            Some(LlvmType::BFloat) => "bfloat".to_string(),
            Some(LlvmType::Float) => "float".to_string(),
            Some(LlvmType::Double) => "double".to_string(),
            Some(LlvmType::X86Fp80) => "x86_fp80".to_string(),
            Some(LlvmType::Fp128) => "fp128".to_string(),
            Some(LlvmType::PpcFp128) => "ppc_fp128".to_string(),
            Some(LlvmType::Label) => "label".to_string(),
            Some(LlvmType::Metadata) => "metadata".to_string(),
            Some(LlvmType::Token) => "token".to_string(),
            Some(LlvmType::Opaque) => "opaque".to_string(),
            Some(LlvmType::Integer(bits)) => format!("i{}", bits),
            Some(LlvmType::Pointer { pointee: None, .. }) => "ptr".to_string(),
            Some(LlvmType::Pointer {
                pointee: Some(p), ..
            }) => format!("{}*", self.format_type_depth(*p, next)),
            Some(LlvmType::Array { len, elem }) => {
                format!("[{} x {}]", len, self.format_type_depth(*elem, next))
            }
            Some(LlvmType::Vector {
                len,
                elem,
                scalable,
            }) => {
                if *scalable {
                    format!(
                        "<vscale x {} x {}>",
                        len,
                        self.format_type_depth(*elem, next)
                    )
                } else {
                    format!("<{} x {}>", len, self.format_type_depth(*elem, next))
                }
            }
            Some(LlvmType::AnonStruct { is_packed, fields }) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|&f| self.format_type_depth(f, next))
                    .collect();
                if *is_packed {
                    format!("<{{ {} }}>", inner.join(", "))
                } else {
                    format!("{{ {} }}", inner.join(", "))
                }
            }
            Some(LlvmType::NamedStruct { name, .. }) => format!("%{}", format_ident(name)),
            Some(LlvmType::Function {
                is_vararg,
                ret,
                params,
            }) => {
                let mut ps: Vec<String> = params
                    .iter()
                    .map(|&p| self.format_type_depth(p, next))
                    .collect();
                if *is_vararg {
                    ps.push("...".to_string());
                }
                format!("{} ({})", self.format_type_depth(*ret, next), ps.join(", "))
            }
            None => "ptr".to_string(),
        }
    }

    pub(crate) fn fn_signature_parts(&self, fn_ty_idx: usize) -> Option<(usize, &[usize], bool)> {
        self.fn_signature_parts_depth(fn_ty_idx, 0)
    }

    fn fn_signature_parts_depth(
        &self,
        fn_ty_idx: usize,
        depth: usize,
    ) -> Option<(usize, &[usize], bool)> {
        const MAX_TYPE_DEPTH: usize = 32;
        if depth >= MAX_TYPE_DEPTH {
            return None;
        }
        match self.types.get(fn_ty_idx) {
            Some(LlvmType::Function {
                is_vararg,
                ret,
                params,
            }) => Some((*ret, params.as_slice(), *is_vararg)),
            Some(LlvmType::Pointer {
                pointee: Some(inner),
                ..
            }) => self.fn_signature_parts_depth(*inner, depth + 1),
            _ => None,
        }
    }

    pub(crate) fn type_is_void(&self, ty_idx: usize) -> bool {
        matches!(self.types.get(ty_idx), Some(LlvmType::Void))
    }

    pub(crate) fn zero_value_for_type(&self, ty_idx: usize) -> String {
        match self.types.get(ty_idx) {
            Some(LlvmType::Integer(1)) => "false".to_string(),
            Some(LlvmType::Integer(_)) => "0".to_string(),
            Some(LlvmType::Float | LlvmType::Double | LlvmType::Half | LlvmType::BFloat) => {
                "0.0".to_string()
            }
            Some(LlvmType::Pointer { .. }) => "null".to_string(),
            _ => "zeroinitializer".to_string(),
        }
    }

    pub(crate) fn ensure_opaque_ptr_ty(&self) -> usize {
        self.types
            .iter()
            .position(|t| {
                matches!(
                    t,
                    LlvmType::Pointer {
                        pointee: None,
                        addrspace: 0
                    }
                )
            })
            .or_else(|| {
                self.types
                    .iter()
                    .position(|t| matches!(t, LlvmType::Pointer { .. }))
            })
            .unwrap_or(0)
    }

    pub(crate) fn decode_type_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let blockinfo = self.blockinfo.clone();
        walk_block(cursor, TYPE_BLOCK_ID_NEW, aw, end_bit, &blockinfo, |ev| {
            if let BlockEvent::Record(rec) = ev {
                self.handle_type_record(rec);
            }
            Ok(())
        })
    }

    fn handle_type_record(&mut self, rec: Record) {
        match rec.code {
            1 => {
                if let Some(&cap) = rec.ops.first() {
                    let cap_clamped = super::to_idx(cap).min(65536);
                    if self.types.capacity() < cap_clamped {
                        self.types.reserve(cap_clamped - self.types.len());
                    }
                }
            }
            2 => self.types.push(LlvmType::Void),
            3 => self.types.push(LlvmType::Float),
            4 => self.types.push(LlvmType::Double),
            5 => self.types.push(LlvmType::Label),
            6 => {
                if let Some(name) = self.pending_struct_name.take() {
                    self.types.push(LlvmType::NamedStruct {
                        name,
                        is_packed: false,
                        fields: Vec::new(),
                    });
                } else {
                    self.types.push(LlvmType::Opaque);
                }
            }
            7 => {
                let bits = super::to_u32_sat(rec.ops.first().copied().unwrap_or(32));
                self.types.push(LlvmType::Integer(bits));
            }
            8 => {
                // TYPE_CODE_POINTER (legacy typed pointers): [pointee, addrspace]
                let pointee = rec.ops.first().map(|&x| super::to_idx(x));
                let addrspace = super::to_u32_sat(rec.ops.get(1).copied().unwrap_or(0));
                self.types.push(LlvmType::Pointer { pointee, addrspace });
            }
            9 => {
                // TYPE_CODE_FUNCTION_OLD: [is_vararg, ignored_attr, ret, params...]
                let is_vararg = rec.ops.first().copied().unwrap_or(0) != 0;
                let ret = super::to_idx(rec.ops.get(2).copied().unwrap_or(0));
                let params = rec.ops.iter().skip(3).map(|&x| super::to_idx(x)).collect();
                self.types.push(LlvmType::Function {
                    is_vararg,
                    ret,
                    params,
                });
            }
            10 => self.types.push(LlvmType::Half),
            11 => {
                let len = rec.ops.first().copied().unwrap_or(0);
                let elem = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                self.types.push(LlvmType::Array { len, elem });
            }
            12 => {
                let len = rec.ops.first().copied().unwrap_or(0);
                let elem = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                let scalable = rec.ops.get(2).copied().unwrap_or(0) != 0;
                self.types.push(LlvmType::Vector {
                    len,
                    elem,
                    scalable,
                });
            }
            13 => self.types.push(LlvmType::X86Fp80),
            14 => self.types.push(LlvmType::Fp128),
            15 => self.types.push(LlvmType::PpcFp128),
            16 => self.types.push(LlvmType::Metadata),
            18 => {
                let is_packed = rec.ops.first().copied().unwrap_or(0) != 0;
                let fields = rec.ops.iter().skip(1).map(|&x| super::to_idx(x)).collect();
                self.types.push(LlvmType::AnonStruct { is_packed, fields });
            }
            19 => self.pending_struct_name = Some(ops_to_string(&rec.ops)),
            20 => {
                let name = self
                    .pending_struct_name
                    .take()
                    .unwrap_or_else(|| format!("struct.{}", self.types.len()));
                let is_packed = rec.ops.first().copied().unwrap_or(0) != 0;
                let fields = rec.ops.iter().skip(1).map(|&x| super::to_idx(x)).collect();
                self.types.push(LlvmType::NamedStruct {
                    name,
                    is_packed,
                    fields,
                });
            }
            21 => {
                let is_vararg = rec.ops.first().copied().unwrap_or(0) != 0;
                let ret = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                let params = rec.ops.iter().skip(2).map(|&x| super::to_idx(x)).collect();
                self.types.push(LlvmType::Function {
                    is_vararg,
                    ret,
                    params,
                });
            }
            22 => self.types.push(LlvmType::Token),
            23 => self.types.push(LlvmType::BFloat),
            25 => {
                let addrspace = super::to_u32_sat(rec.ops.first().copied().unwrap_or(0));
                self.types.push(LlvmType::Pointer {
                    pointee: None,
                    addrspace,
                });
            }
            17 | 24 | 26..=64 => self.types.push(LlvmType::Opaque),
            _ => self.types.push(LlvmType::Opaque),
        }
    }

    pub(crate) fn decode_paramattr_group_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let blockinfo = self.blockinfo.clone();
        walk_block(
            cursor,
            PARAMATTR_GROUP_BLOCK_ID,
            aw,
            end_bit,
            &blockinfo,
            |ev| {
                if let BlockEvent::Record(rec) = ev {
                    self.handle_paramattr_group_record(rec);
                }
                Ok(())
            },
        )
    }

    fn handle_paramattr_group_record(&mut self, rec: Record) {
        if rec.code != 3 || rec.ops.len() < 2 {
            return;
        }
        let grpid = rec.ops[0];
        let attr_idx = rec.ops[1];
        let mut entries = Vec::new();
        let mut i = 2;
        while i < rec.ops.len() {
            let kind = rec.ops[i];
            i += 1;
            match kind {
                0 => {
                    if i < rec.ops.len() {
                        entries.push(AttrEntry::Enum(rec.ops[i]));
                        i += 1;
                    }
                }
                1 => {
                    if i + 1 < rec.ops.len() {
                        entries.push(AttrEntry::EnumInt(rec.ops[i], rec.ops[i + 1]));
                        i += 2;
                    } else {
                        break;
                    }
                }
                3 | 4 => {
                    let read_zstr = |ops: &[u64], pos: &mut usize| -> String {
                        let mut bytes = Vec::new();
                        while *pos < ops.len() && ops[*pos] != 0 {
                            bytes.push((ops[*pos] & 0xFF) as u8);
                            *pos += 1;
                        }
                        if *pos < ops.len() && ops[*pos] == 0 {
                            *pos += 1;
                        }
                        String::from_utf8_lossy(&bytes).into_owned()
                    };
                    let key = read_zstr(&rec.ops, &mut i);
                    if kind == 3 {
                        entries.push(AttrEntry::StrKey(key));
                    } else {
                        let val = read_zstr(&rec.ops, &mut i);
                        entries.push(AttrEntry::StrKeyVal(key, val));
                    }
                }
                5 => {
                    // ConstantRange attribute: skip (key, bit_width, lo_words, hi_words)
                    if i + 1 < rec.ops.len() {
                        let bit_width = super::to_idx(rec.ops[i + 1]).min(4096);
                        let active_words = bit_width.div_ceil(64).max(1);
                        i = i.saturating_add(2 + active_words * 2);
                    } else {
                        break;
                    }
                }
                6 => {
                    // ConstantRangeList attribute: conservatively consume remainder if format unknown
                    break;
                }
                _ => break,
            }
        }
        self.attr_groups
            .insert(grpid, AttrGroup { attr_idx, entries });
    }

    pub(crate) fn decode_paramattr_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let blockinfo = self.blockinfo.clone();
        walk_block(cursor, PARAMATTR_BLOCK_ID, aw, end_bit, &blockinfo, |ev| {
            if let BlockEvent::Record(rec) = ev {
                if rec.code == 2 {
                    self.attr_lists.push(rec.ops);
                }
            }
            Ok(())
        })
    }

    pub(crate) fn extract_indexed_type(&self, mut ty_idx: usize, idxs: &[u64]) -> usize {
        for &raw in idxs {
            let i = super::to_idx(raw);
            ty_idx = match self.types.get(ty_idx) {
                Some(LlvmType::Array { elem, .. }) | Some(LlvmType::Vector { elem, .. }) => *elem,
                Some(LlvmType::AnonStruct { fields, .. })
                | Some(LlvmType::NamedStruct { fields, .. }) => fields.get(i).copied().unwrap_or(0),
                _ => return 0,
            };
        }
        ty_idx
    }

    pub(crate) fn ensure_vec_ty(&mut self, len: u64, elem: usize, scalable: bool) -> usize {
        if let Some(pos) = self.types.iter().position(|t| {
            matches!(t, LlvmType::Vector { len: l, elem: e, scalable: s } if *l == len && *e == elem && *s == scalable)
        }) {
            return pos;
        }
        self.types.push(LlvmType::Vector {
            len,
            elem,
            scalable,
        });
        self.types.len() - 1
    }

    pub(crate) fn ensure_anon_struct_ty(&mut self, fields: Vec<usize>) -> usize {
        if let Some(pos) = self.types.iter().position(
            |t| matches!(t, LlvmType::AnonStruct { is_packed: false, fields: f } if *f == fields),
        ) {
            return pos;
        }
        self.types.push(LlvmType::AnonStruct {
            is_packed: false,
            fields,
        });
        self.types.len() - 1
    }

    pub(crate) fn ensure_i1_ty(&mut self) -> usize {
        if let Some(pos) = self
            .types
            .iter()
            .position(|t| matches!(t, LlvmType::Integer(1)))
        {
            return pos;
        }
        self.types.push(LlvmType::Integer(1));
        self.types.len() - 1
    }

    fn format_const_scalar_raw(&self, ty_idx: usize, raw: u64) -> String {
        match self.types.get(ty_idx) {
            Some(LlvmType::Integer(1)) => {
                if (raw & 1) == 0 {
                    "false".to_string()
                } else {
                    "true".to_string()
                }
            }
            Some(LlvmType::Integer(bits)) => {
                let w = (*bits).clamp(1, 64);
                let shift = 64u32.saturating_sub(w);
                let signed = ((raw << shift) as i64) >> shift;
                signed.to_string()
            }
            Some(LlvmType::Half) => format!("0x{:016X}", ieee_half_to_f64_bits(raw as u16)),
            Some(LlvmType::BFloat) => {
                let b = (f32::from_bits(((raw as u16) as u32) << 16) as f64).to_bits();
                format!("0x{:016X}", b)
            }
            Some(LlvmType::Float) => {
                let b = (f32::from_bits(raw as u32) as f64).to_bits();
                format!("0x{:016X}", b)
            }
            Some(LlvmType::Double) => format!("0x{:016X}", raw),
            _ => raw.to_string(),
        }
    }

    pub(crate) fn decode_constants_block(
        &self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
        values: &mut Vec<ValueEntry>,
    ) -> Result<(), BitcodeError> {
        let mut cur_ty: usize = 0;
        walk_block(
            cursor,
            CONSTANTS_BLOCK_ID,
            aw,
            end_bit,
            &self.blockinfo,
            |ev| {
                let BlockEvent::Record(rec) = ev else {
                    return Ok(());
                };
                match rec.code {
                    1 => {
                        if let Some(&ty) = rec.ops.first() {
                            cur_ty = super::to_idx(ty);
                        }
                    }
                    2 => values.push(ValueEntry {
                        ty_idx: cur_ty,
                        repr: self.zero_value_for_type(cur_ty),
                    }),
                    3 => values.push(ValueEntry {
                        ty_idx: cur_ty,
                        repr: "undef".to_string(),
                    }),
                    19 => values.push(ValueEntry {
                        ty_idx: cur_ty,
                        repr: "poison".to_string(),
                    }),
                    4 => {
                        let val = decode_sign_rotated(rec.ops.first().copied().unwrap_or(0));
                        let repr = if matches!(self.types.get(cur_ty), Some(LlvmType::Integer(1))) {
                            if val == 0 {
                                "false".to_string()
                            } else {
                                "true".to_string()
                            }
                        } else {
                            val.to_string()
                        };
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    5 => {
                        let bits = match self.types.get(cur_ty) {
                            Some(LlvmType::Integer(w)) => *w,
                            _ => 128,
                        };
                        let repr = decode_wide_int(&rec.ops, bits);
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    6 => {
                        let raw0 = rec.ops.first().copied().unwrap_or(0);
                        let repr = match self.types.get(cur_ty) {
                            Some(LlvmType::Fp128) if rec.ops.len() >= 2 => {
                                format!("0xL{:016X}{:016X}", raw0, rec.ops[1])
                            }
                            Some(LlvmType::PpcFp128) if rec.ops.len() >= 2 => {
                                format!("0xM{:016X}{:016X}", raw0, rec.ops[1])
                            }
                            Some(LlvmType::Float) => {
                                format!("0x{:016X}", (f32::from_bits(raw0 as u32) as f64).to_bits())
                            }
                            Some(LlvmType::Half) => {
                                format!("0x{:016X}", ieee_half_to_f64_bits(raw0 as u16))
                            }
                            Some(LlvmType::BFloat) => {
                                let b =
                                    (f32::from_bits(((raw0 as u16) as u32) << 16) as f64).to_bits();
                                format!("0x{:016X}", b)
                            }
                            _ => format!("0x{:016X}", raw0),
                        };
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    8 | 9 => {
                        let mut bytes: Vec<u8> =
                            rec.ops.iter().map(|&b| (b & 0xFF) as u8).collect();
                        if rec.code == 9 {
                            bytes.push(0);
                        }
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr: format_ir_bytes_literal(&bytes),
                        });
                    }
                    7 => {
                        let repr = self.format_const_aggregate(cur_ty, &rec.ops, values);
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    22 => {
                        let repr = self.format_const_data(cur_ty, &rec.ops);
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    11 => {
                        let opc = rec.ops.first().copied().unwrap_or(10);
                        let op_ty = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                        let op_val = super::to_idx(rec.ops.get(2).copied().unwrap_or(0));
                        let src_val_s = values
                            .get(op_val)
                            .map(|v| v.repr.clone())
                            .unwrap_or_else(|| mod_val_placeholder(op_val));
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr: format!(
                                "{} ({} {} to {})",
                                cast_opcode_name(opc),
                                self.format_type(op_ty),
                                src_val_s,
                                self.format_type(cur_ty)
                            ),
                        });
                    }
                    12 | 20 | 24 | 32 => {
                        // Constant GEP / INBOUNDS GEP (LLVM 14..20)
                        let (elem_ty_idx, op_start) = if rec.code == 32 && rec.ops.len() >= 4 {
                            (super::to_idx(rec.ops[0]), 2usize)
                        } else if (rec.code == 12 || rec.code == 20) && rec.ops.len() >= 3 {
                            let base_ty = super::to_idx(rec.ops[0]);
                            let pt_ty = match self.types.get(base_ty) {
                                Some(LlvmType::Pointer {
                                    pointee: Some(p), ..
                                }) => *p,
                                _ => base_ty,
                            };
                            (pt_ty, 0usize)
                        } else {
                            (cur_ty, rec.ops.len())
                        };
                        let mut args_s = Vec::new();
                        let mut k = op_start;
                        while k + 1 < rec.ops.len() {
                            let ot = super::to_idx(rec.ops[k]);
                            let ov = super::to_idx(rec.ops[k + 1]);
                            let vr = values
                                .get(ov)
                                .map(|v| v.repr.clone())
                                .unwrap_or_else(|| mod_val_placeholder(ov));
                            args_s.push(format!("{} {}", self.format_type(ot), vr));
                            k += 2;
                        }
                        let repr = if args_s.is_empty() {
                            self.zero_value_for_type(cur_ty)
                        } else {
                            format!(
                                "getelementptr ({}, {})",
                                self.format_type(elem_ty_idx),
                                args_s.join(", ")
                            )
                        };
                        values.push(ValueEntry {
                            ty_idx: cur_ty,
                            repr,
                        });
                    }
                    10 | 13..=18 | 21 | 23 | 25..=34 => values.push(ValueEntry {
                        ty_idx: cur_ty,
                        repr: self.zero_value_for_type(cur_ty),
                    }),
                    _ => {}
                }
                Ok(())
            },
        )
    }

    fn format_const_aggregate(&self, cur_ty: usize, ops: &[u64], values: &[ValueEntry]) -> String {
        if ops.is_empty() {
            return self.zero_value_for_type(cur_ty);
        }
        let fmt_elem = |elem_t: usize, op: u64| -> String {
            let vid = super::to_idx(op);
            let vr = values
                .get(vid)
                .map(|v| v.repr.clone())
                .unwrap_or_else(|| mod_val_placeholder(vid));
            format!("{} {}", self.format_type(elem_t), vr)
        };
        match self.types.get(cur_ty) {
            Some(LlvmType::Array { elem, .. }) => {
                let items: Vec<String> = ops.iter().map(|&op| fmt_elem(*elem, op)).collect();
                format!("[{}]", items.join(", "))
            }
            Some(LlvmType::Vector { elem, .. }) => {
                let items: Vec<String> = ops.iter().map(|&op| fmt_elem(*elem, op)).collect();
                format!("<{}>", items.join(", "))
            }
            Some(LlvmType::AnonStruct { is_packed, fields })
            | Some(LlvmType::NamedStruct {
                is_packed, fields, ..
            }) if ops.len() == fields.len() => {
                let items: Vec<String> = fields
                    .iter()
                    .zip(ops.iter())
                    .map(|(&ft, &op)| fmt_elem(ft, op))
                    .collect();
                if *is_packed {
                    format!("<{{ {} }}>", items.join(", "))
                } else {
                    format!("{{ {} }}", items.join(", "))
                }
            }
            _ => self.zero_value_for_type(cur_ty),
        }
    }

    fn format_const_data(&self, cur_ty: usize, ops: &[u64]) -> String {
        if ops.is_empty() {
            return self.zero_value_for_type(cur_ty);
        }
        let (elem_ty, open, close) = match self.types.get(cur_ty) {
            Some(LlvmType::Array { elem, .. }) => (*elem, "[", "]"),
            Some(LlvmType::Vector { elem, .. }) => (*elem, "<", ">"),
            _ => return self.zero_value_for_type(cur_ty),
        };
        let elem_s = self.format_type(elem_ty);
        let items: Vec<String> = ops
            .iter()
            .map(|&raw| format!("{} {}", elem_s, self.format_const_scalar_raw(elem_ty, raw)))
            .collect();
        format!("{}{}{}", open, items.join(", "), close)
    }

    pub(crate) fn decode_module_vst(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let blockinfo = self.blockinfo.clone();
        walk_block(
            cursor,
            VALUE_SYMTAB_BLOCK_ID,
            aw,
            end_bit,
            &blockinfo,
            |ev| {
                let BlockEvent::Record(rec) = ev else {
                    return Ok(());
                };
                let (val_id, name_start) = if rec.code == 1 && rec.ops.len() >= 2 {
                    (super::to_idx(rec.ops[0]), 1)
                } else if rec.code == 3 && rec.ops.len() >= 3 {
                    (super::to_idx(rec.ops[0]), 2)
                } else {
                    return Ok(());
                };
                let name = ops_to_string(&rec.ops[name_start..]);
                if !name.is_empty() {
                    self.vst_global_names.insert(val_id, name);
                }
                Ok(())
            },
        )
    }

    pub(crate) fn decode_metadata_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let blockinfo = self.blockinfo.clone();
        let mut pending_name: Option<String> = None;
        walk_block(
            cursor,
            METADATA_BLOCK_ID,
            aw,
            end_bit,
            &blockinfo,
            |ev| match ev {
                BlockEvent::Record(rec) => self.handle_metadata_record(rec, &mut pending_name),
                BlockEvent::Subblock { .. } => Ok(()),
            },
        )
    }

    fn handle_metadata_record(
        &mut self,
        rec: Record,
        pending_name: &mut Option<String>,
    ) -> Result<(), BitcodeError> {
        match rec.code {
            1 => self
                .metadata
                .push(MetadataItem::Str(ops_to_string(&rec.ops))),
            2 => {
                let ty_idx = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                let val_id = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                let val_repr = self
                    .module_values
                    .get(val_id)
                    .map(|v| v.repr.clone())
                    .unwrap_or_else(|| "0".to_string());
                self.metadata.push(MetadataItem::Value { ty_idx, val_repr });
            }
            3 | 11 => {
                let refs = rec
                    .ops
                    .iter()
                    .map(|&v| {
                        if v == 0 {
                            None
                        } else {
                            Some(super::to_idx(v - 1))
                        }
                    })
                    .collect();
                if rec.code == 11 {
                    self.metadata.push(MetadataItem::DistinctNode(refs));
                } else {
                    self.metadata.push(MetadataItem::Node(refs));
                }
            }
            4 => *pending_name = Some(ops_to_string(&rec.ops)),
            5..=7 => {}
            8..=9 | 12..=34 | 36..=50 => self.metadata.push(MetadataItem::Other),
            10 => {
                if let Some(name) = pending_name.take() {
                    let ids = rec.ops.iter().map(|&v| super::to_idx(v)).collect();
                    self.named_metadata.push((name, ids));
                }
            }
            35 => {
                let raw_count = rec.ops.first().copied().unwrap_or(0);
                let raw_offset = rec.ops.get(1).copied().unwrap_or(0);
                if let (Ok(count), Ok(offset)) =
                    (usize::try_from(raw_count), usize::try_from(raw_offset))
                {
                    if offset <= rec.blob.len() {
                        let mut len_cursor = BitCursor::new(&rec.blob[..offset])?;
                        let mut pos = offset;
                        for _ in 0..count {
                            let slen = usize::try_from(len_cursor.read_vbr(6)?).map_err(|_| {
                                BitcodeError::new("Metadata string length overflow")
                            })?;
                            let s_end = pos.saturating_add(slen).min(rec.blob.len());
                            let s = String::from_utf8_lossy(&rec.blob[pos..s_end]).into_owned();
                            self.metadata.push(MetadataItem::Str(s));
                            pos = s_end;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub(crate) fn mod_val_placeholder(val_id: usize) -> String {
    format!("__MOD_VAL_{}__", val_id)
}

fn ieee_half_to_f64_bits(h: u16) -> u64 {
    let sign = ((h as u32) & 0x8000) << 16;
    let exp = (h >> 10) & 0x1F;
    let mant = (h & 0x03FF) as u32;
    let f32_bits: u32 = if exp == 0 {
        if mant == 0 {
            sign
        } else {
            let mut e = -14i32;
            let mut m = mant;
            while (m & 0x0400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x03FF;
            let new_exp = ((e + 127) as u32) << 23;
            sign | new_exp | (m << 13)
        }
    } else if exp == 31 {
        sign | 0x7F80_0000 | (mant << 13)
    } else {
        let new_exp = ((exp as u32) + (127 - 15)) << 23;
        sign | new_exp | (mant << 13)
    };
    (f32::from_bits(f32_bits) as f64).to_bits()
}

pub(crate) fn format_ident(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'$'))
    {
        s.to_string()
    } else {
        format!("\"{}\"", escape_llvm_str(s))
    }
}

pub(crate) fn escape_llvm_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(b as char),
            other => {
                let _ = write!(out, "\\{:02X}", other);
            }
        }
    }
    out
}

pub(crate) fn format_ir_bytes_literal(bytes: &[u8]) -> String {
    let mut out = String::from("c\"");
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(b as char),
            other => {
                let _ = write!(out, "\\{:02X}", other);
            }
        }
    }
    out.push('"');
    out
}

pub(crate) fn cast_opcode_name(opc: u64) -> &'static str {
    match opc {
        0 => "trunc",
        1 => "zext",
        2 => "sext",
        3 => "fptoui",
        4 => "fptosi",
        5 => "uitofp",
        6 => "sitofp",
        7 => "fptrunc",
        8 => "fpext",
        9 => "ptrtoint",
        10 => "inttoptr",
        11 => "bitcast",
        12 => "addrspacecast",
        _ => "bitcast",
    }
}

fn decode_wide_int(ops: &[u64], bits: u32) -> String {
    if ops.is_empty() {
        return "0".to_string();
    }
    if bits <= 128 && ops.len() <= 2 {
        let w0 = decode_sign_rotated(ops[0]) as u64 as u128;
        let w1 = if ops.len() >= 2 {
            decode_sign_rotated(ops[1]) as u64 as u128
        } else if (w0 as i64) < 0 && bits > 64 {
            u64::MAX as u128
        } else {
            0u128
        };
        let combined = w0 | (w1 << 64);
        let shift = 128u32.saturating_sub(bits.clamp(1, 128));
        (((combined as i128) << shift) >> shift).to_string()
    } else {
        decode_sign_rotated(ops[0]).to_string()
    }
}
