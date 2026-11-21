# qir-reversing System Architecture

## 1. High-Level Architecture Overview

`qir-reversing` is a memory-safe, high-precision quantum reverse engineering platform written in pure Rust. It transforms opaque LLVM Quantum Intermediate Representation (QIR) bitcode (`.bc`) and textual IR (`.ll`) into verified quantum circuit models, executes formal commutation-based deobfuscation passes, and validates circuit equivalence in $2^N$-dimensional Hilbert space.

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
│  │  Zero-Copy      │  │ Quantum Commutation Algebra  │ │
│  │  SSA Parser     │  │ & Multi-Pass Deobfuscator    │ │
│  └────────┬────────┘  └──────────────┬───────────────┘ │
│           │                          │                 │
│  ┌────────▼────────┐  ┌──────────────▼───────────────┐ │
│  │ ASCII & Vector  │  │ 2^N Hilbert Space Unitary    │ │
│  │ SVG Emitter     │  │ & Process Fidelity Verifier  │ │
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

### 2.1 Parser (`qir_core::parser`)
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

### 2.3 Mathematical Verifier (`qir_core::unitary`)
- Exact matrix representation of $N$-qubit operators in $\mathbb{C}^{2^N \times 2^N}$.
- Operator matrix multiplication: $U_{tot} = G_k \cdots G_2 G_1$.
- Frobenius matrix distance: $\|U_1 - U_2\|_F = \sqrt{\sum |(U_1)_{ij} - (U_2)_{ij}|^2}$.
- Quantum Process Fidelity: $F(U_1, U_2) = \frac{1}{d^2} |\text{Tr}(U_1^\dagger U_2)|^2$, ensuring invariance under global phase shifts $e^{i\theta}$.
- Reversibility theorem check: $\|U \cdot U^\dagger - I\|_F < 10^{-12}$.

