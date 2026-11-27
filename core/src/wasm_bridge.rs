//! WebAssembly Bridge for in-browser and edge worker QIR analysis
//! Author: Matvei Igumnov <matvei@iqumnov.com>

#![allow(unsafe_code)]

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::deobfuscator::QirDeobfuscator;
use crate::emitter::QirEmitter;
use crate::parser::QirParser;
use crate::unitary::UnitaryVerifier;

fn count_quantum_ops(insts: &[crate::types::QuantumInstruction]) -> usize {
    insts.iter().filter(|i| !i.is_control_flow).count()
}

fn require_non_empty_ir(ir_code: &str) -> Result<(), String> {
    if ir_code.trim().is_empty() {
        Err("Input QIR payload is empty.".to_string())
    } else if !crate::parser::looks_like_llvm_ir(ir_code) {
        Err("Input does not contain recognizable LLVM IR or QIR instructions.".to_string())
    } else {
        Ok(())
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_decode_bitcode(bytes: &[u8]) -> Result<String, String> {
    if !crate::bitcode::is_llvm_bitcode(bytes) {
        return Err(
            "Input is not a valid LLVM bitcode stream (expected BC\\xc0\\xde or 0x0B17C0DE wrapper)"
                .to_string(),
        );
    }
    let ir_text = crate::bitcode::decode_bitcode_to_ir(bytes).map_err(|e| e.to_string())?;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(&ir_text).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ir": ir_text,
        "opcodes_count": count_quantum_ops(&insts),
        "num_qubits": parser.metadata.num_qubits,
        "num_results": parser.metadata.num_results,
        "entry_point": parser.metadata.entry_point
    }))
    .map_err(|e| e.to_string())
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_disassemble(ir_code: &str) -> Result<String, String> {
    require_non_empty_ir(ir_code)?;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    let disasm = QirEmitter::format_disassembly(&insts, "module.ll");

    serde_json::to_string_pretty(&serde_json::json!({
        "disassembly": disasm,
        "opcodes_count": count_quantum_ops(&insts),
        "num_qubits": parser.metadata.num_qubits,
        "num_results": parser.metadata.num_results,
        "entry_point": parser.metadata.entry_point
    }))
    .map_err(|e| e.to_string())
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_deobfuscate(ir_code: &str) -> Result<String, String> {
    require_non_empty_ir(ir_code)?;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;

    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&insts);
    let report = deobf.generate_report(&insts, &cleaned);
    let has_live_ops = cleaned.iter().any(|i| !i.is_control_flow);
    let live_qubits = if has_live_ops {
        cleaned
            .iter()
            .filter(|i| !i.is_control_flow)
            .flat_map(|i| i.gate.active_qubits())
            .map(|q| q.0.saturating_add(1))
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    let qasm_qubits = live_qubits.max(1);
    let clean_disasm = QirEmitter::format_disassembly(&cleaned, "cleaned.ll");
    let qasm = QirEmitter::to_openqasm(&cleaned, qasm_qubits, 0);
    let ascii = QirEmitter::render_ascii_circuit(&cleaned);
    let svg = QirEmitter::render_svg_circuit(&cleaned, qasm_qubits);

    serde_json::to_string_pretty(&serde_json::json!({
        "original_count": count_quantum_ops(&insts),
        "cleaned_count": count_quantum_ops(&cleaned),
        "live_qubits": live_qubits,
        "obfuscation_ratio": deobf.obfuscation_ratio,
        "findings": deobf.findings,
        "clean_disassembly": clean_disasm,
        "clean_qasm": qasm,
        "clean_ascii": ascii,
        "clean_svg": svg,
        "report": report
    }))
    .map_err(|e| e.to_string())
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_render_ascii(ir_code: &str) -> Result<String, String> {
    require_non_empty_ir(ir_code)?;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    Ok(QirEmitter::render_ascii_circuit(&insts))
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_render_svg(ir_code: &str) -> Result<String, String> {
    require_non_empty_ir(ir_code)?;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir_code).map_err(|e| e.to_string())?;
    Ok(QirEmitter::render_svg_circuit(
        &insts,
        parser.metadata.num_qubits,
    ))
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qir_wasm_verify(orig_ir: &str, clean_ir: &str, num_qubits: usize) -> Result<String, String> {
    require_non_empty_ir(orig_ir)?;
    let mut parser_orig = QirParser::new();
    let insts_orig = parser_orig.parse_ir(orig_ir).map_err(|e| e.to_string())?;

    let mut deobf = QirDeobfuscator::new();
    let (live_orig, _) = deobf.prune_dead_qubits(&insts_orig);

    let insts_clean = if clean_ir.trim().is_empty() {
        deobf.deobfuscate(&insts_orig)
    } else {
        require_non_empty_ir(clean_ir)?;
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
            .map(|m| m.saturating_add(1))
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
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(bytes.len() + 4);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend(bytes);
    let mut boxed = out.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    ptr
}

unsafe fn unpack_slice<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], String> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err("Null input pointer passed to WebAssembly bridge".to_string());
    }
    if len > isize::MAX as usize {
        return Err("Input length exceeds maximum addressable slice size".to_string());
    }
    Ok(std::slice::from_raw_parts(ptr, len))
}

unsafe fn unpack_ir_source(ptr: *const u8, len: usize) -> Result<String, String> {
    let slice = unpack_slice(ptr, len)?;
    if slice.is_empty() {
        return Ok(String::new());
    }
    if crate::bitcode::is_llvm_bitcode(slice) {
        return crate::bitcode::decode_bitcode_to_ir(slice).map_err(|e| e.to_string());
    }
    std::str::from_utf8(slice).map(str::to_string).map_err(|_| {
        "Input is neither valid LLVM bitcode (.bc) nor UTF-8 LLVM QIR text.".to_string()
    })
}

/// Decodes raw binary LLVM bitcode (`.bc`) passed via C-pointer to textual LLVM IR JSON.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_decode_bitcode_c(ptr: *const u8, len: usize) -> *mut u8 {
    let res = match unpack_slice(ptr, len) {
        Ok(bytes) => qir_wasm_decode_bitcode(bytes)
            .unwrap_or_else(|e| serde_json::json!({ "error": e }).to_string()),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    pack_string(res)
}

#[no_mangle]
pub extern "C" fn qir_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let mut boxed = vec![0u8; len].into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
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
    if !ptr.is_null() && len > 0 {
        let _ = Vec::from_raw_parts(ptr, 0, len);
    }
}

