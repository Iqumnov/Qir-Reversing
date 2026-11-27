# QIR Reversing System Architecture

## 1. High-Level Architecture Overview

**QIR Reversing** (`qir-reversing`) is a memory-safe, high-precision quantum reverse engineering platform written in pure Rust (`#![deny(unsafe_code)]` at crate root). It transforms opaque LLVM Quantum Intermediate Representation (QIR) bitcode (`.bc` / `0x0B17C0DE` wrappers) and textual IR (`.ll`) into verified quantum circuit models, executes formal commutation-based deobfuscation passes, and validates circuit equivalence up to 64 qubits using dense $2^N$ unitary simulation and Canonical QMDD alternating checking.

```
┌────────────────────────────────────────────────────────┐
│                   Input Artifacts                      │
│        LLVM Bitcode (.bc) / Textual IR (.ll)           │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│                qir-core Engine (Rust)                  │
│                                                        │
│  ┌─────────────────┐  ┌──────────────────────────────┐ │
│  │ Pure-Rust .bc & │  │ Quantum Commutation Algebra  │ │
│  │ Zero-Copy Parser│  │ & Multi-Pass Deobfuscator    │ │
│  └────────┬────────┘  └──────────────┬───────────────┘ │
│           │                          │                 │
│  ┌────────▼────────┐  ┌──────────────▼───────────────┐ │
│  │ ASCII & Vector  │  │ Dense 2^N & Canonical QMDD   │ │
│  │ SVG Emitter     │  │ Equivalence Verifier (<=64q) │ │
│  └─────────────────┘  └──────────────────────────────┘ │
└───────────┬───────────────────┬───────────────────┬────┘
            │                   │                   │
            ▼                   ▼                   ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  qir-rev CLI     │  │  qir-rev-mcp     │  │  rz_qir Plugin   │
│  Native Binary   │  │  JSON-RPC 2.0    │  │  C-ABI Dynamic   │
│  (disasm/verify) │  │  Model Protocol  │  │  Rizin / Cutter  │
└──────────────────┘  └──────────────────┘  └──────────────────┘
```

## 2. Core Subsystems

### 2.1 Pure-Rust Bitcode Decoder & Parser (`qir_core::bitcode`, `qir_core::parser`)
- Zero-dependency pure-Rust/WASM LLVM Bitstream reader decoding `BC\xc0\xde` streams and `0x0B17C0DE` wrappers directly in CLI, browser WASM, and Edge isolates.
- Zero-copy string scanning for LLVM textual IR (`.ll`).
- IEEE-754 64-bit double hex literal decoding (`0x3FE9...` $\to$ floating point radians).
- Intrinsic pattern recognition for `@__quantum__qis__*` and runtime functions.
- SSA variable pointer mapping for dynamic and static `inttoptr` constructs.

### 2.2 Quantum Commutation Engine (`qir_core::deobfuscator`)
- Backward causal cone dead qubit extraction ($O(G \cdot Q)$).
- Non-interfering gate commutation based on Pauli algebra:
  $$[A \otimes I, I \otimes B] = 0$$
  $$[Z_i, Z_j] = 0, \quad [CX(c, t), Z_c] = 0, \quad [CX(c, t), X_t] = 0$$
- Involutory pair elimination: $G^2 = I$ for $G \in \{H, X, Y, Z, CX, CZ, \text{SWAP}, CCX\}$.
- Continuous Lie group rotation addition: $R_z(\alpha) \cdot R_z(\beta) = R_z(\alpha + \beta)$.

### 2.3 Hybrid Formal Verifier (`qir_core::unitary`, `qir_core::dd`)
- Exact dense matrix representation of $N$-qubit operators in $\mathbb{C}^{2^N \times 2^N}$ up to 12 qubits natively (10 qubits on `wasm32`).
- Canonical QMDD alternating equivalence engine ($G \to \mathbb{I} \leftarrow H^\dagger$) scaling from 13 (or 11 in WASM) up to 64 qubits without exponential state allocation.
- Operator matrix multiplication: $U_{tot} = G_k \cdots G_2 G_1$.
- Phase-aligned Frobenius matrix distance: $\min_{\phi} \|U_1 - e^{i\phi} U_2\|_F < 10^{-6}\sqrt{d}$.
- Quantum Process Fidelity: $F(U_1, U_2) = \frac{1}{d^2} |\text{Tr}(U_1^\dagger U_2)|^2$, ensuring invariance under global phase shifts $e^{i\theta}$.
- Scale-aware reversibility theorem check: $\|U \cdot U^\dagger - I\|_F < \text{unitarity\_tolerance}(N)$ ($10^{-9}$ for $N \le 12$, scaling as $10^{-9} \cdot 2^{(N-12)/2}$).

