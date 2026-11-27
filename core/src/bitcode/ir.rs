//! Module-level orchestration, function-body instruction decoding, and LLVM IR rendering.

use std::collections::HashMap;
use std::fmt::Write;

use super::bitstream::{
    decode_blockinfo_into, decode_sign_rotated, ops_to_string, walk_block, BitCursor, BitcodeError,
    BlockEvent, Record,
};
use super::types_meta::{
    cast_opcode_name, format_ident, mod_val_placeholder, FunctionBodyIr, FunctionDecl,
    GlobalVarDecl, LlvmType, ModuleDecoder, ValueEntry, BLOCKINFO_BLOCK_ID, CONSTANTS_BLOCK_ID,
    FUNCTION_BLOCK_ID, METADATA_BLOCK_ID, MODULE_BLOCK_ID, PARAMATTR_BLOCK_ID,
    PARAMATTR_GROUP_BLOCK_ID, TYPE_BLOCK_ID_NEW, VALUE_SYMTAB_BLOCK_ID,
};

impl ModuleDecoder {
    pub(crate) fn parse_top_level(
        &mut self,
        cursor: &mut BitCursor<'_>,
    ) -> Result<(), BitcodeError> {
        let mut seen_module = false;
        while cursor.bit_pos < cursor.total_bits {
            match cursor.read_bits(2)? {
                0 => {
                    cursor.align32();
                    break;
                }
                1 => {
                    let (bid, new_aw, end_bit) = cursor.enter_subblock()?;
                    match bid {
                        BLOCKINFO_BLOCK_ID => {
                            decode_blockinfo_into(cursor, new_aw, end_bit, &mut self.blockinfo)?;
                        }
                        MODULE_BLOCK_ID => {
                            seen_module = true;
                            self.decode_module_block(cursor, new_aw, end_bit)?;
                        }
                        _ => cursor.bit_pos = end_bit,
                    }
                    cursor.bit_pos = end_bit;
                }
                _ => {
                    return Err(BitcodeError::new(
                        "Unexpected top-level non-block record in bitstream",
                    ));
                }
            }
        }
        if !seen_module {
            return Err(BitcodeError::new("Bitcode contains no MODULE_BLOCK"));
        }
        Ok(())
    }

