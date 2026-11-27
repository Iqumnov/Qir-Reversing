//! LLVM textual IR emitter for decoded bitcode modules.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use super::types_meta::{
    escape_llvm_str, format_ident, AttrEntry, FunctionDecl, GlobalVarDecl, LlvmType, MetadataItem,
    ModuleDecoder,
};

impl ModuleDecoder {
    fn format_attr_group(&self, grpid: u64) -> Option<String> {
        let g = self.attr_groups.get(&grpid)?;
        let parts: Vec<String> = g
            .entries
            .iter()
            .filter_map(|e| match e {
                AttrEntry::Enum(k) => attr_enum_name(*k).map(str::to_string),
                AttrEntry::EnumInt(86, v) => Some(match *v {
                    0 => "memory(none)".to_string(),
                    1 => "memory(read)".to_string(),
                    2 => "memory(write)".to_string(),
                    _ => "memory(readwrite)".to_string(),
                }),
                AttrEntry::EnumInt(_, _) => None,
                AttrEntry::StrKey(k) => Some(format!("\"{}\"", escape_llvm_str(k))),
                AttrEntry::StrKeyVal(k, v) => Some(format!(
                    "\"{}\"=\"{}\"",
                    escape_llvm_str(k),
                    escape_llvm_str(v)
                )),
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    fn fn_attr_group_ref(&self, paramattr_id: usize) -> Option<u64> {
        if paramattr_id == 0 {
            return None;
        }
        let grp_ids = self.attr_lists.get(paramattr_id - 1)?;
        grp_ids
            .iter()
            .copied()
            .find(|&gid| {
                self.attr_groups
                    .get(&gid)
                    .is_some_and(|g| g.attr_idx == u32::MAX as u64 || g.attr_idx == u64::MAX)
            })
            .or_else(|| grp_ids.first().copied())
    }

    fn global_decl_name(&self, g: &GlobalVarDecl) -> String {
        if !g.name.is_empty() {
            g.name.clone()
        } else if let Some(vname) = self.vst_global_names.get(&g.val_id) {
            vname.clone()
        } else {
            format!("global.{}", g.val_id)
        }
    }

    fn fn_decl_name(&self, f: &FunctionDecl) -> String {
        if !f.name.is_empty() {
            f.name.clone()
        } else if let Some(vname) = self.vst_global_names.get(&f.val_id) {
            vname.clone()
        } else {
            format!("fn.{}", f.val_id)
        }
    }

    fn resolve_mod_placeholders(&self, s: &str) -> String {
        replace_indexed_placeholders(s, "__MOD_VAL_", |vid| {
            if let Some(n) = self.vst_global_names.get(&vid) {
                format!("@{}", format_ident(n))
            } else if let Some(g) = self.globals.iter().find(|g| g.val_id == vid) {
                format!("@{}", format_ident(&self.global_decl_name(g)))
            } else if let Some(f) = self.functions.iter().find(|f| f.val_id == vid) {
                format!("@{}", format_ident(&self.fn_decl_name(f)))
            } else {
                format!("@val.{}", vid)
            }
        })
    }

    pub(crate) fn render_ir(&self) -> String {
        let mut out = String::with_capacity(2048);
        if let Some(src) = &self.source_filename {
            if !src.is_empty() {
                let _ = writeln!(out, "; ModuleID = '{}'", src);
                let _ = writeln!(out, "source_filename = \"{}\"", escape_llvm_str(src));
            }
        }
        if let Some(dl) = &self.target_datalayout {
            let _ = writeln!(out, "target datalayout = \"{}\"", escape_llvm_str(dl));
        }
        if let Some(triple) = &self.target_triple {
            let _ = writeln!(out, "target triple = \"{}\"", escape_llvm_str(triple));
        }
        if !out.is_empty() {
            out.push('\n');
        }

        for ty in &self.types {
            if let LlvmType::NamedStruct {
                name,
                is_packed,
                fields,
            } = ty
            {
                if fields.is_empty() {
                    let _ = writeln!(out, "%{} = type opaque", format_ident(name));
                } else {
                    let fs: Vec<String> = fields.iter().map(|&f| self.format_type(f)).collect();
                    let body = if *is_packed {
                        format!("<{{ {} }}>", fs.join(", "))
                    } else {
                        format!("{{ {} }}", fs.join(", "))
                    };
                    let _ = writeln!(out, "%{} = type {}", format_ident(name), body);
                }
            }
        }

        for g in &self.globals {
            let kw = if g.is_const { "constant" } else { "global" };
            let linkage = linkage_keyword(g.linkage);
            let ty_s = self.format_type(g.val_ty_idx);
            let init_raw = if g.init_id > 0 {
                self.module_values
                    .get(g.init_id - 1)
                    .map(|v| v.repr.clone())
                    .unwrap_or_else(|| self.zero_value_for_type(g.val_ty_idx))
            } else {
                self.zero_value_for_type(g.val_ty_idx)
            };
            let init_s = self.resolve_mod_placeholders(&init_raw);
            let _ = writeln!(
                out,
                "@{} = {}{} {} {}",
                format_ident(&self.global_decl_name(g)),
                linkage,
                kw,
                ty_s,
                init_s
            );
        }
        if !self.globals.is_empty() {
            out.push('\n');
        }

        let mut attr_map: HashMap<u64, usize> = HashMap::new();
        let mut ordered_groups: Vec<(usize, String)> = Vec::new();
        for f in &self.functions {
            if let Some(gid) = self.fn_attr_group_ref(f.paramattr_id) {
                if let std::collections::hash_map::Entry::Vacant(e) = attr_map.entry(gid) {
                    if let Some(text) = self.format_attr_group(gid) {
                        let idx = ordered_groups.len();
                        e.insert(idx);
                        ordered_groups.push((idx, text));
                    }
                }
            }
        }

        for is_def in [true, false] {
            for (func_idx, f) in self.functions.iter().enumerate() {
                if f.is_proto == is_def {
                    continue;
                }
                let raw_name = self.fn_decl_name(f);
                let (ret_ty, params, is_vararg) =
                    self.fn_signature_parts(f.fn_ty_idx)
                        .unwrap_or((0, &[], false));
                let ret_s = self.format_type(ret_ty);
                let body_opt = self.bodies.iter().find(|b| b.func_idx == func_idx);
                let mut param_strs: Vec<String> = params
                    .iter()
                    .enumerate()
                    .map(|(pi, &pty)| {
                        if is_def {
                            let pname = body_opt
                                .and_then(|b| b.param_names.get(pi))
                                .cloned()
                                .unwrap_or_else(|| format!("%{}", pi));
                            format!("{} {}", self.format_type(pty), pname)
                        } else {
                            self.format_type(pty)
                        }
                    })
                    .collect();
                if is_vararg {
                    param_strs.push("...".to_string());
                }
                let attr_suffix = self
                    .fn_attr_group_ref(f.paramattr_id)
                    .and_then(|gid| attr_map.get(&gid))
                    .map(|idx| format!(" #{}", idx))
                    .unwrap_or_default();

                if is_def {
                    let linkage = linkage_keyword(f.linkage);
                    let personality_suffix = f
                        .personality_val_id
                        .map(|vid| {
                            self.resolve_mod_placeholders(&format!(
                                " personality ptr {}",
                                super::types_meta::mod_val_placeholder(vid)
                            ))
                        })
                        .unwrap_or_default();
                    let _ = writeln!(
                        out,
                        "define {}{} @{}({}){}{} {{",
                        linkage,
                        ret_s,
                        format_ident(&raw_name),
                        param_strs.join(", "),
                        attr_suffix,
                        personality_suffix
                    );
                    let empty_body = if ret_s == "void" {
                        "entry:\n  ret void\n"
                    } else {
                        "entry:\n  unreachable\n"
                    };
                    if let Some(body) = body_opt {
                        let mut grouped: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
                        let decl_bbs = body.num_blocks.min(1024);
                        for bb in 0..decl_bbs {
                            grouped.entry(bb).or_default();
                        }
                        for &bb in body.bb_names.keys().take(1024) {
                            grouped.entry(bb).or_default();
                        }
                        for (inst_bb, line) in &body.instructions {
                            grouped.entry(*inst_bb).or_default().push(line.as_str());
                        }
                        if grouped.is_empty() {
                            out.push_str(empty_body);
                        } else {
                            for (bb, lines) in grouped {
                                let label = body
                                    .bb_names
                                    .get(&bb)
                                    .cloned()
                                    .unwrap_or_else(|| format!("bb{}", bb));
                                let _ = writeln!(out, "{}:", format_ident(&label));
                                for line in lines {
                                    let with_bb = replace_bb_placeholders(line, &body.bb_names);
                                    let resolved = self.resolve_mod_placeholders(&with_bb);
                                    let _ = writeln!(out, "  {}", resolved);
                                }
                            }
                        }
                    } else {
                        out.push_str(empty_body);
                    }
                    out.push_str("}\n\n");
                } else {
                    let _ = writeln!(
                        out,
                        "declare {} @{}({}){}\n",
                        ret_s,
                        format_ident(&raw_name),
                        param_strs.join(", "),
                        attr_suffix
                    );
                }
            }
        }

        for (idx, text) in ordered_groups {
            let _ = writeln!(out, "attributes #{} = {{ {} }}", idx, text);
        }

        if !self.named_metadata.is_empty() {
            out.push('\n');
            for (name, ids) in &self.named_metadata {
                let refs: Vec<String> = ids.iter().map(|id| format!("!{}", id)).collect();
                let _ = writeln!(out, "!{} = !{{{}}}", format_ident(name), refs.join(", "));
            }
            out.push('\n');
            for (idx, item) in self.metadata.iter().enumerate() {
                if let MetadataItem::Node(refs) | MetadataItem::DistinctNode(refs) = item {
                    let prefix = if matches!(item, MetadataItem::DistinctNode(_)) {
                        "distinct "
                    } else {
                        ""
                    };
                    let ops: Vec<String> = refs
                        .iter()
                        .map(|r| match r {
                            None => "null".to_string(),
                            Some(ri) => self.format_md_operand(*ri),
                        })
                        .collect();
                    let _ = writeln!(out, "!{} = {}!{{{}}}", idx, prefix, ops.join(", "));
                }
            }
        }

        out
    }

    fn format_md_operand(&self, idx: usize) -> String {
        match self.metadata.get(idx) {
            Some(MetadataItem::Str(s)) => format!("!\"{}\"", escape_llvm_str(s)),
            Some(MetadataItem::Value { ty_idx, val_repr }) => {
                format!(
                    "{} {}",
                    self.format_type(*ty_idx),
                    self.resolve_mod_placeholders(val_repr)
                )
            }
            Some(MetadataItem::Node(_) | MetadataItem::DistinctNode(_)) => format!("!{}", idx),
            _ => "null".to_string(),
        }
    }
}

pub(crate) fn replace_indexed_placeholders<F>(s: &str, prefix: &str, resolve: F) -> String
where
    F: Fn(usize) -> String,
{
    let mut out = String::with_capacity(s.len());
    let mut cursor = 0usize;
    while let Some(rel_start) = s[cursor..].find(prefix) {
        let start = cursor + rel_start;
        out.push_str(&s[cursor..start]);
        let after_prefix = start + prefix.len();
        if let Some(end_rel) = s[after_prefix..].find("__") {
            if let Ok(idx) = s[after_prefix..after_prefix + end_rel].parse::<usize>() {
                out.push_str(&resolve(idx));
                cursor = after_prefix + end_rel + 2;
                continue;
            }
        }
        out.push_str(prefix);
        cursor = after_prefix;
    }
    out.push_str(&s[cursor..]);
    out
}

fn replace_bb_placeholders(line: &str, bb_names: &HashMap<usize, String>) -> String {
    replace_indexed_placeholders(line, "__BB_LABEL_", |bb_idx| {
        let name = bb_names
            .get(&bb_idx)
            .cloned()
            .unwrap_or_else(|| format!("bb{}", bb_idx));
        format_ident(&name)
    })
}

fn linkage_keyword(code: u64) -> &'static str {
    match code {
        3 => "internal ",
        9 => "private ",
        _ => "",
    }
}

fn attr_enum_name(kind: u64) -> Option<&'static str> {
    match kind {
        52 => Some("writeonly"),
        53 => Some("readonly"),
        54 => Some("readnone"),
        65 => Some("memory(none)"),
        _ => None,
    }
}
