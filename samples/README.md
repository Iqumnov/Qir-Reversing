# Quantum Intermediate Representation (QIR) Reference Samples

Standard reference test circuits for the `qir-rev` reverse engineering and verification toolchain.

## Sample Inventory

| File | Qubits | Classical Bits | Gates | Description |
|---|---|---|---|---|
| `bell_state.bc` / `.ll` | 2 | 2 | 5 | Canonical Bell State $|\Phi^+\rangle = (|00\rangle + |11\rangle)/\sqrt{2}$ with phase rotation $R_z(\pi/4)$ and projective measurement. |
| `obfuscated_circuit.bc` / `.ll` | 3 | 2 | 18 | Synthetic obfuscated circuit featuring involutory gate pairs ($H^2=I$, $X^2=I$, $CX^2=I$), inverse phase rotations, and dead-qubit telemetry injection. |
| `grover_2qubit.ll` | 2 | 2 | 12 | 2-qubit Grover search algorithm with state preparation, quantum phase oracle, and diffusion operator. |

## Quick Verification

```bash
# Disassemble binary bitcode (.bc)
qir-rev disasm samples/bell_state.bc

# Deobfuscate and recover canonical OpenQASM 3.0
qir-rev deobf --qasm samples/obfuscated_circuit.ll

# Mathematically prove unitary equivalence on Hilbert state space
qir-rev verify samples/obfuscated_circuit.ll
```
