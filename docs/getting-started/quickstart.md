# QIR Reversing Quickstart Guide

## 1. Building the Platform

To build all native binaries and test suites with zero external dependencies:

```bash
./build_all.sh
```

This compiles:
- `target/release/qir-rev`: Native CLI executable (`qrev` alias is linked automatically when installed via `install.sh` / `install.ps1`).
- `target/release/qir-rev-mcp`: Model Context Protocol server.
- `target/release/librz_qir.so`: Rizin / Cutter C-ABI architecture plugin (`.dylib` on macOS, `.dll` on Windows).

## 2. CLI Command Overview

### 2.1 Disassemble QIR Bitcode or Textual IR
```bash
./target/release/qir-rev disasm demo_programs/lr2_quantum_demo.ll
```
Outputs tabular disassembly with hex opcodes, mnemonics, and semantic annotations.

### 2.2 Render 2D ASCII Timeline Diagram
```bash
./target/release/qir-rev circuit demo_programs/lr2_quantum_demo.ll
```
Outputs visual timeline showing qubits, single-qubit gates, and multi-qubit routing connections (`┼`).

### 2.3 Deobfuscate & Audit Quantum Circuit
```bash
./target/release/qir-rev deobf --qasm demo_programs/lr2_obfuscated.ll
```
Executes dead qubit pruning, involution collapse, and inverse rotation cancellation. Emits clean canonical OpenQASM 3.0.

### 2.4 Formally Verify Unitary Equivalence (Up to 64 Qubits)
```bash
./target/release/qir-rev verify demo_programs/lr2_obfuscated.bc
```
Executes exact dense $2^N \times 2^N$ unitary simulation ($\le 12$ qubits) or Canonical QMDD alternating checking ($13 \le N \le 64$ qubits), calculating Frobenius distance, quantum process fidelity, and unitarity invariance ($U \cdot U^\dagger = I$) directly on `.bc` bitcode or `.ll` text without `llvm-dis`.
