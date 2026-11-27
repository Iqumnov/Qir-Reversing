//! Zero-Copy & High-Precision LLVM QIR Parser
//! Author: Matvei Igumnov <matvei@iqumnov.com>

use crate::types::{QuantumGate, QuantumInstruction, Qubit, ResultBit};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("I/O error while reading quantum bytecode: {0}")]
    IoError(String),
    #[error("Malformed LLVM IR syntax: {0}")]
    MalformedSyntax(String),
    #[error("Invalid IEEE-754 hex float literal: {0}")]
    InvalidFloatLiteral(String),
    #[error("Unknown or unsupported QIR intrinsic: {0}")]
    UnsupportedIntrinsic(String),
    #[error("Binary LLVM Bitcode (.bc) decode failed: {0}")]
    BitcodeDecompilerUnavailable(String),
}

/// Metadata extracted from QIR LLVM module headers.
#[derive(Debug, Clone, Default)]
pub struct QirModuleMetadata {
    pub num_qubits: u32,
    pub num_results: u32,
    pub entry_point: String,
    pub qir_profiles: String,
}

/// LLVM QIR text and bitcode parser.
pub struct QirParser {
    pub metadata: QirModuleMetadata,
    var_qubits: HashMap<String, u32>,
    var_results: HashMap<String, u32>,
    var_opaque_ptrs: HashMap<String, u32>,
}

impl Default for QirParser {
    fn default() -> Self {
        Self::new()
    }
}

impl QirParser {
    pub fn new() -> Self {
        Self {
            metadata: QirModuleMetadata::default(),
            var_qubits: HashMap::new(),
            var_results: HashMap::new(),
            var_opaque_ptrs: HashMap::new(),
        }
    }

    /// Parse file automatically distinguishing LLVM Bitcode (.bc) and Textual IR (.ll).
    pub fn parse_file(
        &mut self,
        path: &std::path::Path,
    ) -> Result<Vec<QuantumInstruction>, ParserError> {
        let bytes = std::fs::read(path).map_err(|e| ParserError::IoError(e.to_string()))?;
        self.parse_bytes(&bytes, path)
    }