/// Disassembles QIR textual assembly or binary bitcode passed via C-pointer.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_disassemble_c(ptr: *const u8, len: usize) -> *mut u8 {
    let res = match unpack_ir_source(ptr, len) {
        Ok(input) => qir_wasm_disassemble(&input)
            .unwrap_or_else(|e| serde_json::json!({ "error": e }).to_string()),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    pack_string(res)
}

/// Deobfuscates QIR textual assembly or binary bitcode passed via C-pointer.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_deobfuscate_c(ptr: *const u8, len: usize) -> *mut u8 {
    let res = match unpack_ir_source(ptr, len) {
        Ok(input) => qir_wasm_deobfuscate(&input)
            .unwrap_or_else(|e| serde_json::json!({ "error": e }).to_string()),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    pack_string(res)
}

/// Renders a 2D ASCII quantum circuit schematic.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_render_ascii_c(ptr: *const u8, len: usize) -> *mut u8 {
    let res = match unpack_ir_source(ptr, len) {
        Ok(input) => qir_wasm_render_ascii(&input).unwrap_or_else(|e| format!("; Error: {}", e)),
        Err(e) => format!("; Error: {}", e),
    };
    pack_string(res)
}

/// Renders an SVG vector quantum circuit diagram.
///
/// # Safety
///
/// `ptr` must point to a valid byte buffer of length `len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_render_svg_c(ptr: *const u8, len: usize) -> *mut u8 {
    let format_svg_err = |e: &str| {
        let escaped = e
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><text x=\"10\" y=\"20\" fill=\"#ffffff\">Error: {}</text></svg>", escaped)
    };
    let res = match unpack_ir_source(ptr, len) {
        Ok(input) => qir_wasm_render_svg(&input).unwrap_or_else(|e| format_svg_err(&e)),
        Err(e) => format_svg_err(&e),
    };
    pack_string(res)
}

/// Verifies unitary equivalence between original and cleaned QIR circuits.
///
/// # Safety
///
/// `orig_ptr` and `clean_ptr` must point to valid bitcode/UTF-8 byte buffers of lengths `orig_len` and `clean_len`.
#[no_mangle]
pub unsafe extern "C" fn qir_wasm_verify_c(
    orig_ptr: *const u8,
    orig_len: usize,
    clean_ptr: *const u8,
    clean_len: usize,
    num_qubits: usize,
) -> *mut u8 {
    let res = match (
        unpack_ir_source(orig_ptr, orig_len),
        unpack_ir_source(clean_ptr, clean_len),
    ) {
        (Ok(orig), Ok(clean)) => qir_wasm_verify(&orig, &clean, num_qubits)
            .unwrap_or_else(|e| serde_json::json!({ "error": e }).to_string()),
        (Err(e), _) | (_, Err(e)) => serde_json::json!({ "error": e }).to_string(),
    };
    pack_string(res)
}