    fn decode_module_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let mut abbrevs = self
            .blockinfo
            .get(&MODULE_BLOCK_ID)
            .cloned()
            .unwrap_or_default();
        while cursor.bit_pos < end_bit {
            match cursor.read_bits(aw)? {
                0 => {
                    cursor.align32();
                    break;
                }
                1 => {
                    let (bid, new_aw, sub_end) = cursor.enter_subblock()?;
                    match bid {
                        BLOCKINFO_BLOCK_ID => {
                            decode_blockinfo_into(cursor, new_aw, sub_end, &mut self.blockinfo)?;
                        }
                        TYPE_BLOCK_ID_NEW => self.decode_type_block(cursor, new_aw, sub_end)?,
                        PARAMATTR_GROUP_BLOCK_ID => {
                            self.decode_paramattr_group_block(cursor, new_aw, sub_end)?;
                        }
                        PARAMATTR_BLOCK_ID => {
                            self.decode_paramattr_block(cursor, new_aw, sub_end)?;
                        }
                        CONSTANTS_BLOCK_ID => {
                            let mut vals = self.module_values.clone();
                            self.decode_constants_block(cursor, new_aw, sub_end, &mut vals)?;
                            self.module_values = vals;
                        }
                        FUNCTION_BLOCK_ID => self.decode_function_block(cursor, new_aw, sub_end)?,
                        VALUE_SYMTAB_BLOCK_ID => {
                            self.decode_module_vst(cursor, new_aw, sub_end)?;
                        }
                        METADATA_BLOCK_ID => self.decode_metadata_block(cursor, new_aw, sub_end)?,
                        _ => cursor.bit_pos = sub_end,
                    }
                    cursor.bit_pos = sub_end;
                }
                2 => {
                    let ab = cursor.define_abbrev()?;
                    if abbrevs.len() >= super::bitstream::MAX_ABBREVS_PER_BLOCK {
                        return Err(BitcodeError::new("MODULE_BLOCK abbrev limit exceeded"));
                    }
                    abbrevs.push(ab);
                }
                3 => {
                    let rec = cursor.read_unabbrev_record()?;
                    self.handle_module_record(rec)?;
                }
                id => {
                    let idx = usize::try_from(id)
                        .ok()
                        .and_then(|v| v.checked_sub(4))
                        .ok_or_else(|| BitcodeError::new("Invalid MODULE_BLOCK abbrev"))?;
                    let ab = abbrevs.get(idx).cloned().ok_or_else(|| {
                        BitcodeError::new(format!("Undefined abbrev ID {} in MODULE_BLOCK", id))
                    })?;
                    let rec = cursor.read_record(&ab)?;
                    self.handle_module_record(rec)?;
                }
            }
        }
        Ok(())
    }

    fn handle_module_record(&mut self, rec: Record) -> Result<(), BitcodeError> {
        match rec.code {
            1 => {
                if let Some(&v) = rec.ops.first() {
                    self.version = v;
                }
            }
            2 => self.target_triple = Some(ops_to_string(&rec.ops)),
            3 => self.target_datalayout = Some(ops_to_string(&rec.ops)),
            16 => self.source_filename = Some(ops_to_string(&rec.ops)),
            7 => {
                // MODULE_CODE_GLOBALVAR
                let off = if self.version >= 2 { 2 } else { 0 };
                let name = if self.version >= 2 && rec.ops.len() >= 2 {
                    self.strtab_slice(rec.ops[0], rec.ops[1])
                } else {
                    String::new()
                };
                let raw_ty = super::to_idx(rec.ops.get(off).copied().unwrap_or(0));
                let flags = rec.ops.get(off + 1).copied().unwrap_or(0);
                let is_const = (flags & 1) != 0;
                let explicit_ty = (flags & 2) != 0;
                let init_id = super::to_idx(rec.ops.get(off + 2).copied().unwrap_or(0));
                let linkage = rec.ops.get(off + 3).copied().unwrap_or(0);
                let val_ty_idx = if explicit_ty {
                    raw_ty
                } else {
                    match self.types.get(raw_ty) {
                        Some(LlvmType::Pointer {
                            pointee: Some(p), ..
                        }) => *p,
                        _ => raw_ty,
                    }
                };
                let val_id = self.module_values.len();
                let ptr_ty = self.ensure_opaque_ptr_ty();
                let repr = if name.is_empty() {
                    mod_val_placeholder(val_id)
                } else {
                    format!("@{}", format_ident(&name))
                };
                self.module_values.push(ValueEntry {
                    ty_idx: ptr_ty,
                    repr,
                });
                self.globals.push(GlobalVarDecl {
                    val_id,
                    name,
                    is_const,
                    val_ty_idx,
                    init_id,
                    linkage,
                });
            }
            8 => {
                // MODULE_CODE_FUNCTION
                let off = if self.version >= 2 { 2 } else { 0 };
                let name = if self.version >= 2 && rec.ops.len() >= 2 {
                    self.strtab_slice(rec.ops[0], rec.ops[1])
                } else {
                    String::new()
                };
                let fn_ty_idx = super::to_idx(rec.ops.get(off).copied().unwrap_or(0));
                let is_proto = rec.ops.get(off + 2).copied().unwrap_or(0) != 0;
                let linkage = rec.ops.get(off + 3).copied().unwrap_or(0);
                let paramattr_id = super::to_idx(rec.ops.get(off + 4).copied().unwrap_or(0));
                let personality_val_id = rec
                    .ops
                    .get(off + 14)
                    .copied()
                    .filter(|&v| v > 0)
                    .map(|v| super::to_idx(v - 1));
                let val_id = self.module_values.len();
                let ptr_ty = self.ensure_opaque_ptr_ty();
                let repr = if name.is_empty() {
                    mod_val_placeholder(val_id)
                } else {
                    format!("@{}", format_ident(&name))
                };
                self.module_values.push(ValueEntry {
                    ty_idx: ptr_ty,
                    repr,
                });
                self.functions.push(FunctionDecl {
                    val_id,
                    name,
                    fn_ty_idx,
                    is_proto,
                    paramattr_id,
                    linkage,
                    personality_val_id,
                });
            }
            9 | 14 | 24 => {
                let name = if self.version >= 2 && rec.code != 9 && rec.ops.len() >= 2 {
                    self.strtab_slice(rec.ops[0], rec.ops[1])
                } else {
                    String::new()
                };
                let val_id = self.module_values.len();
                let ptr_ty = self.ensure_opaque_ptr_ty();
                let repr = if name.is_empty() {
                    mod_val_placeholder(val_id)
                } else {
                    format!("@{}", format_ident(&name))
                };
                self.module_values.push(ValueEntry {
                    ty_idx: ptr_ty,
                    repr,
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn decode_function_block(
        &mut self,
        cursor: &mut BitCursor<'_>,
        aw: u32,
        end_bit: usize,
    ) -> Result<(), BitcodeError> {
        let func_idx = self.find_next_defined_func_idx()?;
        let fn_ty_idx = self.functions[func_idx].fn_ty_idx;
        let param_tys: Vec<usize> = self
            .fn_signature_parts(fn_ty_idx)
            .map(|(_, ps, _)| ps.to_vec())
            .unwrap_or_default();

        let mut values = self.module_values.clone();
        let param_start = values.len();
        let mut next_local_num = 0usize;
        for &pty in &param_tys {
            let name = format!("%{}", next_local_num);
            next_local_num += 1;
            values.push(ValueEntry {
                ty_idx: pty,
                repr: name,
            });
        }
        let param_end = values.len();

        let mut instructions: Vec<(usize, String)> = Vec::new();
        let mut bb_names: HashMap<usize, String> = HashMap::new();
        let mut num_blocks: usize = 1;
        let mut cur_bb: usize = 0;
        let mut block_terminated = false;
        let rel_ids = self.version >= 1;
        let blockinfo = self.blockinfo.clone();

        walk_block(
            cursor,
            FUNCTION_BLOCK_ID,
            aw,
            end_bit,
            &blockinfo,
            |ev| match ev {
                BlockEvent::Subblock {
                    cursor: cur,
                    block_id,
                    abbrev_width,
                    end_bit: sub_end,
                } => match block_id {
                    CONSTANTS_BLOCK_ID => {
                        self.decode_constants_block(cur, abbrev_width, sub_end, &mut values)
                    }
                    VALUE_SYMTAB_BLOCK_ID => walk_block(
                        cur,
                        VALUE_SYMTAB_BLOCK_ID,
                        abbrev_width,
                        sub_end,
                        &blockinfo,
                        |inner| {
                            if let BlockEvent::Record(vst_rec) = inner {
                                Self::apply_func_vst_record(vst_rec, &mut values, &mut bb_names);
                            }
                            Ok(())
                        },
                    ),
                    _ => Ok(()),
                },
                BlockEvent::Record(rec) => self.handle_function_record(
                    rec,
                    rel_ids,
                    &mut values,
                    &mut next_local_num,
                    &mut instructions,
                    &mut num_blocks,
                    &mut cur_bb,
                    &mut block_terminated,
                ),
            },
        )?;

        let resolved_instructions: Vec<(usize, String)> = instructions
            .into_iter()
            .map(|(bb, line)| {
                let resolved_line =
                    super::render::replace_indexed_placeholders(&line, "__VAL_ID_", |vid| {
                        values
                            .get(vid)
                            .map(|v| v.repr.clone())
                            .unwrap_or_else(|| format!("%unk.{}", vid))
                    });
                (bb, resolved_line)
            })
            .collect();

        let param_names: Vec<String> = values[param_start..param_end]
            .iter()
            .map(|v| v.repr.clone())
            .collect();

        self.bodies.push(FunctionBodyIr {
            func_idx,
            param_names,
            instructions: resolved_instructions,
            bb_names,
            num_blocks,
        });
        Ok(())
    }

    fn find_next_defined_func_idx(&mut self) -> Result<usize, BitcodeError> {
        while self.next_body_cursor < self.functions.len() {
            let idx = self.next_body_cursor;
            self.next_body_cursor += 1;
            if !self.functions[idx].is_proto {
                return Ok(idx);
            }
        }
        Err(BitcodeError::new(
            "Found FUNCTION_BLOCK without matching defined FUNCTION record",
        ))
    }

    fn apply_func_vst_record(
        rec: Record,
        values: &mut [ValueEntry],
        bb_names: &mut HashMap<usize, String>,
    ) {
        if rec.code == 2 && rec.ops.len() >= 2 {
            let bb = super::to_idx(rec.ops[0]);
            let name = ops_to_string(&rec.ops[1..]);
            if !name.is_empty() {
                bb_names.insert(bb, name);
            }
        } else if rec.code == 1 && rec.ops.len() >= 2 {
            let vid = super::to_idx(rec.ops[0]);
            let name = ops_to_string(&rec.ops[1..]);
            if !name.is_empty() {
                if let Some(entry) = values.get_mut(vid) {
                    if !entry.repr.starts_with('@') && !entry.repr.starts_with("__MOD_VAL_") {
                        entry.repr = format!("%{}", format_ident(&name));
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_function_record(
        &mut self,
        rec: Record,
        rel_ids: bool,
        values: &mut Vec<ValueEntry>,
        next_local_num: &mut usize,
        instructions: &mut Vec<(usize, String)>,
        num_blocks: &mut usize,
        cur_bb: &mut usize,
        block_terminated: &mut bool,
    ) -> Result<(), BitcodeError> {
        // Non-instruction metadata/debug records inside FUNCTION_BLOCK must neither
        // consume a Value ID slot nor advance basic blocks after a terminator.
        // Note: 58 (FREEZE) and 59 (ATOMICRMW) ARE value-producing instructions.
        if matches!(rec.code, 1 | 33 | 35 | 55 | 60..=65) {
            if rec.code == 1 {
                if let Some(&n) = rec.ops.first() {
                    *num_blocks = super::to_idx(n).clamp(1, 65536);
                }
            }
            return Ok(());
        }

        if *block_terminated {
            *cur_bb += 1;
            *block_terminated = false;
        }

        let next_id = values.len();
        let resolve = |raw: u64| -> usize {
            if rel_ids {
                usize::try_from(raw)
                    .ok()
                    .and_then(|d| next_id.checked_sub(d))
                    .unwrap_or(usize::MAX)
            } else {
                usize::try_from(raw).unwrap_or(usize::MAX)
            }
        };
        let resolve_signed = |raw: u64| -> usize {
            if rel_ids {
                let delta = decode_sign_rotated(raw);
                let target = (next_id as i128).saturating_sub(delta as i128);
                usize::try_from(target).unwrap_or(usize::MAX)
            } else {
                usize::try_from(raw).unwrap_or(usize::MAX)
            }
        };
        let get_val = |idx: usize, values: &[ValueEntry]| -> (String, String) {
            match values.get(idx) {
                Some(v) => {
                    let repr = if v.repr.starts_with('%') {
                        val_placeholder(idx)
                    } else {
                        v.repr.clone()
                    };
                    (self.format_type(v.ty_idx), repr)
                }
                None => ("ptr".to_string(), val_placeholder(idx)),
            }
        };

        match rec.code {
            2 => {
                // BINOP
                let lhs = resolve(rec.ops.first().copied().unwrap_or(0));
                let rhs = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let opc = rec.ops.get(2).copied().unwrap_or(0);
                let lty = values.get(lhs).map(|v| v.ty_idx).unwrap_or(0);
                let is_fp = matches!(
                    self.types.get(lty),
                    Some(
                        LlvmType::Float
                            | LlvmType::Double
                            | LlvmType::Half
                            | LlvmType::BFloat
                            | LlvmType::X86Fp80
                            | LlvmType::Fp128
                            | LlvmType::PpcFp128
                    )
                );
                let (ty_s, l_s) = get_val(lhs, values);
                let (_, r_s) = get_val(rhs, values);
                let res_ph = alloc_local(lty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = {} {} {}, {}",
                        res_ph,
                        binop_name(opc, is_fp),
                        ty_s,
                        l_s,
                        r_s
                    ),
                ));
            }
            3 => {
                // CAST
                let val = resolve(rec.ops.first().copied().unwrap_or(0));
                let dest_ty = super::to_idx(rec.ops.get(1).copied().unwrap_or(0));
                let opc = rec.ops.get(2).copied().unwrap_or(0);
                let (src_ty_s, val_s) = get_val(val, values);
                let res_ph = alloc_local(dest_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = {} {} {} to {}",
                        res_ph,
                        cast_opcode_name(opc),
                        src_ty_s,
                        val_s,
                        self.format_type(dest_ty)
                    ),
                ));
            }
            10 => {
                *block_terminated = true;
                if let Some(&op0) = rec.ops.first() {
                    let (ty_s, val_s) = get_val(resolve(op0), values);
                    instructions.push((*cur_bb, format!("ret {} {}", ty_s, val_s)));
                } else {
                    instructions.push((*cur_bb, "ret void".to_string()));
                }
            }
            11 => {
                *block_terminated = true;
                if rec.ops.len() >= 3 {
                    let t_bb = super::to_idx(rec.ops[0]);
                    let f_bb = super::to_idx(rec.ops[1]);
                    let (_, c_s) = get_val(resolve(rec.ops[2]), values);
                    instructions.push((
                        *cur_bb,
                        format!(
                            "br i1 {}, label %{}, label %{}",
                            c_s,
                            bb_placeholder(t_bb),
                            bb_placeholder(f_bb)
                        ),
                    ));
                } else if let Some(&t_bb) = rec.ops.first() {
                    instructions.push((
                        *cur_bb,
                        format!("br label %{}", bb_placeholder(super::to_idx(t_bb))),
                    ));
                }
            }
            12 => {
                *block_terminated = true;
                let cond_ty = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                let cond = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let def_bb = super::to_idx(rec.ops.get(2).copied().unwrap_or(0));
                let (_, c_s) = get_val(cond, values);
                let ty_s = self.format_type(cond_ty);
                let mut line = format!(
                    "switch {} {}, label %{} [",
                    ty_s,
                    c_s,
                    bb_placeholder(def_bb)
                );
                let mut idx = 3;
                while idx + 1 < rec.ops.len() {
                    let (_, cv_s) = get_val(super::to_idx(rec.ops[idx]), values);
                    let case_bb = super::to_idx(rec.ops[idx + 1]);
                    let _ = write!(
                        line,
                        " {} {}, label %{}",
                        ty_s,
                        cv_s,
                        bb_placeholder(case_bb)
                    );
                    idx += 2;
                }
                line.push_str(" ]");
                instructions.push((*cur_bb, line));
            }
            31 => {
                // INDIRECTBR: [addr_ty, addr_rel_id, dest_bb0, dest_bb1, ...]
                *block_terminated = true;
                let line = if rec.ops.len() >= 2 {
                    let addr_ty = super::to_idx(rec.ops[0]);
                    let addr_id = resolve(rec.ops[1]);
                    let (val_ty_s, addr_s) = get_val(addr_id, values);
                    let ty_s = if self.types.get(addr_ty).is_some() {
                        self.format_type(addr_ty)
                    } else {
                        val_ty_s
                    };
                    let dests: Vec<String> = rec.ops[2..]
                        .iter()
                        .map(|&bb| format!("label %{}", bb_placeholder(super::to_idx(bb))))
                        .collect();
                    format!("indirectbr {} {}, [{}]", ty_s, addr_s, dests.join(", "))
                } else {
                    "; indirectbr".to_string()
                };
                instructions.push((*cur_bb, line));
            }
            14 | 15 | 50 | 51 | 53 => {
                *block_terminated = true;
                let line = match rec.code {
                    15 => "unreachable".to_string(),
                    _ => "; terminator".to_string(),
                };
                instructions.push((*cur_bb, line));
            }
            39 => {
                *block_terminated = true;
                let line = if let Some(&op0) = rec.ops.first() {
                    let (ty_s, val_s) = get_val(resolve(op0), values);
                    format!("resume {} {}", ty_s, val_s)
                } else {
                    "resume void".to_string()
                };
                instructions.push((*cur_bb, line));
            }
            52 => {
                *block_terminated = true;
                let token_ty = self
                    .types
                    .iter()
                    .position(|t| matches!(t, LlvmType::Token))
                    .unwrap_or(0);
                let _res_ph = alloc_local(token_ty, values, next_local_num);
                instructions.push((*cur_bb, "; catchswitch".to_string()));
            }
            16 => {
                // PHI: incoming values use sign-rotated signed relative IDs (may be forward backedges)
                let ty_idx = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                let ty_s = self.format_type(ty_idx);
                let mut pairs = Vec::new();
                let mut idx = 1;
                while idx + 1 < rec.ops.len() {
                    let target_id = resolve_signed(rec.ops[idx]);
                    let v_s = match values.get(target_id) {
                        Some(v) if !v.repr.starts_with('%') => v.repr.clone(),
                        _ => val_placeholder(target_id),
                    };
                    let bb = super::to_idx(rec.ops[idx + 1]);
                    pairs.push(format!("[ {}, %{} ]", v_s, bb_placeholder(bb)));
                    idx += 2;
                }
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!("{} = phi {} {}", res_ph, ty_s, pairs.join(", ")),
                ));
            }
            19 => {
                let elem_ty = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                let ptr_ty = self.ensure_opaque_ptr_ty();
                let res_ph = alloc_local(ptr_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!("{} = alloca {}", res_ph, self.format_type(elem_ty)),
                ));
            }
            20 => {
                let ptr = resolve(rec.ops.first().copied().unwrap_or(0));
                let val_ty = if rec.ops.len() >= 4 {
                    super::to_idx(rec.ops[1])
                } else {
                    match values.get(ptr).and_then(|v| self.types.get(v.ty_idx)) {
                        Some(LlvmType::Pointer {
                            pointee: Some(p), ..
                        }) => *p,
                        _ => 0,
                    }
                };
                let (pty_s, p_s) = get_val(ptr, values);
                let res_ph = alloc_local(val_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = load {}, {} {}",
                        res_ph,
                        self.format_type(val_ty),
                        pty_s,
                        p_s
                    ),
                ));
            }
            24 | 44 => {
                let (pty_s, p_s) = get_val(resolve(rec.ops.first().copied().unwrap_or(0)), values);
                let (vty_s, v_s) = get_val(resolve(rec.ops.get(1).copied().unwrap_or(0)), values);
                instructions.push((
                    *cur_bb,
                    format!("store {} {}, {} {}", vty_s, v_s, pty_s, p_s),
                ));
            }
            9 | 28 => {
                let lhs = resolve(rec.ops.first().copied().unwrap_or(0));
                let rhs = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let pred = rec.ops.get(2).copied().unwrap_or(32);
                let (lty_s, l_s) = get_val(lhs, values);
                let (_, r_s) = get_val(rhs, values);
                let i1_ty = self
                    .types
                    .iter()
                    .position(|t| matches!(t, LlvmType::Integer(1)))
                    .unwrap_or(0);
                let res_ph = alloc_local(i1_ty, values, next_local_num);
                let (inst, p_str) = cmp_predicate_str(pred);
                instructions.push((
                    *cur_bb,
                    format!("{} = {} {} {} {}, {}", res_ph, inst, p_str, lty_s, l_s, r_s),
                ));
            }
            5 | 29 => {
                let op0 = resolve(rec.ops.first().copied().unwrap_or(0));
                let op1 = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let op2 = resolve(rec.ops.get(2).copied().unwrap_or(0));
                let (cond_id, t_id, f_id) = if rec.code == 29 {
                    (op2, op0, op1)
                } else {
                    (op0, op1, op2)
                };
                let (cty_s, c_s) = get_val(cond_id, values);
                let (tty_s, t_s) = get_val(t_id, values);
                let (fty_s, f_s) = get_val(f_id, values);
                let ty_idx = values.get(t_id).map(|v| v.ty_idx).unwrap_or(0);
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = select {} {}, {} {}, {} {}",
                        res_ph, cty_s, c_s, tty_s, t_s, fty_s, f_s
                    ),
                ));
            }
            13 | 34 => {
                // INVOKE (13, explicit_fnty bit 13) or CALL (34, explicit_fnty bit 15)
                let is_invoke = rec.code == 13;
                let mut pos = 1usize; // skip attr_idx
                let ccinfo = rec.ops.get(pos).copied().unwrap_or(0);
                pos += 1;
                let invoke_bbs = if is_invoke {
                    *block_terminated = true;
                    if pos + 2 > rec.ops.len() {
                        return Err(BitcodeError::new("Truncated INVOKE target block operands"));
                    }
                    let norm_bb = super::to_idx(rec.ops[pos]);
                    let unw_bb = super::to_idx(rec.ops[pos + 1]);
                    pos += 2;
                    Some((norm_bb, unw_bb))
                } else {
                    None
                };
                if (ccinfo & (1 << 17)) != 0 {
                    pos += 1; // fast-math flags
                }
                let fnty_bit = if is_invoke { 1 << 13 } else { 1 << 15 };
                let explicit_fnty = if (ccinfo & fnty_bit) != 0 {
                    let t = rec.ops.get(pos).copied().map(super::to_idx);
                    pos += 1;
                    t
                } else {
                    None
                };
                if pos >= rec.ops.len() {
                    return Err(BitcodeError::new("Truncated CALL/INVOKE callee operand"));
                }
                let callee = resolve(rec.ops[pos]);
                pos += 1;

                let (_, callee_repr) = get_val(callee, values);
                let inferred_fnty = explicit_fnty.or_else(|| {
                    self.functions
                        .iter()
                        .find(|f| f.val_id == callee)
                        .map(|f| f.fn_ty_idx)
                });
                let ret_ty_idx = inferred_fnty
                    .and_then(|ft| self.fn_signature_parts(ft))
                    .map(|(ret, _, _)| ret)
                    .unwrap_or_else(|| {
                        self.types
                            .iter()
                            .position(|t| matches!(t, LlvmType::Void))
                            .unwrap_or(0)
                    });
                let ret_is_void = self.type_is_void(ret_ty_idx);

                let args_slice = rec.ops.get(pos..).unwrap_or(&[]);
                let rendered_args: Vec<String> = args_slice
                    .iter()
                    .map(|&r| {
                        let (aty_s, a_s) = get_val(resolve(r), values);
                        format!("{} {}", aty_s, a_s)
                    })
                    .collect();

                let ret_s = self.format_type(ret_ty_idx);
                let tail_prefix = if !is_invoke && (ccinfo & 1) != 0 {
                    "tail "
                } else {
                    ""
                };
                let prefix = if ret_is_void {
                    String::new()
                } else {
                    format!("{} = ", alloc_local(ret_ty_idx, values, next_local_num))
                };
                let line = if let Some((norm_bb, unw_bb)) = invoke_bbs {
                    format!(
                        "{}invoke {} {}({}) to label %{} unwind label %{}",
                        prefix,
                        ret_s,
                        callee_repr,
                        rendered_args.join(", "),
                        bb_placeholder(norm_bb),
                        bb_placeholder(unw_bb)
                    )
                } else {
                    format!(
                        "{}{}call {} {}({})",
                        prefix,
                        tail_prefix,
                        ret_s,
                        callee_repr,
                        rendered_args.join(", ")
                    )
                };
                instructions.push((*cur_bb, line));
            }
            4 | 30 | 43 => {
                let ptr_ty = self.ensure_opaque_ptr_ty();
                let base_idx = if rec.code == 43 { 2 } else { 0 };
                let (pty_s, p_s) =
                    get_val(resolve(rec.ops.get(base_idx).copied().unwrap_or(0)), values);
                let elem_s = if rec.code == 43 {
                    self.format_type(super::to_idx(rec.ops.get(1).copied().unwrap_or(0)))
                } else {
                    "i8".to_string()
                };
                let idxs: Vec<String> = rec
                    .ops
                    .iter()
                    .skip(base_idx + 1)
                    .map(|&r| {
                        let (ts, vs) = get_val(resolve(r), values);
                        format!("{} {}", ts, vs)
                    })
                    .collect();
                let res_ph = alloc_local(ptr_ty, values, next_local_num);
                let gep_line = if idxs.is_empty() {
                    format!("{} = getelementptr {}, {} {}", res_ph, elem_s, pty_s, p_s)
                } else {
                    format!(
                        "{} = getelementptr {}, {} {}, {}",
                        res_ph,
                        elem_s,
                        pty_s,
                        p_s,
                        idxs.join(", ")
                    )
                };
                instructions.push((*cur_bb, gep_line));
            }
            6 => {
                // FUNC_CODE_INST_EXTRACTELEMENT: [vec, idx]
                let vec_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let idx_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let elem_ty = match values.get(vec_op).and_then(|v| self.types.get(v.ty_idx)) {
                    Some(LlvmType::Vector { elem, .. }) => *elem,
                    _ => 0,
                };
                let (vty_s, v_s) = get_val(vec_op, values);
                let (ity_s, i_s) = get_val(idx_op, values);
                let res_ph = alloc_local(elem_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = extractelement {} {}, {} {}",
                        res_ph, vty_s, v_s, ity_s, i_s
                    ),
                ));
            }
            7 => {
                // FUNC_CODE_INST_INSERTELEMENT: [vec, elt, idx]
                let vec_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let elt_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let idx_op = resolve(rec.ops.get(2).copied().unwrap_or(0));
                let vec_ty = values.get(vec_op).map(|v| v.ty_idx).unwrap_or(0);
                let (vty_s, v_s) = get_val(vec_op, values);
                let (ety_s, e_s) = get_val(elt_op, values);
                let (ity_s, i_s) = get_val(idx_op, values);
                let res_ph = alloc_local(vec_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = insertelement {} {}, {} {}, {} {}",
                        res_ph, vty_s, v_s, ety_s, e_s, ity_s, i_s
                    ),
                ));
            }
            8 => {
                // FUNC_CODE_INST_SHUFFLEVECTOR: [v1, v2, mask]
                let v1_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let v2_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let m_op = resolve(rec.ops.get(2).copied().unwrap_or(0));
                let (elem_ty, v1_scalable) =
                    match values.get(v1_op).and_then(|v| self.types.get(v.ty_idx)) {
                        Some(LlvmType::Vector { elem, scalable, .. }) => (*elem, *scalable),
                        _ => (0, false),
                    };
                let (mask_len, mask_scalable) =
                    match values.get(m_op).and_then(|v| self.types.get(v.ty_idx)) {
                        Some(LlvmType::Vector { len, scalable, .. }) => (*len, *scalable),
                        _ => (4, v1_scalable),
                    };
                let (v1_ty_s, v1_s) = get_val(v1_op, values);
                let (v2_ty_s, v2_s) = get_val(v2_op, values);
                let (m_ty_s, m_s) = get_val(m_op, values);
                let res_ty = self.ensure_vec_ty(mask_len, elem_ty, mask_scalable);
                let res_ph = alloc_local(res_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = shufflevector {} {}, {} {}, {} {}",
                        res_ph, v1_ty_s, v1_s, v2_ty_s, v2_s, m_ty_s, m_s
                    ),
                ));
            }
            26 => {
                // FUNC_CODE_INST_EXTRACTVAL: [agg, idx0, ...]
                let agg_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let agg_ty = values.get(agg_op).map(|v| v.ty_idx).unwrap_or(0);
                let idxs = rec.ops.get(1..).unwrap_or(&[]);
                let field_ty = self.extract_indexed_type(agg_ty, idxs);
                let (aty_s, a_s) = get_val(agg_op, values);
                let res_ph = alloc_local(field_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = extractvalue {} {}, {}",
                        res_ph,
                        aty_s,
                        a_s,
                        join_agg_indices(idxs)
                    ),
                ));
            }
            27 => {
                // FUNC_CODE_INST_INSERTVAL: [agg, elt, idx0, ...]
                let agg_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let elt_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let agg_ty = values.get(agg_op).map(|v| v.ty_idx).unwrap_or(0);
                let idxs = rec.ops.get(2..).unwrap_or(&[]);
                let (aty_s, a_s) = get_val(agg_op, values);
                let (ety_s, e_s) = get_val(elt_op, values);
                let res_ph = alloc_local(agg_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = insertvalue {} {}, {} {}, {}",
                        res_ph,
                        aty_s,
                        a_s,
                        ety_s,
                        e_s,
                        join_agg_indices(idxs)
                    ),
                ));
            }
            36 => {
                // FUNC_CODE_INST_FENCE
                let ordering = atomic_ordering_name(rec.ops.first().copied().unwrap_or(6));
                instructions.push((*cur_bb, format!("fence {}", ordering)));
            }
            42 | 45 => {
                // STOREATOMIC_OLD (42), STOREATOMIC (45) — void instructions
            }
            46 => {
                // FUNC_CODE_INST_CMPXCHG
                let ptr_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let cmp_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let new_op = resolve(rec.ops.get(2).copied().unwrap_or(0));
                let succ = atomic_ordering_name(rec.ops.get(4).copied().unwrap_or(6));
                let fail = match rec.ops.get(6).copied().unwrap_or(2) {
                    3 => "acquire",
                    6 => "seq_cst",
                    _ => "monotonic",
                };
                let (pty_s, p_s) = get_val(ptr_op, values);
                let (cty_s, c_s) = get_val(cmp_op, values);
                let (nty_s, n_s) = get_val(new_op, values);
                let val_ty = values.get(cmp_op).map(|v| v.ty_idx).unwrap_or(0);
                let i1_ty = self.ensure_i1_ty();
                let pair_ty = self.ensure_anon_struct_ty(vec![val_ty, i1_ty]);
                let res_ph = alloc_local(pair_ty, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = cmpxchg {} {}, {} {}, {} {} {} {}",
                        res_ph, pty_s, p_s, cty_s, c_s, nty_s, n_s, succ, fail
                    ),
                ));
            }
            47 => {
                // FUNC_CODE_INST_LANDINGPAD
                let ty_idx = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                let ty_s = self.format_type(ty_idx);
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((*cur_bb, format!("{} = landingpad {} cleanup", res_ph, ty_s)));
            }
            56 => {
                // FUNC_CODE_INST_UNOP (fneg)
                let src = resolve(rec.ops.first().copied().unwrap_or(0));
                let ty_idx = values.get(src).map(|v| v.ty_idx).unwrap_or(0);
                let (ty_s, val_s) = get_val(src, values);
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((*cur_bb, format!("{} = fneg {} {}", res_ph, ty_s, val_s)));
            }
            58 => {
                // FUNC_CODE_INST_FREEZE
                let src = resolve(rec.ops.first().copied().unwrap_or(0));
                let ty_idx = values.get(src).map(|v| v.ty_idx).unwrap_or(0);
                let (ty_s, val_s) = get_val(src, values);
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((*cur_bb, format!("{} = freeze {} {}", res_ph, ty_s, val_s)));
            }
            59 => {
                // FUNC_CODE_INST_ATOMICRMW
                let ptr_op = resolve(rec.ops.first().copied().unwrap_or(0));
                let val_op = resolve(rec.ops.get(1).copied().unwrap_or(0));
                let rmw_op = atomic_rmw_op_name(rec.ops.get(2).copied().unwrap_or(0));
                let ordering = atomic_ordering_name(rec.ops.get(4).copied().unwrap_or(6));
                let (pty_s, p_s) = get_val(ptr_op, values);
                let (vty_s, v_s) = get_val(val_op, values);
                let ty_idx = values.get(val_op).map(|v| v.ty_idx).unwrap_or(0);
                let res_ph = alloc_local(ty_idx, values, next_local_num);
                instructions.push((
                    *cur_bb,
                    format!(
                        "{} = atomicrmw {} {} {}, {} {} {}",
                        res_ph, rmw_op, pty_s, p_s, vty_s, v_s, ordering
                    ),
                ));
            }
            57 => {
                *block_terminated = true;
                let ty_idx = super::to_idx(rec.ops.first().copied().unwrap_or(0));
                if !self.type_is_void(ty_idx) {
                    let _ = alloc_local(ty_idx, values, next_local_num);
                }
            }
            _ => {
                // Other value-producing opcodes (EXTRACTVAL=26, INSERTVAL=27, LANDINGPAD=47, etc.)
                let ty_idx = self.ensure_opaque_ptr_ty();
                let _ = alloc_local(ty_idx, values, next_local_num);
            }
        }
        Ok(())
    }
}

