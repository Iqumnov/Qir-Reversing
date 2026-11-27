# QIR Quantum Bytecode Reversing & Deobfuscation Web Suite

Tactical web-based reverse engineering, deobfuscation, and circuit reconstruction suite for **Quantum Intermediate Representation (QIR)** LLVM bitcode and IR. Designed and built by **Matvei Igumnov** (<matvei@iqumnov.com>).

---

## ⚡ Architecture Overview

The web suite is powered 100% by pure Rust WebAssembly (`wasm32-unknown-unknown` target of `qir-core`), ensuring exact mathematical parity with the native Rust CLI (`qir-rev`) and native Rizin plugin (`librz_qir.so`).

### Pure Rust WebAssembly Execution Strategy

```
┌────────────────────────────────────────────────────────────────────────┐
│                        USER BROWSER / CLIENT                           │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │               Rust WebAssembly (web/wasm/qir_core.wasm)        │   │
│   │   - Pure-Rust LLVM IR & Binary Bitcode (.bc) Decoder             │   │
│   │   - Quantum Commutation & Discrete Phase Deobfuscator          │   │
│   │   - 2^N Hilbert Space & QMDD Equivalence Verifier (<=64q)      │   │
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

# Option A: Node.js Full Dashboard & Local Edge API (Recommended)
node server.js

# Option B: Built-in Python 3 Static HTTP Server
python3 -m http.server 8088
```

Then open `http://localhost:8088` in your browser.

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
npx wrangler pages deploy . --project-name=qir-reversing
```

#### Via GitHub / Git Repository:
1. Push your repository to GitHub / GitLab.
2. In Cloudflare Dashboard, go to **Workers & Pages** > **Create** > **Pages** > **Connect to Git**.
3. Select the repository and configure:
   - **Framework preset**: `None`
   - **Build command**: *(leave blank)*
   - **Build output directory**: `web`
4. Click **Save and Deploy**.

Cloudflare automatically provisions a `.pages.dev` preview URL (production deployment lives at `https://qir-reversing.iqumnov.com`).

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
| `/api/decode-bitcode` | `POST` | Decodes raw `.bc` (`application/octet-stream`) or JSON `bitcodeBase64` into textual LLVM IR. |
| `/api/analyze` | `POST` | Disassembles QIR text or binary bitcode, extracting qubit count, opcodes, and semantics. |
| `/api/deobfuscate` | `POST` | Executes lookahead commutation optimizer, detects anomalies, and outputs clean QIR/QASM. |
| `/api/circuit` | `POST` | Renders ASCII timeline diagram and vector SVG circuit. |
| `/api/verify` | `POST` | Verifies unitary equivalence up to 64 qubits (dense simulation + canonical QMDD). |
| `/api/full` | `POST` | Comprehensive analysis bundle (Disassembly + Audit + Circuit + QASM + Verification). |

### Example cURL Request

```bash
curl -X POST https://qir-reversing.iqumnov.com/api/deobfuscate \
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
  "liveQubits": 0,
  "obfuscationSeverityPct": 100.0,
  "findings": [
    {
      "id": 1,
      "pattern_type": "IDENTITY_INVOLUTION_PAIR",
      "severity": "High",
      "description": "Redundant self-inverse involutory pair: h q[0] · h q[0] = I",
      "qubits": [0],
      "instruction_indices": [0, 1]
    }
  ],
  "auditReport": "; ── QIR DEOBFUSCATION AUDIT REPORT ...",
  "cleanQasm": "OPENQASM 3.0;\ninclude \"stdgates.inc\";\n\nqubit[1] q;\n\n",
  "cleanAscii": "; ── QUANTUM CIRCUIT TIMELINE DIAGRAM ...",
  "cleanSvg": "<svg ...>...</svg>"
}
```

---

## 🛠 Features & Capabilities

- **Strict Monochrome Brutalist UI**:
  - Matches the ThinkPad ricing style (pitch black `#000000`, neutral greys `#828282`/`#ebebeb`, high contrast `#ffffff`, zero blue-light fatigue).
  - `Hack` / `JetBrains Mono` / `SF Mono` / `Fira Code` monospace typography.
- **Drag & Drop Ingestion**:
  - Direct in-browser upload & pure-Rust WASM decoding of binary LLVM bitcode (`.bc` / `0x0B17C0DE` wrappers) as well as textual `.ll` IR files.
  - Interactive monospace LLVM QIR source editor with reactive gate validation.
- **Multi-Tab Tactical View**:
  1. `DISASSEMBLY`: Hex offsets, synthetic bytecode, quantum mnemonics, and semantic circuit theory comments.
  2. `CIRCUIT`: Dynamic responsive vector SVG circuit diagram (while edge `/api/circuit` also provides 2D ASCII timelines).
  3. `DEOBFUSCATION`: Detected anomaly table, severity metrics, opcode reduction counters, and canonical OpenQASM 3.0 output.
  4. `OPENQASM 3.0`: Clean canonical quantum assembly ready for execution on Qiskit, IBM Quantum, or Rigetti QPUs.
- **Automated Test Suite**:
  - Run `npm test` or `node test_suite.js` to execute the automated end-to-end test suite across all 7 validation suites covering parsing, pure-Rust `.bc` bitcode decoding, disassembling, circuit rendering, deobfuscation, $2^N$ & canonical QMDD equivalence verification, edge worker API routes, live Node HTTP binary serving, and UI logic.

