//! Model Context Protocol (MCP) Server for QIR Reverse Engineering
//! Author: Matvei Igumnov <matvei@iqumnov.com>

use qir_core::{QirDeobfuscator, QirEmitter, QirParser};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default, deserialize_with = "deserialize_present_id")]
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn deserialize_present_id<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

#[derive(Debug)]
enum ToolError {
    InvalidParams(String),
    Execution(String),
}

fn count_quantum_ops(insts: &[qir_core::QuantumInstruction]) -> usize {
    insts.iter().filter(|i| !i.is_control_flow).count()
}

fn compute_live_qubits(insts: &[qir_core::QuantumInstruction]) -> u32 {
    insts
        .iter()
        .filter(|i| !i.is_control_flow)
        .flat_map(|i| i.gate.active_qubits())
        .map(|q| q.0.saturating_add(1))
        .max()
        .unwrap_or(0)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ToolError> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    if cleaned.is_empty() {
        return Err(ToolError::InvalidParams(
            "Parameter 'bitcode_base64' must not be empty".to_string(),
        ));
    }
    let val_of = |ch: char| -> Option<u8> {
        match ch {
            'A'..='Z' => Some((ch as u8) - b'A'),
            'a'..='z' => Some((ch as u8) - b'a' + 26),
            '0'..='9' => Some((ch as u8) - b'0' + 52),
            '+' => Some(62),
            '/' => Some(63),
            _ => None,
        }
    };
    let mut padded = cleaned;
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let bytes = padded.as_bytes();
    let num_chunks = bytes.len() / 4;
    let mut out = Vec::with_capacity(num_chunks * 3);
    for (idx, chunk) in bytes.chunks_exact(4).enumerate() {
        let a = val_of(chunk[0] as char)
            .ok_or_else(|| ToolError::InvalidParams("Invalid base64 character".to_string()))?;
        let b = val_of(chunk[1] as char)
            .ok_or_else(|| ToolError::InvalidParams("Invalid base64 character".to_string()))?;
        let c_opt =
            if chunk[2] == b'=' {
                None
            } else {
                Some(val_of(chunk[2] as char).ok_or_else(|| {
                    ToolError::InvalidParams("Invalid base64 character".to_string())
                })?)
            };
        let d_opt =
            if chunk[3] == b'=' {
                None
            } else {
                Some(val_of(chunk[3] as char).ok_or_else(|| {
                    ToolError::InvalidParams("Invalid base64 character".to_string())
                })?)
            };
        if (c_opt.is_none() && d_opt.is_some())
            || (idx + 1 < num_chunks && (c_opt.is_none() || d_opt.is_none()))
        {
            return Err(ToolError::InvalidParams(
                "Malformed base64 padding".to_string(),
            ));
        }
        out.push((a << 2) | (b >> 4));
        if let Some(c) = c_opt {
            out.push((b << 4) | (c >> 2));
            if let Some(d) = d_opt {
                out.push((c << 6) | d);
            }
        }
    }
    Ok(out)
}

fn decode_bitcode_arg(b64: &str) -> Result<String, ToolError> {
    let bytes = decode_base64(b64)?;
    if !qir_core::bitcode::is_llvm_bitcode(&bytes) {
        return Err(ToolError::Execution(
            "Decoded payload is not valid LLVM bitcode (expected BC\\xc0\\xde or 0x0B17C0DE wrapper)"
                .to_string(),
        ));
    }
    qir_core::bitcode::decode_bitcode_to_ir(&bytes).map_err(|e| ToolError::Execution(e.to_string()))
}

fn resolve_ir_arg(
    args: &Value,
    code_key: &str,
    b64_key: &str,
    required: bool,
) -> Result<Option<String>, ToolError> {
    if let Some(b64_val) = args.get(b64_key).filter(|v| !v.is_null()) {
        let b64 = b64_val.as_str().ok_or_else(|| {
            ToolError::InvalidParams(format!("Parameter '{}' must be a string", b64_key))
        })?;
        if !b64.trim().is_empty() {
            return decode_bitcode_arg(b64).map(Some);
        }
    }
    if let Some(code_val) = args.get(code_key).filter(|v| !v.is_null()) {
        let code = code_val.as_str().ok_or_else(|| {
            ToolError::InvalidParams(format!("Parameter '{}' must be a string", code_key))
        })?;
        if code.trim().is_empty() {
            if required {
                return Err(ToolError::Execution(
                    "Input QIR payload is empty.".to_string(),
                ));
            }
            return Ok(None);
        }
        if !qir_core::parser::looks_like_llvm_ir(code) {
            return Err(ToolError::Execution(format!(
                "Parameter '{}' does not contain recognizable LLVM IR or QIR instructions.",
                code_key
            )));
        }
        return Ok(Some(code.to_string()));
    }
    if required {
        Err(ToolError::InvalidParams(format!(
            "Missing required parameter '{}' or '{}'",
            code_key, b64_key
        )))
    } else {
        Ok(None)
    }
}

