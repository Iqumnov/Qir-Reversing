# qir-reversing CLI Reference Manual

```
Usage: qir <COMMAND>

Commands:
  disasm   Disassemble QIR LLVM IR / Bitcode file with quantum mnemonics
  circuit  Render 2D ASCII quantum circuit timeline diagram
  deobf    Execute multi-pass quantum deobfuscation and generate audit report
  verify   Formally verify unitary matrix equivalence in 2^N Hilbert space
  export   Export canonical quantum assembly (OpenQASM 3.0)
  help     Print this message or the help of the given subcommand(s)
```

## Flags & Parameters

### `qir disasm <FILE>`
- `<FILE>`: Path to `.ll` textual IR or `.bc` bitcode container.

### `qir circuit <FILE>`
- `<FILE>`: Path to `.ll` or `.bc` file. Renders multi-qubit connections (`─●───`, `─X───`, `─┼───`).

### `qir deobf <FILE> [-q, --qasm]`
- `--qasm`: Appends decompiled canonical OpenQASM 3.0 source block to stdout.

### `qir verify <FILE>`
- Computes $2^N$ matrix representation of original versus cleaned instruction streams.
- Computes Frobenius error $\|U_{orig} - U_{clean}\|_F$.
- Computes Quantum Process Fidelity $F(U_{orig}, U_{clean}) = \frac{1}{d^2} |\text{Tr}(U^\dagger V)|^2$.
- Checks reversibility: $\|U \cdot U^\dagger - I\| < 10^{-12}$.

