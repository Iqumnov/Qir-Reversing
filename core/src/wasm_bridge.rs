//! WebAssembly Bridge for in-browser and edge worker QIR analysis
//! Author: Matvei Igumnov <iqumnov@proton.me>

#![allow(unsafe_code)]

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::deobfuscator::QirDeobfuscator;
use crate::emitter::QirEmitter;
use crate::parser::QirParser;
use crate::unitary::UnitaryVerifier;

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_disassemble(ir_code: &str) -> Result<String, String> {
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    let disasm = QirEmitter::format_disassembly(&insts, "module.ll");

    serde_json::to_string_pretty(&serde_json::json!({
        "disassembly": disasm,
        "opcodes_count": insts.len(),
        "num_qubits": parser.metadata.num_qubits,
        "num_results": parser.metadata.num_results,
        "entry_point": parser.metadata.entry_point
    }))
    .map_err(|e| e.to_string())
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_deobfuscate(ir_code: &str) -> Result<String, String> {
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;

    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&insts);
    let report = deobf.generate_report(&insts, &cleaned);
    let qasm = QirEmitter::to_openqasm(
        &cleaned,
        parser.metadata.num_qubits,
        parser.metadata.num_results,
    );
    let ascii = QirEmitter::render_ascii_circuit(&cleaned);
    let svg = QirEmitter::render_svg_circuit(&cleaned, parser.metadata.num_qubits);

    serde_json::to_string_pretty(&serde_json::json!({
        "original_count": insts.len(),
        "cleaned_count": cleaned.len(),
        "obfuscation_ratio": deobf.obfuscation_ratio,
        "findings": deobf.findings,
        "clean_qasm": qasm,
        "clean_ascii": ascii,
        "clean_svg": svg,
        "report": report
    }))
    .map_err(|e| e.to_string())
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_render_ascii(ir_code: &str) -> Result<String, String> {
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    Ok(QirEmitter::render_ascii_circuit(&insts))
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_render_svg(ir_code: &str) -> Result<String, String> {
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    Ok(QirEmitter::render_svg_circuit(
        &insts,
        parser.metadata.num_qubits,
    ))
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_verify(orig_ir: &str, clean_ir: &str, num_qubits: usize) -> Result<String, String> {
    let mut parser_orig = QirParser::new();
    let insts_orig = parser_orig.parse_ir(orig_ir).map_err(|e| e.to_string())?;

    let mut deobf = QirDeobfuscator::new();
    let (live_orig, _) = deobf.prune_dead_qubits(&insts_orig);

    let insts_clean = if clean_ir.trim().is_empty() {
        deobf.deobfuscate(&insts_orig)
    } else {
        let mut parser_clean = QirParser::new();
        parser_clean.parse_ir(clean_ir).map_err(|e| e.to_string())?
    };

    let n_q = if num_qubits == 0 {
        let max_q = live_orig
            .iter()
            .chain(insts_clean.iter())
            .flat_map(|i| i.gate.active_qubits())
            .map(|q| q.0)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0) as usize;
        max_q.max(1)
    } else {
        num_qubits
    };

    let res = UnitaryVerifier::verify_equivalence(&live_orig, &insts_clean, n_q);

    serde_json::to_string_pretty(&res).map_err(|e| e.to_string())
}

// ── Pure C-ABI WebAssembly Exports (Zero-Dependency In-Browser & Edge WASM) ───

fn pack_string(s: String) -> *mut u8 {
    let bytes = s.into_bytes();
    let len = bytes.len() as u32;
    let mut out = Vec::with_capacity(bytes.len() + 4);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend(bytes);
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ptr
}

unsafe fn unpack_str<'a>(ptr: *const u8, len: usize) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    std::str::from_utf8(slice).unwrap_or("")
}

#[no_mangle]
pub extern "C" fn qir_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Deallocates memory allocated by `qir_alloc`.
///
/// # Safety
///
/// The caller must ensure that `ptr` was allocated by `qir_alloc` with length `len`,
/// and that `ptr` has not already been freed.
#[no_mangle]
pub unsafe extern "C" fn qir_dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        let _ = Vec::from_raw_parts(ptr, 0, len);
    }
}

/// Disassembles QIR textual assembly or bitcode passed via C-pointer.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_disassemble_c(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unpack_str(ptr, len);
    let res = qir_wasm_disassemble(input).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));
    pack_string(res)
}

/// Deobfuscates QIR textual assembly or bitcode passed via C-pointer.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_deobfuscate_c(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unpack_str(ptr, len);
    let res = qir_wasm_deobfuscate(input).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));
    pack_string(res)
}

/// Renders a 2D ASCII quantum circuit schematic.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_render_ascii_c(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unpack_str(ptr, len);
    let res = qir_wasm_render_ascii(input).unwrap_or_else(|e| format!("; Error: {}", e));
    pack_string(res)
}

/// Renders an SVG vector quantum circuit diagram.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_render_svg_c(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unpack_str(ptr, len);
    let res = qir_wasm_render_svg(input)
        .unwrap_or_else(|e| format!("<svg><text>Error: {}</text></svg>", e));
    pack_string(res)
}

/// Verifies unitary equivalence between original and cleaned QIR circuits.
///
/// # Safety
///
/// `orig_ptr` and `clean_ptr` must point to valid UTF-8 byte buffers of lengths `orig_len` and `clean_len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_verify_c(
    orig_ptr: *const u8,
    orig_len: usize,
    clean_ptr: *const u8,
    clean_len: usize,
    num_qubits: usize,
) -> *mut u8 {
    let orig = unpack_str(orig_ptr, orig_len);
    let clean = unpack_str(clean_ptr, clean_len);
    let res = qir_wasm_verify(orig, clean, num_qubits)
        .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));
    pack_string(res)
}
