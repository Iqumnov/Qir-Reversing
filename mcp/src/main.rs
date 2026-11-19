//! Model Context Protocol (MCP) Server for QIR Reverse Engineering
//! Author: Matvei Igumnov <iqumnov@proton.me>

use qir_core::{QirDeobfuscator, QirEmitter, QirParser};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
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

fn handle_tool_call(name: &str, args: &Value) -> Result<Value, String> {
    let code = args
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'code'".to_string())?;

    let mut parser = QirParser::new();
    let insts = parser.parse_ir(code).map_err(|e| e.to_string())?;

    match name {
        "qir_disassemble" => {
            let disasm = QirEmitter::format_disassembly(&insts, "payload.ll");
            Ok(serde_json::json!({
                "disassembly": disasm,
                "opcodes_count": insts.len(),
                "qubits": parser.metadata.num_qubits,
                "results": parser.metadata.num_results
            }))
        }
        "qir_deobfuscate" => {
            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);
            let report = deobf.generate_report(&insts, &cleaned);
            let qasm = QirEmitter::to_openqasm(
                &cleaned,
                parser.metadata.num_qubits,
                parser.metadata.num_results,
            );

            Ok(serde_json::json!({
                "original_count": insts.len(),
                "cleaned_count": cleaned.len(),
                "obfuscation_severity_pct": deobf.obfuscation_ratio,
                "findings": deobf.findings,
                "audit_report": report,
                "clean_openqasm": qasm
            }))
        }
        "qir_render_circuit" => {
            let ascii = QirEmitter::render_ascii_circuit(&insts);
            Ok(serde_json::json!({ "ascii_circuit": ascii }))
        }
        "qir_render_svg" => {
            let svg = QirEmitter::render_svg_circuit(&insts, parser.metadata.num_qubits);
            Ok(serde_json::json!({ "svg_circuit": svg }))
        }
        "qir_verify_unitary" => {
            let mut deobf = QirDeobfuscator::new();
            let (live_orig, _) = deobf.prune_dead_qubits(&insts);
            let cleaned = deobf.deobfuscate(&insts);
            let max_live_q = live_orig
                .iter()
                .chain(cleaned.iter())
                .flat_map(|i| i.gate.active_qubits())
                .map(|q| q.0)
                .max()
                .map(|m| m + 1)
                .unwrap_or(0) as usize;
            let n_qubits = max_live_q.max(1);
            let res = qir_core::UnitaryVerifier::verify_equivalence(&live_orig, &cleaned, n_qubits);
            Ok(serde_json::json!(res))
        }
        "qir_export_qasm" => {
            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);
            let qasm = QirEmitter::to_openqasm(
                &cleaned,
                parser.metadata.num_qubits,
                parser.metadata.num_results,
            );
            Ok(serde_json::json!({ "openqasm_3": qasm }))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

#[tokio::main]
async fn main() {
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

        // Strict JSON-RPC 2.0: Server MUST NOT reply to a Notification
        if req.id.is_null() {
            continue;
        }

        let res = match req.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": { "name": "qir-mcp", "version": "0.1.0" },
                    "capabilities": { "tools": {} }
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::json!({
                    "tools": [
                        {
                            "name": "qir_disassemble",
                            "description": "Disassembles QIR LLVM IR into annotated quantum assembly",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
                            }
                        },
                        {
                            "name": "qir_deobfuscate",
                            "description": "Performs anti-obfuscation quantum peephole audit and generates clean QASM",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
                            }
                        },
                        {
                            "name": "qir_render_circuit",
                            "description": "Renders 2D ASCII timeline diagram of quantum circuit",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
                            }
                        },
                        {
                            "name": "qir_render_svg",
                            "description": "Renders dark tactical vector SVG diagram of quantum circuit",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
                            }
                        },
                        {
                            "name": "qir_verify_unitary",
                            "description": "Formally computes 2^N Hilbert space unitaries, Frobenius error, and quantum process fidelity",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
                            }
                        },
                        {
                            "name": "qir_export_qasm",
                            "description": "Decompiles QIR circuit directly to canonical OpenQASM 3.0",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "code": { "type": "string", "description": "LLVM QIR IR code" } },
                                "required": ["code"]
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
                        id: req.id,
                        result: Some(
                            serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap() }] }),
                        ),
                        error: None,
                    },
                    Err(err) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: None,
                        error: Some(serde_json::json!({ "code": -32603, "message": err })),
                    },
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(serde_json::json!({ "code": -32601, "message": "Method not found" })),
            },
        };

        println!("{}", serde_json::to_string(&res).unwrap());
        let _ = stdout.flush();
    }
}