fn val_placeholder(id: usize) -> String {
    format!("__VAL_ID_{}__", id)
}

fn alloc_local(ty_idx: usize, values: &mut Vec<ValueEntry>, next_local_num: &mut usize) -> String {
    let vid = values.len();
    let res_name = format!("%{}", *next_local_num);
    *next_local_num += 1;
    values.push(ValueEntry {
        ty_idx,
        repr: res_name,
    });
    val_placeholder(vid)
}

fn join_agg_indices(idxs: &[u64]) -> String {
    if idxs.is_empty() {
        "0".to_string()
    } else {
        idxs.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn atomic_rmw_op_name(op: u64) -> &'static str {
    match op {
        0 => "xchg",
        1 => "add",
        2 => "sub",
        3 => "and",
        4 => "nand",
        5 => "or",
        6 => "xor",
        7 => "max",
        8 => "min",
        9 => "umax",
        10 => "umin",
        11 => "fadd",
        12 => "fsub",
        13 => "fmax",
        14 => "fmin",
        _ => "xchg",
    }
}

fn atomic_ordering_name(ord: u64) -> &'static str {
    match ord {
        1 => "unordered",
        2 => "monotonic",
        3 => "acquire",
        4 => "release",
        5 => "acq_rel",
        6 => "seq_cst",
        _ => "seq_cst",
    }
}

