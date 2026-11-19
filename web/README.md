# QIR Quantum Bytecode Reversing & Deobfuscation Web Suite

Tactical web-based reverse engineering, deobfuscation, and circuit reconstruction suite for **Quantum Intermediate Representation (QIR)** LLVM bitcode and IR. Designed and built by **Matvei Igumnov (Group 231-351)** for the course *Reverse Engineering of Quantum Bytecode*.

---

## ⚡ Architecture Overview

The web suite is powered 100% by pure Rust WebAssembly (`wasm32-unknown-unknown` target of `qir-core`), ensuring exact mathematical parity with the native Rust CLI (`qir`) and native Rizin plugin (`librz_qir.so`).

### Pure Rust WebAssembly Execution Strategy

```
┌────────────────────────────────────────────────────────────────────────┐
│                        USER BROWSER / CLIENT                           │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │               Rust WebAssembly (web/wasm/qir_core.wasm)        │   │
│   │   - Zero-Copy LLVM IR & Bitcode Parser                         │   │
│   │   - Quantum Commutation & Discrete Phase Deobfuscator          │   │
│   │   - 2^N Hilbert Space Unitary Equivalence & Fidelity Verifier  │   │
│   │   - 2D ASCII & Dark Tactical Vector SVG Emitters               │   │
│   └────────────────────────────────────────────────────────────────┘   │
│            ▲                                                           │
│            │ 100% Client-Side In-Memory Execution (0 ms latency)       │
└────────────┼───────────────────────────────────────────────────────────┘
             │ (Optional Fallback / Edge REST API)
┌────────────▼───────────────────────────────────────────────────────────┐
│              CLOUDFLARE EDGE (V8 Isolates / Global CDN)                │
│                                                                        │
│   ┌─────────────────┐       ┌─────────────────┐                        │
│   │Cloudflare Pages │  OR   │Cloudflare Worker│                        │
│   │ (Static Assets) │       │ (Rust WASM API) │                        │
│   └─────────────────┘       └─────────────────┘                        │
└────────────────────────────────────────────────────────────────────────┘
```

1. **Client-First Static SPA (Cloudflare Pages)**:
   - The compiled pure Rust WebAssembly binary (`qir_core.wasm`) executes directly inside the browser's WebAssembly runtime with zero third-party dependencies.
   - Zero backend server dependencies and zero operational costs.
   - Instant response on drag-and-drop file upload.

2. **Edge REST API (Cloudflare Workers & Pages Functions)**:
   - The compiled Rust WebAssembly module is loaded into Cloudflare edge V8 isolates via `worker.js` and `_worker.js`.
   - Provides programmatic reversing, deobfuscation, and unitary verification via standard REST endpoints (`/api/analyze`, `/api/deobfuscate`, `/api/verify`, `/api/full`).

---

## 🚀 Quick Start & Local Preview

To launch the web dashboard locally on your machine:

```bash
cd web

# Option A: Built-in Python 3 HTTP Server
python3 -m http.server 8080

# Option B: Node.js static server
npx serve -l 8080 .
```

Then open `http://localhost:8080` in your browser.

---

## 🌐 Deploying to Cloudflare

### Method 1: Cloudflare Pages (Recommended - 1 Click / Zero Config)

Cloudflare Pages provides global CDN hosting with automatic HTTPS, custom domains, and DDoS protection for free.

#### Via Wrangler CLI:
```bash
cd web

# Log in to Cloudflare (one-time setup)
npx wrangler login

# Deploy static suite + Advanced Edge Functions to Cloudflare Pages
npx wrangler pages deploy . --project-name=qir-quantum-reversing
```

#### Via GitHub / Git Repository:
1. Push your repository to GitHub / GitLab.
2. In Cloudflare Dashboard, go to **Workers & Pages** > **Create** > **Pages** > **Connect to Git**.
3. Select the repository and configure:
   - **Framework preset**: `None`
   - **Build command**: *(leave blank)*
   - **Build output directory**: `web`
4. Click **Save and Deploy**.

Cloudflare automatically provisions an address like `https://qir-quantum-reversing.pages.dev`.

---

### Method 2: Standalone Cloudflare Worker

To deploy exclusively as an edge REST API with static asset bindings:

```bash
cd web

# Run local Worker development server with Miniflare:
npx wrangler dev

# Deploy to Cloudflare Workers:
npx wrangler deploy
```

---

## 📡 Edge REST API Endpoints

The Cloudflare Worker / Pages edge engine exposes the following REST API endpoints:

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/api/health` | `GET` | Health check & engine metadata. |
| `/api/analyze` | `POST` | Disassembles QIR text, extracts qubit count, opcodes, and semantic comments. |
| `/api/deobfuscate` | `POST` | Executes lookahead peephole optimizer, detects anomalies, and outputs clean QIR/QASM. |
| `/api/circuit` | `POST` | Renders ASCII timeline diagram, vector SVG circuit, and OpenQASM 3.0. |
| `/api/full` | `POST` | Comprehensive analysis bundle (Disassembly + Audit + Circuit + QASM). |

### Example cURL Request

```bash
curl -X POST https://qir-quantum-reversing.pages.dev/api/deobfuscate \
  -H "Content-Type: application/json" \
  -d '{
    "code": "%Qubit = type opaque\ndeclare void @__quantum__qis__h__body(%Qubit*)\ndefine void @main() {\n  call void @__quantum__qis__h__body(%Qubit* null)\n  call void @__quantum__qis__h__body(%Qubit* null)\n  ret void\n}"
  }'
```

#### Example Response:
```json
{
  "status": "ok",
  "originalCount": 2,
  "cleanCount": 0,
  "eliminatedCount": 2,
  "obfuscationSeverityPct": 100.0,
  "findings": [
    {
      "patternType": "IDENTITY_INVOLUTION_PAIR",
      "severity": "HIGH",
      "description": "Redundant self-inverse involutory pair: h · h = I on qubit q[0]",
      "qubits": [0]
    }
  ],
  "cleanQasm": "OPENQASM 3.0;\ninclude \"stdgates.inc\";\n\nqubit[1] q;\nbit[0] r;\n\n"
}
```

---

## 🛠 Features & Capabilities

- **Strict Monochrome Brutalist UI**:
  - Matches the ThinkPad ricing style (pitch black `#000000`, neutral greys `#888888`/`#e0e0e0`, high contrast `#ffffff`, zero blue-light fatigue).
  - JetBrains Mono / Cascadia Code typography.
- **Drag & Drop Ingestion**:
  - Direct upload of `.ll` (LLVM IR textual representation) and `.bc` (LLVM bitcode container).
  - Interactive source code editor with live syntax highlighting and one-click demo presets.
- **Multi-Tab Tactical View**:
  1. `DISASSEMBLY`: Hex offsets, synthetic bytecode, quantum mnemonics, and semantic circuit theory comments.
  2. `CIRCUIT`: Dynamic interactive SVG circuit diagram + 2D ASCII timeline matrix.
  3. `ANTI-OBFUSCATION AUDIT`: Detected anomaly table, severity metrics, opcode reduction counters, and detailed reversing telemetry report.
  4. `OPENQASM 3.0`: Clean canonical quantum assembly ready for execution on Qiskit, IBM Quantum, or Rigetti QPUs.
- **Automated Test Suite**:
  - Run `npm test` or `node test_suite.js` to execute 10 automated end-to-end tests covering parsing, disassembling, deobfuscating, and edge worker API integrity.