fn handle_tool_call(name: &str, args: &Value) -> Result<Value, ToolError> {
    if name == "qir_decode_bitcode" {
        let b64 = match args.get("bitcode_base64").filter(|v| !v.is_null()) {
            Some(v) => v.as_str().ok_or_else(|| {
                ToolError::InvalidParams("Parameter 'bitcode_base64' must be a string".to_string())
            })?,
            None => {
                return Err(ToolError::InvalidParams(
                    "Missing required parameter 'bitcode_base64'".to_string(),
                ))
            }
        };
        if b64.trim().is_empty() {
            return Err(ToolError::InvalidParams(
                "Missing required parameter 'bitcode_base64'".to_string(),
            ));
        }
        let ir = decode_bitcode_arg(b64)?;
        let mut parser = QirParser::new();
        let insts = parser
            .parse_ir(&ir)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        return Ok(serde_json::json!({
            "ir": ir,
            "opcodes_count": count_quantum_ops(&insts),
            "num_qubits": parser.metadata.num_qubits,
            "num_results": parser.metadata.num_results,
            "entry_point": parser.metadata.entry_point
        }));
    }

    if !matches!(
        name,
        "qir_disassemble"
            | "qir_deobfuscate"
            | "qir_render_circuit"
            | "qir_render_svg"
            | "qir_verify_unitary"
            | "qir_export_qasm"
    ) {
        return Err(ToolError::InvalidParams(format!("Unknown tool: {}", name)));
    }

    let code = resolve_ir_arg(args, "code", "bitcode_base64", true)?
        .ok_or_else(|| ToolError::Execution("Input QIR payload is empty.".to_string()))?;

    let mut parser = QirParser::new();
    let parse_insts = |p: &mut QirParser, src: &str| {
        p.parse_ir(src)
            .map_err(|e| ToolError::Execution(e.to_string()))
    };

    match name {
        "qir_disassemble" => {
            let insts = parse_insts(&mut parser, &code)?;
            let disasm = QirEmitter::format_disassembly(&insts, "payload.ll");
            Ok(serde_json::json!({
                "disassembly": disasm,
                "opcodes_count": count_quantum_ops(&insts),
                "qubits": parser.metadata.num_qubits,
                "results": parser.metadata.num_results,
                "entry_point": parser.metadata.entry_point
            }))
        }
        "qir_deobfuscate" => {
            let insts = parse_insts(&mut parser, &code)?;
            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);
            let report = deobf.generate_report(&insts, &cleaned);
            let live_qubits = compute_live_qubits(&cleaned);
            let render_qubits = live_qubits.max(1);
            let qasm = QirEmitter::to_openqasm(&cleaned, render_qubits, 0);
            let clean_ascii = QirEmitter::render_ascii_circuit(&cleaned);
            let clean_svg = QirEmitter::render_svg_circuit(&cleaned, render_qubits);

            Ok(serde_json::json!({
                "original_count": count_quantum_ops(&insts),
                "cleaned_count": count_quantum_ops(&cleaned),
                "live_qubits": live_qubits,
                "obfuscation_severity_pct": deobf.obfuscation_ratio,
                "findings": deobf.findings,
                "audit_report": report,
                "clean_ascii": clean_ascii,
                "clean_svg": clean_svg,
                "clean_openqasm": qasm
            }))
        }
        "qir_render_circuit" => {
            let insts = parse_insts(&mut parser, &code)?;
            let ascii = QirEmitter::render_ascii_circuit(&insts);
            Ok(serde_json::json!({ "ascii_circuit": ascii }))
        }
        "qir_render_svg" => {
            let insts = parse_insts(&mut parser, &code)?;
            let render_cleaned = match args.get("cleaned").filter(|v| !v.is_null()) {
                Some(v) => v.as_bool().ok_or_else(|| {
                    ToolError::InvalidParams("Parameter 'cleaned' must be a boolean".to_string())
                })?,
                None => false,
            };
            let svg = if render_cleaned {
                let mut deobf = QirDeobfuscator::new();
                let cleaned = deobf.deobfuscate(&insts);
                let live_qubits = compute_live_qubits(&cleaned).max(1);
                QirEmitter::render_svg_circuit(&cleaned, live_qubits)
            } else {
                QirEmitter::render_svg_circuit(&insts, parser.metadata.num_qubits)
            };
            Ok(serde_json::json!({ "svg_circuit": svg }))
        }
        "qir_verify_unitary" => {
            let insts = parse_insts(&mut parser, &code)?;
            let mut deobf = QirDeobfuscator::new();
            let (live_orig, _) = deobf.prune_dead_qubits(&insts);
            let cleaned = match resolve_ir_arg(args, "clean_code", "clean_bitcode_base64", false)? {
                Some(clean_src) => {
                    let mut clean_parser = QirParser::new();
                    parse_insts(&mut clean_parser, &clean_src)?
                }
                None => deobf.deobfuscate(&insts),
            };
            let explicit_qubits = match args.get("num_qubits").filter(|v| !v.is_null()) {
                Some(v) => v
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| {
                        ToolError::InvalidParams(
                            "Parameter 'num_qubits' must be a non-negative integer".to_string(),
                        )
                    })?,
                None => 0,
            };
            let n_qubits = if explicit_qubits > 0 {
                explicit_qubits
            } else {
                let max_live_q = live_orig
                    .iter()
                    .chain(cleaned.iter())
                    .filter(|i| !i.is_control_flow)
                    .flat_map(|i| i.gate.active_qubits())
                    .map(|q| q.0)
                    .max()
                    .map(|m| m.saturating_add(1))
                    .unwrap_or(0) as usize;
                max_live_q.max(1)
            };
            let res = qir_core::UnitaryVerifier::verify_equivalence(&live_orig, &cleaned, n_qubits);
            Ok(serde_json::json!(res))
        }
        "qir_export_qasm" => {
            let insts = parse_insts(&mut parser, &code)?;
            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);
            let live_qubits = compute_live_qubits(&cleaned).max(1);
            let qasm = QirEmitter::to_openqasm(&cleaned, live_qubits, 0);
            Ok(serde_json::json!({ "openqasm_3": qasm }))
        }
        _ => Err(ToolError::InvalidParams(format!("Unknown tool: {}", name))),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_res = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                println!("{}", err_res);
                let _ = stdout.flush();
                continue;
            }
        };

        // Strict JSON-RPC 2.0: Server MUST NOT reply to a Notification (omitted `id` or `notifications/*`)
        if req.method.starts_with("notifications/") || req.id.is_none() {
            continue;
        }
        let req_id = req.id.unwrap_or(Value::Null);
        if req.jsonrpc != "2.0" {
            let err_res = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: None,
                error: Some(
                    serde_json::json!({ "code": -32600, "message": "Invalid Request: expected jsonrpc '2.0'" }),
                ),
            };
            if let Ok(s) = serde_json::to_string(&err_res) {
                println!("{}", s);
                let _ = stdout.flush();
            }
            continue;
        }

        let res = match req.method.as_str() {
            "ping" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: Some(serde_json::json!({})),
                error: None,
            },
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id.clone(),
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": { "name": "qir-rev-mcp", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": { "tools": {} }
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id.clone(),
                result: Some(serde_json::json!({
                    "tools": [
                        {
                            "name": "qir_decode_bitcode",
                            "description": "Decodes raw binary LLVM bitcode (.bc / 0x0B17C0DE wrapper) via pure Rust into textual LLVM QIR",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded LLVM bitcode (.bc) bytes" }
                                },
                                "required": ["bitcode_base64"]
                            }
                        },
                        {
                            "name": "qir_disassemble",
                            "description": "Disassembles QIR LLVM IR or binary bitcode into annotated quantum assembly",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded binary LLVM bitcode (.bc)" }
                                }
                            }
                        },
                        {
                            "name": "qir_deobfuscate",
                            "description": "Performs anti-obfuscation quantum peephole audit and generates clean ASCII, SVG, and OpenQASM 3.0",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded binary LLVM bitcode (.bc)" }
                                }
                            }
                        },
                        {
                            "name": "qir_render_circuit",
                            "description": "Renders 2D ASCII timeline diagram of quantum circuit",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded binary LLVM bitcode (.bc)" }
                                }
                            }
                        },
                        {
                            "name": "qir_render_svg",
                            "description": "Renders dark tactical vector SVG diagram of quantum circuit (optionally deobfuscated first)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded binary LLVM bitcode (.bc)" },
                                    "cleaned": { "type": "boolean", "description": "When true, deobfuscates before rendering SVG" }
                                }
                            }
                        },
                        {
                            "name": "qir_verify_unitary",
                            "description": "Formally verifies circuit equivalence up to 64 qubits via dense 2^N unitary simulation (<=12q) and Canonical QMDD alternating checking (>12q)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "Original LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded original binary LLVM bitcode (.bc)" },
                                    "clean_code": { "type": "string", "description": "Optional reference/cleaned LLVM QIR IR (auto-deobfuscates if omitted)" },
                                    "clean_bitcode_base64": { "type": "string", "description": "Optional base64-encoded cleaned binary LLVM bitcode (.bc)" },
                                    "num_qubits": { "type": "integer", "description": "Optional qubit count override (defaults to active qubit range)" }
                                }
                            }
                        },
                        {
                            "name": "qir_export_qasm",
                            "description": "Decompiles QIR circuit directly to canonical OpenQASM 3.0",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "code": { "type": "string", "description": "LLVM QIR textual IR code" },
                                    "bitcode_base64": { "type": "string", "description": "Base64-encoded binary LLVM bitcode (.bc)" }
                                }
                            }
                        }
                    ]
                })),
                error: None,
            },
            "tools/call" => {
                let tool_name = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let tool_args = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("arguments"))
                    .unwrap_or(&Value::Null);

                match handle_tool_call(tool_name, tool_args) {
                    Ok(val) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req_id.clone(),
                        result: Some(
                            serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap() }] }),
                        ),
                        error: None,
                    },
                    Err(ToolError::Execution(err)) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req_id.clone(),
                        result: Some(serde_json::json!({
                            "content": [{ "type": "text", "text": err }],
                            "isError": true
                        })),
                        error: None,
                    },
                    Err(ToolError::InvalidParams(err)) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req_id.clone(),
                        result: None,
                        error: Some(serde_json::json!({ "code": -32602, "message": err })),
                    },
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: None,
                error: Some(serde_json::json!({ "code": -32601, "message": "Method not found" })),
            },
        };

        let serialized = serde_json::to_string(&res).unwrap_or_else(|e| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32603, "message": format!("Serialization error: {}", e) }
            })
            .to_string()
        });
        println!("{}", serialized);
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::handle_tool_call;

    const TEST_IR: &str = r#"
