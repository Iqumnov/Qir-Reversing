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
    #[error("Binary LLVM Bitcode (.bc) container requires decompiler (llvm-dis or pyqir) or companion .ll file: {0}")]
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

    /// Parse raw bytes, handling both binary BC header (0x42 0x43 0xc0 0xde) and UTF-8 text.
    pub fn parse_bytes(
        &mut self,
        bytes: &[u8],
        path_hint: &std::path::Path,
    ) -> Result<Vec<QuantumInstruction>, ParserError> {
        // 1. Check for LLVM Bitcode Magic Header: 0x42 0x43 0xc0 0xde ("BC\xc0\xde")
        if bytes.len() >= 4
            && bytes[0] == 0x42
            && bytes[1] == 0x43
            && bytes[2] == 0xc0
            && bytes[3] == 0xde
        {
            // Strategy A: Check companion .ll source file alongside the .bc binary
            let ll_candidate = path_hint.with_extension("ll");
            if ll_candidate.exists() && ll_candidate != path_hint {
                if let Ok(ll_content) = std::fs::read_to_string(&ll_candidate) {
                    return self.parse_ir(&ll_content);
                }
            }

            // Strategy B: Native system decompiler (llvm-dis or pyqir via PATH)
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Attempt with llvm-dis
                if let Ok(output) = std::process::Command::new("llvm-dis")
                    .args(["-o", "-", path_hint.to_str().unwrap_or("")])
                    .output()
                {
                    if output.status.success() {
                        if let Ok(ir_text) = String::from_utf8(output.stdout) {
                            return self.parse_ir(&ir_text);
                        }
                    }
                }

                // Attempt with system / virtualenv python3 with pyqir
                let python_candidates = [
                    std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string()),
                    "python3".to_string(),
                    "python".to_string(),
                ];

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
                        use std::io::Write;
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
        let mut instructions = Vec::new();
        let mut current_address: u64 = 0;

        self.extract_module_metadata(content);

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            // Track inttoptr SSA bindings: %0 = inttoptr (i64 1 to %Qubit*)
            if trimmed.starts_with('%') && trimmed.contains("inttoptr") {
                self.record_pointer_binding(trimmed);
                continue;
            }

            // Parse quantum intrinsic calls (handling standard call, tail call, invoke, and assignment)
            if (trimmed.contains("call ") || trimmed.contains("invoke "))
                && trimmed.contains("@__quantum__qis__")
            {
                if let Some(gate) = self.parse_intrinsic_call(trimmed)? {
                    let mut inst = QuantumInstruction::new(gate, trimmed);
                    inst.address = current_address;
                    instructions.push(inst);
                    current_address += 4;
                }
            }
        }

        // Post-validation: auto-detect qubit count if metadata attribute was omitted
        let max_q = instructions
            .iter()
            .flat_map(|inst| inst.gate.active_qubits())
            .map(|q| q.0)
            .max()
            .map(|max_idx| max_idx + 1)
            .unwrap_or(0);

        if self.metadata.num_qubits < max_q {
            self.metadata.num_qubits = max_q;
        }

        Ok(instructions)
    }

    fn extract_module_metadata(&mut self, content: &str) {
        for line in content.lines() {
            if line.contains("required_num_qubits") {
                if let Some(val) = extract_metadata_int(line, "required_num_qubits") {
                    self.metadata.num_qubits = val;
                }
            }
            if line.contains("required_num_results") {
                if let Some(val) = extract_metadata_int(line, "required_num_results") {
                    self.metadata.num_results = val;
                }
            }
            if line.contains("entry_point") {
                if let Some(name) = extract_metadata_string(line, "entry_point") {
                    self.metadata.entry_point = name;
                }
            }
            if line.contains("qir_profiles") {
                if let Some(prof) = extract_metadata_string(line, "qir_profiles") {
                    self.metadata.qir_profiles = prof;
                }
            }
        }
    }

    fn record_pointer_binding(&mut self, line: &str) {
        // Example: %1 = inttoptr i64 1 to %Qubit*
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() < 2 {
            return;
        }
        let var_name = parts[0].trim();
        let expr = parts[1].trim();

        if let Some(idx) = extract_inttoptr_index(expr) {
            if line.contains("%Qubit") {
                self.var_qubits.insert(var_name.to_string(), idx);
            } else if line.contains("%Result") {
                self.var_results.insert(var_name.to_string(), idx);
            }
        }
    }

    fn parse_intrinsic_call(&mut self, line: &str) -> Result<Option<QuantumGate>, ParserError> {
        let at_idx = match line.find('@') {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let after_at = &line[at_idx + 1..];
        let paren_idx = match after_at.find('(') {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let func_name = &after_at[..paren_idx];
        let args_str = &after_at[paren_idx + 1..after_at.rfind(')').unwrap_or(after_at.len())];

        let args = split_llvm_arguments(args_str);

        // Normalize function name: __quantum__qis__h__body -> h
        let canonical_op = func_name
            .trim_start_matches("__quantum__qis__")
            .trim_end_matches("__body");

        let gate = match canonical_op {
            "h" => QuantumGate::H(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "x" => QuantumGate::X(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "y" => QuantumGate::Y(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "z" => QuantumGate::Z(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "s" => QuantumGate::S(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "s__adj" | "s_adj" | "sdg" => {
                QuantumGate::Sdg(self.parse_qubit_arg(args.first().copied().unwrap_or("")))
            }
            "t" => QuantumGate::T(self.parse_qubit_arg(args.first().copied().unwrap_or(""))),
            "t__adj" | "t_adj" | "tdg" => {
                QuantumGate::Tdg(self.parse_qubit_arg(args.first().copied().unwrap_or("")))
            }
            "cnot" | "cx" => {
                let c = self.parse_qubit_arg(args.first().copied().unwrap_or(""));
                let t = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::CX(c, t)
            }
            "cz" => {
                let c = self.parse_qubit_arg(args.first().copied().unwrap_or(""));
                let t = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::CZ(c, t)
            }
            "swap" => {
                let q1 = self.parse_qubit_arg(args.first().copied().unwrap_or(""));
                let q2 = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::SWAP(q1, q2)
            }
            "ccx" | "toffoli" => {
                let c1 = self.parse_qubit_arg(args.first().copied().unwrap_or(""));
                let c2 = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                let t = self.parse_qubit_arg(args.get(2).copied().unwrap_or(""));
                QuantumGate::CCX(c1, c2, t)
            }
            "rx" => {
                let theta = self.parse_float_arg(args.first().copied().unwrap_or(""))?;
                let q = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::Rx(theta, q)
            }
            "ry" => {
                let theta = self.parse_float_arg(args.first().copied().unwrap_or(""))?;
                let q = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::Ry(theta, q)
            }
            "rz" => {
                let theta = self.parse_float_arg(args.first().copied().unwrap_or(""))?;
                let q = self.parse_qubit_arg(args.get(1).copied().unwrap_or(""));
                QuantumGate::Rz(theta, q)
            }
            "mz" | "m" => {
                let q = self.parse_qubit_arg(args.first().copied().unwrap_or(""));
                let r = if args.len() > 1 {
                    self.parse_result_arg(args.get(1).copied().unwrap_or(""))
                } else if let Some(token) = extract_ssa_token(line.split('=').next().unwrap_or(""))
                {
                    self.var_results
                        .get(token)
                        .copied()
                        .map(ResultBit)
                        .unwrap_or(ResultBit(q.0))
                } else {
                    ResultBit(q.0)
                };
                QuantumGate::Mz(q, r)
            }
            "reset" => {
                QuantumGate::Reset(self.parse_qubit_arg(args.first().copied().unwrap_or("")))
            }
            other => QuantumGate::Custom {
                name: other.to_string(),
                qubits: args.iter().map(|a| self.parse_qubit_arg(a)).collect(),
                results: Vec::new(),
                params: Vec::new(),
            },
        };

        Ok(Some(gate))
    }

    fn parse_qubit_arg(&self, arg: &str) -> Qubit {
        let trimmed = arg.trim();

        // Check for null pointer: %Qubit* null -> q[0]
        if trimmed.contains("null") {
            return Qubit(0);
        }

        // Check for inline inttoptr: inttoptr (i64 1 to %Qubit*)
        if trimmed.contains("inttoptr") {
            if let Some(idx) = extract_inttoptr_index(trimmed) {
                return Qubit(idx);
            }
        }

        // Check for registered SSA variable with exact token match (%0, %1, %10, %q0)
        if let Some(token) = extract_ssa_token(trimmed) {
            if let Some(&idx) = self.var_qubits.get(token) {
                return Qubit(idx);
            }
        }

        // Fallback: extract integer after type
        if let Some(idx) = extract_inttoptr_index(trimmed) {
            return Qubit(idx);
        }

        Qubit(0)
    }

    fn parse_result_arg(&self, arg: &str) -> ResultBit {
        let trimmed = arg.trim();

        if trimmed.contains("null") {
            return ResultBit(0);
        }

        if trimmed.contains("inttoptr") {
            if let Some(idx) = extract_inttoptr_index(trimmed) {
                return ResultBit(idx);
            }
        }

        if let Some(token) = extract_ssa_token(trimmed) {
            if let Some(&idx) = self.var_results.get(token) {
                return ResultBit(idx);
            }
        }

        if let Some(idx) = extract_inttoptr_index(trimmed) {
            return ResultBit(idx);
        }

        ResultBit(0)
    }

    fn parse_float_arg(&self, arg: &str) -> Result<f64, ParserError> {
        let trimmed = arg.trim();

        // 1. LLVM 64-bit Hex Float literal: double 0x3FD921FB54442D18 or -0x...
        if let Some(hex_pos) = trimmed.find("0x") {
            let hex_str = &trimmed[hex_pos + 2..]
                .split(|c: char| !c.is_ascii_hexdigit())
                .next()
                .unwrap_or("");

            if let Ok(bits) = u64::from_str_radix(hex_str, 16) {
                let mut val = f64::from_bits(bits);
                if trimmed[..hex_pos].contains('-') && val > 0.0 {
                    val = -val;
                }
                return Ok(val);
            }
        }

        // 2. Standard decimal / scientific floating point: double 1.5707963, 6.28e-01
        for word in trimmed.split_whitespace() {
            let clean_word = word.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
            if let Ok(val) = clean_word.parse::<f64>() {
                return Ok(val);
            }
        }

        Ok(0.0)
    }
}

// Helpers
fn split_llvm_arguments(args_str: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in args_str.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
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

fn extract_inttoptr_index(s: &str) -> Option<u32> {
    // If format is `i64 <NUM>` or `i32 <NUM>`
    for pattern in &["i64 ", "i32 ", "i16 ", "i8 "] {
        if let Some(pos) = s.find(pattern) {
            let after = &s[pos + pattern.len()..];
            if let Some(idx) = extract_first_integer(after) {
                return Some(idx);
            }
        }
    }
    extract_first_integer(s)
}

fn extract_first_integer(s: &str) -> Option<u32> {
    let mut num_str = String::new();
    let mut in_num = false;

    for c in s.chars() {
        if c.is_ascii_digit() {
            num_str.push(c);
            in_num = true;
        } else if in_num {
            break;
        }
    }

    if num_str.is_empty() {
        None
    } else {
        num_str.parse().ok()
    }
}

fn extract_metadata_int(line: &str, key: &str) -> Option<u32> {
    if let Some(pos) = line.find(key) {
        extract_first_integer(&line[pos + key.len()..])
    } else {
        None
    }
}

fn extract_metadata_string(line: &str, key: &str) -> Option<String> {
    if let Some(pos) = line.find(key) {
        let after = &line[pos + key.len()..];
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
        let len = after
            .find(|c: char| !c.is_alphanumeric() && c != '%' && c != '_')
            .unwrap_or(after.len());
        Some(&after[..len])
    } else {
        None
    }
}