    /// Parse raw bytes, handling both binary LLVM bitcode (`.bc` / `0x0B17C0DE` wrapper) and UTF-8 IR text.
    pub fn parse_bytes(
        &mut self,
        bytes: &[u8],
        path_hint: &std::path::Path,
    ) -> Result<Vec<QuantumInstruction>, ParserError> {
        // Primary decoder: pure-Rust, zero-unsafe, WASM-compatible LLVM bitcode reader.
        if crate::bitcode::is_llvm_bitcode(bytes) {
            match crate::bitcode::decode_bitcode_to_ir(bytes) {
                Ok(ir_text) => return self.parse_ir(&ir_text),
                Err(bc_err) => {
                    // For Darwin/object-wrapped streams or wasm32 (no external CLI), fail directly with the decoder error.
                    let is_raw_bc =
                        bytes.len() >= 4 && bytes[..4] == crate::bitcode::LLVM_BITCODE_MAGIC;
                    if cfg!(target_arch = "wasm32") || !is_raw_bc {
                        return Err(ParserError::BitcodeDecompilerUnavailable(
                            bc_err.to_string(),
                        ));
                    }
                }
            }
        }

        // Fallback Strategy A (Native only): external `llvm-dis` or `pyqir` for unsupported raw bitcode variants
        if bytes.len() >= 4
            && bytes[0] == 0x42
            && bytes[1] == 0x43
            && bytes[2] == 0xc0
            && bytes[3] == 0xde
        {
            // Strategy A: Native system decompiler on the exact input bytes (`llvm-dis` or `pyqir`)
            #[cfg(not(target_arch = "wasm32"))]
            {
                use std::io::Write;
                if let Ok(mut child) = std::process::Command::new("llvm-dis")
                    .args(["-o", "-", "-"])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(bytes);
                    }
                    if let Ok(output) = child.wait_with_output() {
                        if output.status.success() {
                            if let Ok(ir_text) = String::from_utf8(output.stdout) {
                                return self.parse_ir(&ir_text);
                            }
                        }
                    }
                }

                let mut python_candidates = Vec::new();
                if let Ok(env_py) = std::env::var("PYTHON") {
                    if !env_py.trim().is_empty() {
                        python_candidates.push(env_py);
                    }
                }
                for cand in ["python3", "python"] {
                    if !python_candidates.iter().any(|s| s == cand) {
                        python_candidates.push(cand.to_string());
                    }
                }

                for py in &python_candidates {
                    if let Ok(mut child) = std::process::Command::new(py)
                        .args([
                            "-c",
                            "import sys, pyqir; ctx = pyqir.Context(); mod = pyqir.Module.from_bitcode(ctx, sys.stdin.buffer.read()); print(str(mod))"
                        ])
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(bytes);
                        }
                        if let Ok(output) = child.wait_with_output() {
                            if output.status.success() {
                                if let Ok(decompiled) = String::from_utf8(output.stdout) {
                                    return self.parse_ir(&decompiled);
                                }
                            }
                        }
                    }
                }
            }

            // Strategy B: Companion .ll source file fallback when no bitcode decompiler is installed
            let ll_candidate = path_hint.with_extension("ll");
            if ll_candidate.exists() && ll_candidate != path_hint {
                if let Ok(ll_content) = std::fs::read_to_string(&ll_candidate) {
                    return self.parse_ir(&ll_content);
                }
            }

            return Err(ParserError::BitcodeDecompilerUnavailable(format!(
                "Bitcode container '{}' requires a companion .ll file or system 'llvm-dis' / 'pyqir' installed.",
                path_hint.display()
            )));
        }

        // 2. Standard UTF-8 textual IR
        let content =
            std::str::from_utf8(bytes).map_err(|e| ParserError::MalformedSyntax(e.to_string()))?;
        self.parse_ir(content)
    }

    /// Parse LLVM textual IR string into an ordered stream of QuantumInstructions.
    pub fn parse_ir(&mut self, content: &str) -> Result<Vec<QuantumInstruction>, ParserError> {
        self.metadata = QirModuleMetadata::default();
        self.var_qubits.clear();
        self.var_results.clear();
        self.var_opaque_ptrs.clear();

        let mut instructions = Vec::new();
        let mut current_address: u64 = 0;
        let mut next_alloc_q: u32 = 0;
        let mut max_alloc_q: u32 = 0;
        let mut has_emitted_in_fn = false;

        self.extract_module_metadata(content);

        for line in content.lines() {
            // Strip inline LLVM comments outside strings so commented calls/parens never taint IR parsing
            let code_line = strip_llvm_comment(line);
            let trimmed = code_line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("define ") {
                if has_emitted_in_fn {
                    let mut cf = QuantumInstruction::new(QuantumGate::Barrier(Vec::new()), trimmed);
                    cf.is_control_flow = true;
                    cf.address = current_address;
                    instructions.push(cf);
                    has_emitted_in_fn = false;
                }
                self.var_qubits.clear();
                self.var_results.clear();
                self.var_opaque_ptrs.clear();
                max_alloc_q = max_alloc_q.max(next_alloc_q);
                next_alloc_q = 0;
                continue;
            }

            // Detect branch / multi-block basic-block labels as control-flow fences
            if trimmed.ends_with(':') && !trimmed.contains(' ') {
                if has_emitted_in_fn {
                    let mut cf = QuantumInstruction::new(QuantumGate::Barrier(Vec::new()), trimmed);
                    cf.is_control_flow = true;
                    cf.address = current_address;
                    instructions.push(cf);
                }
                continue;
            }

            if trimmed.starts_with("br ") || trimmed.starts_with("switch ") {
                let mut cf = QuantumInstruction::new(QuantumGate::Barrier(Vec::new()), trimmed);
                cf.is_control_flow = true;
                cf.address = current_address;
                instructions.push(cf);
                continue;
            }

            let is_call_line = is_llvm_call_instruction(trimmed);

            // Track standalone inttoptr / bitcast / addrspacecast / getelementptr SSA bindings
            // (Never swallow `%r = call ...` lines that happen to take inline `inttoptr (...)` args!)
            if !is_call_line && trimmed.starts_with('%') && trimmed.contains("inttoptr") {
                self.record_pointer_binding(trimmed);
                continue;
            }
            if !is_call_line
                && trimmed.starts_with('%')
                && (trimmed.contains("= bitcast ")
                    || trimmed.contains("= addrspacecast ")
                    || trimmed.contains("= getelementptr "))
            {
                self.record_alias_binding(trimmed);
                continue;
            }

            if !is_call_line {
                continue;
            }

            // Track runtime dynamic qubit allocations (@__quantum__rt__qubit_allocate / allocate_array)
            if trimmed.contains("@__quantum__rt__qubit_allocate_array") {
                if let Some(count) = extract_typed_int_index(
                    trimmed
                        .split("@__quantum__rt__qubit_allocate_array")
                        .nth(1)
                        .unwrap_or(""),
                ) {
                    next_alloc_q = next_alloc_q.saturating_add(count);
                }
                continue;
            }

            if trimmed.contains("@__quantum__rt__qubit_allocate") {
                if let Some(lhs) = trimmed.split('=').next() {
                    let lhs_tok = lhs.trim();
                    if lhs_tok.starts_with('%') {
                        self.var_qubits.insert(lhs_tok.to_string(), next_alloc_q);
                        next_alloc_q = next_alloc_q.saturating_add(1);
                    }
                }
                continue;
            }

            // Parse quantum intrinsic calls (handling standard call, tail call, invoke, and assignment)
            if trimmed.contains("@__quantum__qis__") {
                if let Some(gate) = self.parse_intrinsic_call(trimmed)? {
                    let mut inst = QuantumInstruction::new(gate, trimmed);
                    inst.address = current_address;
                    instructions.push(inst);
                    current_address += 4;
                    has_emitted_in_fn = true;
                }
            }
        }

        // Post-validation: auto-detect qubit & result counts if metadata attributes were omitted
        let max_q = instructions
            .iter()
            .flat_map(|inst| inst.gate.active_qubits())
            .map(|q| q.0.saturating_add(1))
            .max()
            .unwrap_or(0)
            .max(max_alloc_q.max(next_alloc_q));

        if self.metadata.num_qubits < max_q {
            self.metadata.num_qubits = max_q;
        }

        let max_r = instructions
            .iter()
            .filter_map(|inst| match &inst.gate {
                QuantumGate::Mz(_, r) => Some(r.0.saturating_add(1)),
                QuantumGate::Custom { results, .. } if !results.is_empty() => {
                    results.iter().map(|r| r.0.saturating_add(1)).max()
                }
                _ => None,
            })
            .max()
            .unwrap_or(0);
        if self.metadata.num_results < max_r {
            self.metadata.num_results = max_r;
        }

        Ok(instructions)
    }

    fn extract_module_metadata(&mut self, content: &str) {
        let mut defined_entry: Option<String> = None;
        let mut fallback_define: Option<String> = None;
        let mut entry_attrs: Vec<String> = Vec::new();
        let mut attr_bodies: HashMap<String, String> = HashMap::new();

        for line in content.lines() {
            let trimmed = strip_llvm_comment(line).trim();
            if trimmed.starts_with("attributes ") {
                if let Some(eq_pos) = trimmed.find('=') {
                    let attr_id = trimmed[..eq_pos]
                        .trim()
                        .trim_start_matches("attributes ")
                        .trim();
                    let body = trimmed[eq_pos + 1..].trim();
                    if !attr_id.is_empty() {
                        attr_bodies.insert(attr_id.to_string(), body.to_string());
                        if trimmed.contains("\"entry_point\"") || trimmed.contains("\"EntryPoint\"")
                        {
                            entry_attrs.push(attr_id.to_string());
                        }
                    }
                }
            }
        }

        let mut chosen_attr_id: Option<String> = None;
        for line in content.lines() {
            let trimmed = strip_llvm_comment(line).trim();
            if trimmed.starts_with("define ") {
                if let Some(at_pos) = trimmed.find('@') {
                    let after_at = &trimmed[at_pos + 1..];
                    let end = after_at.find('(').unwrap_or(after_at.len());
                    let fn_name = after_at[..end].trim().trim_matches('"');
                    if !fn_name.is_empty() {
                        if fallback_define.is_none() {
                            fallback_define = Some(fn_name.to_string());
                        }
                        if let Some(matched_attr) = entry_attrs.iter().find(|attr| {
                            trimmed
                                .split_whitespace()
                                .any(|tok| tok.trim_end_matches('{') == attr.as_str())
                        }) {
                            defined_entry = Some(fn_name.to_string());
                            chosen_attr_id = Some(matched_attr.clone());
                        }
                    }
                }
            }
        }

        let attr_scope = chosen_attr_id
            .as_ref()
            .and_then(|id| attr_bodies.get(id))
            .map(|s| s.as_str());

        if let Some(scope) = attr_scope {
            if let Some(val) = extract_metadata_int(scope, "required_num_qubits") {
                self.metadata.num_qubits = val;
            }
            if let Some(val) = extract_metadata_int(scope, "required_num_results") {
                self.metadata.num_results = val;
            }
            if let Some(prof) = extract_metadata_string(scope, "\"qir_profiles\"") {
                self.metadata.qir_profiles = prof;
            }
        }

        for line in content.lines() {
            let trimmed = strip_llvm_comment(line).trim();
            if self.metadata.num_qubits == 0 && trimmed.contains("required_num_qubits") {
                if let Some(val) = extract_metadata_int(trimmed, "required_num_qubits") {
                    self.metadata.num_qubits = val;
                }
            }
            if self.metadata.num_results == 0 && trimmed.contains("required_num_results") {
                if let Some(val) = extract_metadata_int(trimmed, "required_num_results") {
                    self.metadata.num_results = val;
                }
            }
            if let Some(pos) = trimmed.find("\"entry_point\"") {
                let after = trimmed[pos + "\"entry_point\"".len()..].trim_start();
                if after.starts_with('=') {
                    if let Some(name) = extract_metadata_string(trimmed, "\"entry_point\"") {
                        self.metadata.entry_point = name;
                    }
                }
            }
            if self.metadata.qir_profiles.is_empty() && trimmed.contains("\"qir_profiles\"") {
                if let Some(prof) = extract_metadata_string(trimmed, "\"qir_profiles\"") {
                    self.metadata.qir_profiles = prof;
                }
            }
        }
        if self.metadata.entry_point.is_empty() {
            if let Some(name) = defined_entry.or(fallback_define) {
                self.metadata.entry_point = name;
            }
        }
    }

    fn record_pointer_binding(&mut self, line: &str) {
        // Examples: %1 = inttoptr i64 1 to %Qubit*, %1 = inttoptr i64 1 to ptr
        let Some((lhs, expr)) = line.split_once('=') else {
            return;
        };
        let var_name = lhs.trim();
        let expr = expr.trim();
        if !var_name.starts_with('%') {
            return;
        }

        if let Some(idx) = extract_inttoptr_index(expr) {
            let ty_part = expr
                .split_once(" to ")
                .map(|(_, after_to)| after_to)
                .unwrap_or(expr);
            if ty_part.contains("%Result") {
                self.var_results.insert(var_name.to_string(), idx);
            } else if ty_part.contains("%Qubit") {
                self.var_qubits.insert(var_name.to_string(), idx);
            } else if ty_part.contains("ptr") || ty_part.contains("i8*") {
                self.var_opaque_ptrs.insert(var_name.to_string(), idx);
            }
        }
    }

    fn record_alias_binding(&mut self, line: &str) {
        let Some((lhs, expr)) = line.split_once('=') else {
            return;
        };
        let var_name = lhs.trim();
        if !var_name.starts_with('%') {
            return;
        }
        let mut rest = expr.trim();
        while let Some(pos) = rest.find('%') {
            if let Some(tok) = extract_ssa_token(&rest[pos..]) {
                if let Some(&q) = self.var_qubits.get(tok) {
                    self.var_qubits.insert(var_name.to_string(), q);
                    return;
                }
                if let Some(&r) = self.var_results.get(tok) {
                    self.var_results.insert(var_name.to_string(), r);
                    return;
                }
                if let Some(&p) = self.var_opaque_ptrs.get(tok) {
                    self.var_opaque_ptrs.insert(var_name.to_string(), p);
                    return;
                }
                rest = &rest[pos + tok.len()..];
            } else {
                break;
            }
        }
    }

    fn parse_intrinsic_call(&mut self, line: &str) -> Result<Option<QuantumGate>, ParserError> {
        let at_idx = match line.find("@__quantum__qis__").or_else(|| line.find('@')) {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let after_at = &line[at_idx + 1..];
        let paren_idx = match after_at.find('(') {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let func_name = &after_at[..paren_idx];
        let after_paren = &after_at[paren_idx + 1..];
        let close_idx = find_matching_paren(after_paren).ok_or_else(|| {
            ParserError::MalformedSyntax(format!(
                "Unbalanced parentheses in intrinsic call '{}'",
                line.trim()
            ))
        })?;
        let args_str = &after_paren[..close_idx];

        let args = split_llvm_arguments(args_str);

        // Normalize function name: __quantum__qis__h__body -> h
        let stripped_prefix = func_name
            .strip_prefix("__quantum__qis__")
            .unwrap_or(func_name);
        let canonical_op = stripped_prefix
            .strip_suffix("__body")
            .unwrap_or(stripped_prefix);

        let require_arity = |needed: usize| -> Result<(), ParserError> {
            if args.len() < needed {
                Err(ParserError::MalformedSyntax(format!(
                    "QIR intrinsic '{}' expected at least {} argument(s), got {}: '{}'",
                    func_name,
                    needed,
                    args.len(),
                    line.trim()
                )))
            } else {
                Ok(())
            }
        };

        let gate = match canonical_op {
            "h" | "h__adj" | "h_adj" => {
                require_arity(1)?;
                QuantumGate::H(self.parse_qubit_arg(args[0]))
            }
            "x" | "x__adj" | "x_adj" => {
                require_arity(1)?;
                QuantumGate::X(self.parse_qubit_arg(args[0]))
            }
            "y" | "y__adj" | "y_adj" => {
                require_arity(1)?;
                QuantumGate::Y(self.parse_qubit_arg(args[0]))
            }
            "z" | "z__adj" | "z_adj" => {
                require_arity(1)?;
                QuantumGate::Z(self.parse_qubit_arg(args[0]))
            }
            "s" | "sdg__adj" | "sdg_adj" => {
                require_arity(1)?;
                QuantumGate::S(self.parse_qubit_arg(args[0]))
            }
            "s__adj" | "s_adj" | "sdg" => {
                require_arity(1)?;
                QuantumGate::Sdg(self.parse_qubit_arg(args[0]))
            }
            "t" | "tdg__adj" | "tdg_adj" => {
                require_arity(1)?;
                QuantumGate::T(self.parse_qubit_arg(args[0]))
            }
            "t__adj" | "t_adj" | "tdg" => {
                require_arity(1)?;
                QuantumGate::Tdg(self.parse_qubit_arg(args[0]))
            }
            "cnot" | "cx" | "cnot__adj" | "cnot_adj" | "cx__adj" | "cx_adj" => {
                require_arity(2)?;
                let c = self.parse_qubit_arg(args[0]);
                let t = self.parse_qubit_arg(args[1]);
                QuantumGate::CX(c, t)
            }
            "cz" | "cz__adj" | "cz_adj" => {
                require_arity(2)?;
                let c = self.parse_qubit_arg(args[0]);
                let t = self.parse_qubit_arg(args[1]);
                QuantumGate::CZ(c, t)
            }
            "swap" | "swap__adj" | "swap_adj" => {
                require_arity(2)?;
                let q1 = self.parse_qubit_arg(args[0]);
                let q2 = self.parse_qubit_arg(args[1]);
                QuantumGate::SWAP(q1, q2)
            }
            "ccx" | "toffoli" | "ccx__adj" | "ccx_adj" | "toffoli__adj" | "toffoli_adj" => {
                require_arity(3)?;
                let c1 = self.parse_qubit_arg(args[0]);
                let c2 = self.parse_qubit_arg(args[1]);
                let t = self.parse_qubit_arg(args[2]);
                QuantumGate::CCX(c1, c2, t)
            }
            "rx" => {
                require_arity(2)?;
                let theta = self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Rx(theta, q)
            }
            "rx__adj" | "rx_adj" => {
                require_arity(2)?;
                let theta = -self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Rx(theta, q)
            }
            "ry" => {
                require_arity(2)?;
                let theta = self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Ry(theta, q)
            }
            "ry__adj" | "ry_adj" => {
                require_arity(2)?;
                let theta = -self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Ry(theta, q)
            }
            "rz" => {
                require_arity(2)?;
                let theta = self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Rz(theta, q)
            }
            "rz__adj" | "rz_adj" => {
                require_arity(2)?;
                let theta = -self.parse_float_arg(args[0])?;
                let q = self.parse_qubit_arg(args[1]);
                QuantumGate::Rz(theta, q)
            }
            "mz" | "m" => {
                let (q, r) = self.parse_measure_pair(line, &args, func_name)?;
                QuantumGate::Mz(q, r)
            }
            "mresetz" => {
                let (q, r) = self.parse_measure_pair(line, &args, func_name)?;
                QuantumGate::Custom {
                    name: "mresetz".to_string(),
                    qubits: vec![q],
                    results: vec![r],
                    params: Vec::new(),
                }
            }
            "reset" => {
                require_arity(1)?;
                QuantumGate::Reset(self.parse_qubit_arg(args[0]))
            }
            "barrier" => QuantumGate::Barrier(
                args.iter()
                    .filter(|a| !a.trim().is_empty())
                    .map(|a| self.parse_qubit_arg(a))
                    .collect(),
            ),
            "read_result" => {
                require_arity(1)?;
                let r = self.parse_result_arg(args[0]);
                QuantumGate::Custom {
                    name: "read_result".to_string(),
                    qubits: Vec::new(),
                    results: vec![r],
                    params: Vec::new(),
                }
            }
            other => {
                let mut qubits = Vec::new();
                let mut results = Vec::new();
                let mut params = Vec::new();
                for a in &args {
                    let t = a.trim();
                    if t.starts_with("double ") || t.starts_with("float ") {
                        if let Ok(v) = self.parse_float_arg(t) {
                            params.push(v);
                        }
                    } else if t.contains("%Result") || has_ssa_token_in(t, &self.var_results) {
                        results.push(self.parse_result_arg(t));
                    } else {
                        qubits.push(self.parse_qubit_arg(t));
                    }
                }
                QuantumGate::Custom {
                    name: other.to_string(),
                    qubits,
                    results,
                    params,
                }
            }
        };

        Ok(Some(gate))
    }

    fn parse_measure_pair(
        &mut self,
        line: &str,
        args: &[&str],
        func_name: &str,
    ) -> Result<(Qubit, ResultBit), ParserError> {
        if args.is_empty() {
            return Err(ParserError::MalformedSyntax(format!(
                "QIR intrinsic '{}' expected at least 1 argument(s), got 0: '{}'",
                func_name,
                line.trim()
            )));
        }
        let q = self.parse_qubit_arg(args[0]);
        let r = if args.len() > 1 {
            self.parse_result_arg(args[1])
        } else {
            let mut found = None;
            if let Some(rhs_lhs) = line.split('=').next().filter(|_| line.contains('=')) {
                let mut rest = rhs_lhs;
                while let Some(pos) = rest.find('%') {
                    if let Some(tok) = extract_ssa_token(&rest[pos..]) {
                        if let Some(&idx) = self
                            .var_results
                            .get(tok)
                            .or_else(|| self.var_opaque_ptrs.get(tok))
                        {
                            found = Some(ResultBit(idx));
                            break;
                        }
                        if tok == rhs_lhs.trim() {
                            self.var_results.insert(tok.to_string(), q.0);
                            found = Some(ResultBit(q.0));
                            break;
                        }
                        rest = &rest[pos + tok.len()..];
                    } else {
                        break;
                    }
                }
            }
            found.unwrap_or(ResultBit(q.0))
        };
        Ok((q, r))
    }

    fn parse_qubit_arg(&self, arg: &str) -> Qubit {
        let trimmed = arg.trim();

        // Check for exact null pointer token (`null`, `%Qubit* null`, `ptr null`) — avoid `%null_flag`
        if trimmed == "null" || trimmed.ends_with(" null") {
            return Qubit(0);
        }

        // Check for inline inttoptr: `inttoptr (i64 1 to %Qubit*)` or `ptr inttoptr (i64 1 to ptr)`
        if trimmed.contains("inttoptr ") || trimmed.contains("inttoptr(") {
            if let Some(idx) = extract_inttoptr_index(trimmed) {
                return Qubit(idx);
            }
        }

        // Scan every '%' token so type names like `%Qubit*` never shadow the operand `%var`
        let mut rest = trimmed;
        while let Some(pos) = rest.find('%') {
            if let Some(token) = extract_ssa_token(&rest[pos..]) {
                if let Some(&idx) = self
                    .var_qubits
                    .get(token)
                    .or_else(|| self.var_opaque_ptrs.get(token))
                {
                    return Qubit(idx);
                }
                rest = &rest[pos + token.len()..];
            } else {
                break;
            }
        }

        // Fallback: extract explicit typed integer or pure numeric SSA token (%2)
        if let Some(idx) = extract_typed_int_index(trimmed) {
            return Qubit(idx);
        }
        if let Some(idx) = extract_numeric_ssa_index(trimmed) {
            return Qubit(idx);
        }

        Qubit(0)
    }

    fn parse_result_arg(&self, arg: &str) -> ResultBit {
        let trimmed = arg.trim();

        if trimmed == "null" || trimmed.ends_with(" null") {
            return ResultBit(0);
        }

        if trimmed.contains("inttoptr ") || trimmed.contains("inttoptr(") {
            if let Some(idx) = extract_inttoptr_index(trimmed) {
                return ResultBit(idx);
            }
        }

        let mut rest = trimmed;
        while let Some(pos) = rest.find('%') {
            if let Some(token) = extract_ssa_token(&rest[pos..]) {
                if let Some(&idx) = self
                    .var_results
                    .get(token)
                    .or_else(|| self.var_opaque_ptrs.get(token))
                {
                    return ResultBit(idx);
                }
                rest = &rest[pos + token.len()..];
            } else {
                break;
            }
        }

        if let Some(idx) = extract_typed_int_index(trimmed) {
            return ResultBit(idx);
        }
        if let Some(idx) = extract_numeric_ssa_index(trimmed) {
            return ResultBit(idx);
        }

        ResultBit(0)
    }

    fn parse_float_arg(&self, arg: &str) -> Result<f64, ParserError> {
        let trimmed = arg.trim();

        // 1. LLVM 64-bit Hex Float literal: double 0x3FD921FB54442D18 or -0x...
        if let Some(hex_pos) = trimmed.find("0x") {
            let after_prefix = &trimmed[hex_pos + 2..];
            let token_end = after_prefix
                .find(|c: char| c.is_whitespace() || c == ')' || c == ',')
                .unwrap_or(after_prefix.len());
            let hex_str = &after_prefix[..token_end];

            if !hex_str.is_empty()
                && hex_str.len() <= 16
                && hex_str.chars().all(|c| c.is_ascii_hexdigit())
            {
                let padded = format!("{:0>16}", hex_str);
                if let Ok(bits) = u64::from_str_radix(&padded, 16) {
                    let mut val = f64::from_bits(bits);
                    if trimmed[..hex_pos].trim_end().ends_with('-') && val > 0.0 {
                        val = -val;
                    }
                    if !val.is_finite() {
                        return Err(ParserError::InvalidFloatLiteral(trimmed.to_string()));
                    }
                    return Ok(val);
                }
            }
            return Err(ParserError::InvalidFloatLiteral(trimmed.to_string()));
        }

        // 2. Standard decimal / scientific floating point: double 1.5707963, 6.28e-01
        for word in trimmed.split_whitespace() {
            let clean_word = word.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
            if let Ok(val) = clean_word.parse::<f64>() {
                if !val.is_finite() {
                    return Err(ParserError::InvalidFloatLiteral(trimmed.to_string()));
                }
                return Ok(val);
            }
        }

        Err(ParserError::InvalidFloatLiteral(trimmed.to_string()))
    }
}

// Helpers
/// Returns `true` when `ir` contains recognizable LLVM module or QIR instruction syntax.
pub fn looks_like_llvm_ir(ir: &str) -> bool {
    ir.lines().any(|raw| {
        let line = raw.trim();
        !line.is_empty()
            && !line.starts_with(';')
            && (line.starts_with("define ")
                || line.starts_with("declare ")
                || line.starts_with("target ")
                || line.starts_with("source_filename")
                || line.starts_with("attributes ")
                || line.contains("@__quantum__")
                || line.starts_with("call ")
                || line.starts_with("tail ")
                || line.starts_with("musttail ")
                || line.starts_with("notail ")
                || line.starts_with("invoke ")
                || line.starts_with("ret "))
    })
}

fn is_llvm_call_instruction(trimmed: &str) -> bool {
    let target = trimmed
        .split_once('=')
        .map(|(_, rhs)| rhs.trim_start())
        .unwrap_or(trimmed);
    target.starts_with("call ")
        || target.starts_with("tail ")
        || target.starts_with("musttail ")
        || target.starts_with("notail ")
        || target.starts_with("invoke ")
}
fn strip_llvm_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quotes = false;
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ';' {
            return &line[..i];
        }
    }
    line
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quotes = false;
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_llvm_arguments(args_str: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth: usize = 0;
    let mut in_quotes = false;
    let mut escaped = false;
    let mut start = 0;

    for (i, c) in args_str.char_indices() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quotes = false;
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(args_str[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < args_str.len() {
        args.push(args_str[start..].trim());
    }
    args
}

fn extract_typed_int_index(s: &str) -> Option<u32> {
    for pattern in &["i64 ", "i32 ", "i16 ", "i8 "] {
        if let Some(pos) = s.find(pattern) {
            let after = &s[pos + pattern.len()..];
            if let Some(idx) = extract_first_integer(after) {
                return Some(idx);
            }
        }
    }
    None
}

fn extract_numeric_ssa_index(s: &str) -> Option<u32> {
    let mut rest = s;
    while let Some(pos) = rest.find('%') {
        if let Some(tok) = extract_ssa_token(&rest[pos..]) {
            let suffix = &tok[1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(idx) = suffix.parse::<u32>() {
                    return Some(idx);
                }
            }
            rest = &rest[pos + tok.len()..];
        } else {
            break;
        }
    }
    None
}

fn has_ssa_token_in(s: &str, map: &HashMap<String, u32>) -> bool {
    let mut rest = s;
    while let Some(pos) = rest.find('%') {
        if let Some(tok) = extract_ssa_token(&rest[pos..]) {
            if map.contains_key(tok) {
                return true;
            }
            rest = &rest[pos + tok.len()..];
        } else {
            break;
        }
    }
    false
}

fn extract_inttoptr_index(s: &str) -> Option<u32> {
    extract_typed_int_index(s)
}

fn extract_leading_u32(s: &str) -> Option<u32> {
    let trimmed = s.trim_start();
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if end == 0 {
        None
    } else {
        let rest = &trimmed[end..];
        // Reject digits glued into an identifier (e.g. "0foo")
        if rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return None;
        }
        trimmed[..end].parse().ok()
    }
}

fn extract_first_integer(s: &str) -> Option<u32> {
    extract_leading_u32(s)
}

fn extract_metadata_int(line: &str, key: &str) -> Option<u32> {
    if let Some(pos) = line.find(key) {
        let after_key = line[pos + key.len()..].trim_start();
        let after_quote_key = after_key
            .strip_prefix('"')
            .unwrap_or(after_key)
            .trim_start();
        let eq_pos = after_quote_key.find('=')?;
        if eq_pos > 2 {
            return None;
        }
        let value_region = after_quote_key[eq_pos + 1..]
            .trim_start()
            .trim_start_matches('"');
        let value_region = value_region
            .strip_prefix("i64 ")
            .or_else(|| value_region.strip_prefix("i32 "))
            .or_else(|| value_region.strip_prefix("i16 "))
            .or_else(|| value_region.strip_prefix("i8 "))
            .unwrap_or(value_region);
        extract_leading_u32(value_region)
    } else {
        None
    }
}

fn extract_metadata_string(line: &str, key: &str) -> Option<String> {
    if let Some(pos) = line.find(key) {
        let after = line[pos + key.len()..].trim_start();
        if !after.starts_with('=') {
            return None;
        }
        if let Some(quote_start) = after.find('"') {
            let after_quote = &after[quote_start + 1..];
            if let Some(quote_end) = after_quote.find('"') {
                return Some(after_quote[..quote_end].to_string());
            }
        }
    }
    None
}

fn extract_ssa_token(s: &str) -> Option<&str> {
    if let Some(pos) = s.find('%') {
        let after = &s[pos..];
        let body = &after[1..];
        let len = 1 + body
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '.' && c != '-')
            .unwrap_or(body.len());
        if len > 1 {
            Some(&after[..len])
        } else {
            None
        }
    } else {
        None
    }
}