fn bb_placeholder(bb: usize) -> String {
    format!("__BB_LABEL_{}__", bb)
}

fn binop_name(opc: u64, is_fp: bool) -> &'static str {
    if is_fp {
        match opc {
            0 => "fadd",
            1 => "fsub",
            2 => "fmul",
            3 | 4 => "fdiv",
            5 | 6 => "frem",
            _ => "fadd",
        }
    } else {
        match opc {
            0 => "add",
            1 => "sub",
            2 => "mul",
            3 => "udiv",
            4 => "sdiv",
            5 => "urem",
            6 => "srem",
            7 => "shl",
            8 => "lshr",
            9 => "ashr",
            10 => "and",
            11 => "or",
            12 => "xor",
            _ => "add",
        }
    }
}

fn cmp_predicate_str(pred: u64) -> (&'static str, &'static str) {
    match pred {
        32 => ("icmp", "eq"),
        33 => ("icmp", "ne"),
        34 => ("icmp", "ugt"),
        35 => ("icmp", "uge"),
        36 => ("icmp", "ult"),
        37 => ("icmp", "ule"),
        38 => ("icmp", "sgt"),
        39 => ("icmp", "sge"),
        40 => ("icmp", "slt"),
        41 => ("icmp", "sle"),
        0 => ("fcmp", "false"),
        1 => ("fcmp", "oeq"),
        2 => ("fcmp", "ogt"),
        3 => ("fcmp", "oge"),
        4 => ("fcmp", "olt"),
        5 => ("fcmp", "ole"),
        6 => ("fcmp", "one"),
        7 => ("fcmp", "ord"),
        8 => ("fcmp", "uno"),
        9 => ("fcmp", "ueq"),
        10 => ("fcmp", "ugt"),
        11 => ("fcmp", "uge"),
        12 => ("fcmp", "ult"),
        13 => ("fcmp", "ule"),
        14 => ("fcmp", "une"),
        15 => ("fcmp", "true"),
        _ => ("icmp", "eq"),
    }
}
