# QIR Reversing Technical Documentation Portal

Welcome to the technical documentation for **QIR Reversing**, a reverse engineering, deobfuscation, and circuit reconstruction platform for LLVM Quantum Intermediate Representation (QIR).

Designed and developed by **Matvei Igumnov** (<matvei@iqumnov.com>).

---

## 🧭 Documentation Structure (Diataxis Framework)

The documentation is organized according to the **Diataxis information architecture**:

```
docs/
├── README.md                                 # Documentation Portal & Navigation
├── getting-started/
│   └── quickstart.md                         # Build, CLI & verification walkthrough
├── architecture/
│   └── system-architecture.md                # Workspace subsystems & Hilbert verification
├── engines/
│   ├── deobfuscation-passes.md               # Involutions, Rotations, Dead Qubits
│   ├── quantum-commutation-algebra.md        # Pauli & controlled-gate commutator rules
│   └── unitary-hilbert-space.md              # Dense 2^N & Canonical QMDD verification (<=64q)
└── reference/
    ├── cli-reference.md                      # Full command-line interface documentation
    └── mcp-specification.md                  # JSON-RPC 2.0 MCP tools & invocation
```

---

## ⚡ Core Concepts

* **Pure-Rust Bitcode & Zero-Copy Parser (`qir-core`)**: Memory-safe decoding of textual LLVM IR (`.ll`) and binary LLVM bitcode (`BC\xc0\xde` / `0x0B17C0DE` `.bc` containers) with IEEE-754 64-bit floating-point extraction.
* **Lookahead Commutation Engine**: Performs algebraic peephole lookahead across commuting gates without breaking semantic ordering.
* **Peephole Optimization Engine**: Multi-pass algebraic reduction eliminating self-inverse pairs ($H \cdot H = I$), continuous opposite rotations ($R_z(\theta) \cdot R_z(-\theta) = I$), and dead qubit telemetry.
* **Hybrid Formal Equivalence Checker (`<=64q`)**: Exact dense $2^N \times 2^N$ Hilbert simulation ($\le 12$q native / $\le 10$q WASM) paired with Canonical QMDD alternating checking ($G \to \mathbb{I} \leftarrow H^\dagger$) up to 64 qubits.
* **Multi-Target Ecosystem**: Compiles natively to Linux/Windows/macOS CLI binaries, WebAssembly (`wasm32-unknown-unknown`) for Cloudflare Pages / Workers, and native C-FFI shared libraries (`librz_qir.so`) for Rizin and Cutter.