define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__mz__body(ptr null, ptr null)
  ret void
}
attributes #0 = { "entry_point" "required_num_qubits"="1" "required_num_results"="1" }
"#;

    fn encode_base64(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
            out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
            out.push(if chunk.len() > 1 {
                TABLE[((triple >> 6) & 0x3F) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(triple & 0x3F) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    #[test]
    fn test_all_mcp_tools() {
        let args = serde_json::json!({ "code": TEST_IR });

        let disasm = handle_tool_call("qir_disassemble", &args).expect("disasm");
        assert_eq!(disasm["opcodes_count"], 4);

        let deobf = handle_tool_call("qir_deobfuscate", &args).expect("deobf");
        assert_eq!(deobf["original_count"], 4);
        assert_eq!(deobf["cleaned_count"], 2);
        assert_eq!(deobf["live_qubits"], 1);
        assert!(deobf["clean_ascii"].as_str().unwrap().contains("[H]"));
        assert!(deobf["clean_svg"].as_str().unwrap().contains("<svg"));
        assert!(deobf["clean_openqasm"]
            .as_str()
            .unwrap()
            .contains("OPENQASM 3.0;"));

        let ascii = handle_tool_call("qir_render_circuit", &args).expect("ascii");
        assert!(ascii["ascii_circuit"].as_str().unwrap().contains("[H]"));

        let svg = handle_tool_call("qir_render_svg", &args).expect("svg");
        assert!(svg["svg_circuit"].as_str().unwrap().contains("<svg"));
        let svg_clean = handle_tool_call(
            "qir_render_svg",
            &serde_json::json!({ "code": TEST_IR, "cleaned": true }),
        )
        .expect("svg_clean");
        assert!(svg_clean["svg_circuit"].as_str().unwrap().contains("<svg"));

        let verify = handle_tool_call("qir_verify_unitary", &args).expect("verify");
        assert_eq!(verify["is_equivalent"], true);

        let clean_single_h = r#"
define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__mz__body(ptr null, ptr null)
  ret void
}
attributes #0 = { "entry_point" "required_num_qubits"="1" "required_num_results"="1" }
"#;
        let verify_pair = handle_tool_call(
            "qir_verify_unitary",
            &serde_json::json!({ "code": TEST_IR, "clean_code": clean_single_h, "num_qubits": 1 }),
        )
        .expect("verify_pair");
        assert_eq!(verify_pair["is_equivalent"], true);

        let verify_nulls = handle_tool_call(
            "qir_verify_unitary",
            &serde_json::json!({
                "code": TEST_IR,
                "bitcode_base64": null,
                "clean_code": null,
                "clean_bitcode_base64": null,
                "num_qubits": null
            }),
        )
        .expect("verify_nulls");
        assert_eq!(verify_nulls["is_equivalent"], true);

        // Pure-Rust bitcode (.bc) decoding & deobfuscation via MCP
        let bell_bc = include_bytes!("../../samples/bell_state.bc");
        let bell_b64 = encode_base64(bell_bc);
        let decoded_bc = handle_tool_call(
            "qir_decode_bitcode",
            &serde_json::json!({ "bitcode_base64": bell_b64 }),
        )
        .expect("decode_bitcode");
        assert_eq!(decoded_bc["opcodes_count"], 5);
        assert_eq!(decoded_bc["num_qubits"], 2);
        let disasm_bc = handle_tool_call(
            "qir_disassemble",
            &serde_json::json!({ "bitcode_base64": bell_b64 }),
        )
        .expect("disasm_bc");
        assert_eq!(disasm_bc["opcodes_count"], 5);

        // 32-qubit QMDD verification via MCP
        let mut ghz32 = String::from(
            "define void @ghz32() #0 {\nentry:\n  call void @__quantum__qis__h__body(ptr null)\n",
        );
        for q in 0..31 {
            ghz32.push_str(&format!(
                "  call void @__quantum__qis__cnot__body(ptr inttoptr (i64 {} to ptr), ptr inttoptr (i64 {} to ptr))\n",
                q, q + 1
            ));
            if q % 8 == 0 {
                ghz32.push_str(&format!(
                    "  call void @__quantum__qis__x__body(ptr inttoptr (i64 {} to ptr))\n",
                    q
                ));
                ghz32.push_str(&format!(
                    "  call void @__quantum__qis__x__body(ptr inttoptr (i64 {} to ptr))\n",
                    q
                ));
            }
        }
        ghz32.push_str(
            "  ret void\n}\nattributes #0 = { \"entry_point\" \"required_num_qubits\"=\"32\" }\n",
        );
        let verify_32 = handle_tool_call(
            "qir_verify_unitary",
            &serde_json::json!({ "code": ghz32, "num_qubits": 32 }),
        )
        .expect("verify_32");
        assert_eq!(verify_32["is_equivalent"], true);
        assert_eq!(verify_32["verification_method"], "Canonical QMDD");

        let qasm = handle_tool_call("qir_export_qasm", &args).expect("qasm");
        assert!(qasm["openqasm_3"].as_str().unwrap().contains("h q[0];"));

        assert!(handle_tool_call("qir_disassemble", &serde_json::json!({})).is_err());
        assert!(
            handle_tool_call("qir_disassemble", &serde_json::json!({ "code": "   " })).is_err()
        );
        assert!(handle_tool_call(
            "qir_verify_unitary",
            &serde_json::json!({ "code": "hello world" })
        )
        .is_err());
        assert!(handle_tool_call(
            "qir_decode_bitcode",
            &serde_json::json!({ "bitcode_base64": "AA==AAAA" })
        )
        .is_err());
        assert!(handle_tool_call("unknown_tool", &args).is_err());
        let unknown_empty = handle_tool_call("unknown_tool", &serde_json::json!({})).unwrap_err();
        assert!(
            matches!(unknown_empty, super::ToolError::InvalidParams(ref msg) if msg.contains("Unknown tool"))
        );
        assert!(matches!(
            handle_tool_call("qir_decode_bitcode", &serde_json::json!({ "bitcode_base64": 123 })),
            Err(super::ToolError::InvalidParams(ref msg)) if msg.contains("must be a string")
        ));
        assert!(matches!(
            handle_tool_call("qir_render_svg", &serde_json::json!({ "code": TEST_IR, "cleaned": "true" })),
            Err(super::ToolError::InvalidParams(ref msg)) if msg.contains("must be a boolean")
        ));
        let bad_ir = serde_json::json!({ "code": "define void @main() {\n  call void @__quantum__qis__cnot__body(ptr null)\n}\n" });
        assert!(matches!(
            handle_tool_call("qir_disassemble", &bad_ir),
            Err(super::ToolError::Execution(_))
        ));

        let req_null: super::JsonRpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).unwrap();
        assert_eq!(req_null.id, Some(serde_json::Value::Null));
        let req_notif: super::JsonRpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"ping"}"#).unwrap();
        assert_eq!(req_notif.id, None);
    }
}
