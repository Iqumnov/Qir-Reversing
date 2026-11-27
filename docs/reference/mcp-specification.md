# Model Context Protocol (MCP) Specification

## 1. Overview

`qir-rev-mcp` implements the official Model Context Protocol (MCP) JSON-RPC 2.0 specification for AI assistant integration (Claude Desktop, Cursor, Copilot, MCP hosts).

## 2. Tools Exported

| Tool Name | Parameters | Returns |
| :--- | :--- | :--- |
| `qir_decode_bitcode` | `bitcode_base64: string` | Decoded textual LLVM QIR IR, opcode count, qubit/result counts, entry point |
| `qir_disassemble` | `code?: string`, `bitcode_base64?: string` | Formatted disassembly table with hex bytes, semantic annotations, and counts |
| `qir_deobfuscate` | `code?: string`, `bitcode_base64?: string` | Obfuscation ratio, anomaly findings, audit report, `live_qubits`, `clean_ascii`, `clean_svg`, clean OpenQASM |
| `qir_render_circuit` | `code?: string`, `bitcode_base64?: string` | 2D ASCII timeline diagram with multi-qubit routing |
| `qir_render_svg` | `code?: string`, `bitcode_base64?: string`, `cleaned?: boolean` | Dark tactical vector SVG circuit |
| `qir_verify_unitary` | `code?: string`, `bitcode_base64?: string`, `clean_code?: string`, `clean_bitcode_base64?: string`, `num_qubits?: integer` | Dense $2^N$ ($\le 12$q) or Canonical QMDD ($\le 64$q) equivalence verdict, Frobenius error, process fidelity, unitarity drift |
| `qir_export_qasm` | `code?: string`, `bitcode_base64?: string` | Canonical OpenQASM 3.0 source |

## 3. Invocation Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "qir_deobfuscate",
    "arguments": {
      "code": "call void @__quantum__qis__h__body(ptr null)\ncall void @__quantum__qis__h__body(ptr null)\ncall void @__quantum__qis__mz__body(ptr null, ptr null)\n"
    }
  }
}
```
