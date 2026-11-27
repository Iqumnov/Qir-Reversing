# QIR Reversing CLI Reference Manual

```
Usage: qir-rev <COMMAND>  (alias: qrev)

Commands:
  disasm   Disassemble QIR LLVM IR / Bitcode file with quantum mnemonics
  circuit  Render 2D ASCII quantum circuit timeline diagram
  deobf    Execute multi-pass quantum deobfuscation and generate audit report
  verify   Formally verify unitary equivalence up to 64q (Dense 2^N <=12q, QMDD <=64q)
  export   Export canonical quantum assembly (OpenQASM 3.0) or Vector SVG
  help     Print this message or the help of the given subcommand(s)
```

## Flags & Parameters

### `qir-rev disasm <FILE> [--emit-ir]`
- `<FILE>`: Path to `.ll` textual IR or `.bc` bitcode container (decoded via pure Rust without `llvm-dis`).
- `--emit-ir`: Prints the decoded textual LLVM IR module directly to stdout (useful for inspecting `.bc` modules).

### `qir-rev circuit <FILE>`
- `<FILE>`: Path to `.ll` or `.bc` file. Renders multi-qubit connections (`─●───`, `─X───`, `─┼───`).

### `qir-rev deobf <FILE> [-q, --qasm]`
- `--qasm`: Appends decompiled canonical OpenQASM 3.0 source block to stdout.

### `qir-rev verify <FILE>`
- Selects exact dense $2^N$ unitary simulation ($N \le 12$) or Canonical QMDD alternating checking ($13 \le N \le 64$).
- Computes phase-aligned Frobenius error $\min_\phi \|U_{\text{orig}} - e^{i\phi} U_{\text{clean}}\|_F < 10^{-6}\sqrt{d}$.
- Computes Quantum Process Fidelity $F(U_{\text{orig}}, U_{\text{clean}}) = \frac{1}{d^2} |\text{Tr}(U^\dagger V)|^2$.
- Checks reversibility: $\|U \cdot U^\dagger - I\|_F < \text{unitarity\_tolerance}(N)$ ($10^{-9}$ for $N \le 12$).

### `qir-rev export <FILE> [-f, --format qasm|svg]`
- `-f, --format`: Target export format: `qasm` (default, OpenQASM 3.0) or `svg` (dark tactical vector diagram).
