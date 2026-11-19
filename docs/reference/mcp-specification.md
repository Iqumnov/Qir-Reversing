# Model Context Protocol (MCP) Specification

## 1. Overview

`qir-mcp` implements the official Model Context Protocol (MCP) JSON-RPC 2.0 specification for AI assistant integration (Claude Desktop, Cursor, Copilot, Antigravity).

## 2. Tools Exported

| Tool Name | Parameters | Returns |
| :--- | :--- | :--- |
| `qir_disassemble` | `code: string` | Formatted disassembly table with hex bytes and semantic annotations |
| `qir_deobfuscate` | `code: string` | Obfuscation ratio, anomaly findings list, executive audit report, clean OpenQASM |
| `qir_render_circuit` | `code: string` | 2D ASCII timeline diagram with multi-qubit routing |
| `qir_render_svg` | `code: string` | Dark tactical vector SVG circuit |
| `qir_verify_unitary` | `code: string` | Frobenius matrix error, quantum process fidelity, unitarity drift |
| `qir_export_qasm` | `code: string` | Canonical OpenQASM 3.0 source |

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

