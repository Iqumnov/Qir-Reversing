# QIR Reversing Quickstart Guide

## 1. Building the Platform

To build all native binaries and test suites with zero external dependencies:

```bash
./build_all.sh
```

This compiles:
- `target/release/qir-rev`: Native CLI executable (alias: `qrev`).
- `target/release/qir-rev-mcp`: Model Context Protocol server.
- `target/release/librz_qir.so`: Rizin / Cutter C-ABI architecture plugin.

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

### 2.4 Formally Verify Unitary Equivalence
```bash
./target/release/qir-rev verify demo_programs/lr2_obfuscated.ll
```
Computes $2^N \times 2^N$ unitaries, calculates Frobenius distance and quantum process fidelity, and verifies unitarity invariance ($U \cdot U^\dagger = I$).
