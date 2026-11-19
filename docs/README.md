# qir-reversing Technical Documentation Portal

Welcome to the technical documentation for **qir-reversing**, a reverse engineering, deobfuscation, and circuit reconstruction platform for LLVM Quantum Intermediate Representation (QIR).

Designed and developed by **Matvei Igumnov (Group 231-351)**.

---

## 🧭 Documentation Structure (Diataxis Framework)

The documentation is organized according to the **Diataxis information architecture**:

```
docs/
├── README.md                           # Documentation Portal & Navigation
├── getting-started/
│   ├── installation.md                 # Rust Toolchain, Native CLI, Docker, WASM
│   └── quickstart.md                   # 5-minute reverse-engineering walkthrough
├── architecture/
│   ├── qir-specification.md            # LLVM QIR Profile Specification & Bitcode
│   └── quantum-dag.md                  # Quantum Circuit DAG & Topological Sorter
├── engines/
│   └── deobfuscation-passes.md         # Involutions, Rotations, Dead Qubits
└── reference/
    ├── cli-reference.md                # Full command-line interface documentation
    └── openqasm-mapping.md             # QIR intrinsic to OpenQASM 3.0 semantic map
```

---

## ⚡ Core Concepts

* **Zero-Copy Parser (`qir-core`)**: Memory-safe parsing of textual LLVM IR (`.ll`) and binary bitcode (`.bc`) with IEEE-754 64-bit floating-point extraction.
* **Quantum Circuit DAG**: Models qubit timelines as a Directed Acyclic Graph (DAG) for exact dependency analysis and circuit depth profiling.
* **Peephole Optimization Engine**: Multi-pass algebraic reduction eliminating self-inverse pairs ($H \cdot H = I$), continuous opposite rotations ($R_z(\theta) \cdot R_z(-\theta) = I$), and dead qubit telemetry.
* **Multi-Target Ecosystem**: Compiles natively to Linux/Windows/macOS CLI binaries, WebAssembly (`wasm32-unknown-unknown`) for Cloudflare Pages / Workers, and native C-FFI shared libraries (`librz_qir.so`) for Rizin and Cutter.

