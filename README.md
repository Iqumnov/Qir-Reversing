# QIR Reversing

Quantum Intermediate Representation Reverse Engineering, Deobfuscation, and Formal Verification

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/Iqumnov/Qir-Reversing/actions/workflows/ci.yml/badge.svg)](https://github.com/Iqumnov/Qir-Reversing/actions)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.24528097.svg)](https://doi.org/10.5281/zenodo.24528097)
[![Formal Verification: Lean 4](https://img.shields.io/badge/Formal%20Proof-Lean%204-purple.svg)](proofs/UnitaryEquivalence.lean)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Native%20Server-purple.svg)](https://modelcontextprotocol.io/)
[![Live Web Dashboard](https://img.shields.io/badge/Live%20Web-qir--reversing.iqumnov.com-emerald.svg)](https://qir-reversing.iqumnov.com)
[![Author: Matvei Igumnov](https://img.shields.io/badge/Author-Matvei%20Igumnov-blue.svg)](https://github.com/Iqumnov)

> **Live Web Application**: [https://qir-reversing.iqumnov.com](https://qir-reversing.iqumnov.com)  
> Zero-install, client-side WebAssembly analyzer for textual LLVM QIR (`.ll`) and binary bitcode (`.bc`). Includes real-time disassembly, 2D ASCII/SVG circuit visualization, algebraic deobfuscation, OpenQASM 3.0 export, and $2^N$-dimensional Hilbert space unitary equivalence verification.

---

## 1. Platform Architecture

The platform is designed as a modular Rust Cargo workspace with zero memory overhead, `#![forbid(unsafe_code)]` in the mathematical core, and multi-target compilation into native CLI binaries, WebAssembly modules, and C-ABI shared libraries for reverse engineering frameworks:

```
┌────────────────────────────────────────────────────────────────────────┐
│                            QIR REVERSING                               │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   qir-core       │       │    qir-rev       │       │   qir-rev-mcp    │
│ - Bitcode/IR     │       │ - Disassembly    │       │ - Model Context  │
│   Parser         │──────▶│ - 2D ASCII & SVG │       │   Protocol for   │
│ - Commutation    │       │ - Deobfuscation  │       │   AI Agents      │
│   Deobfuscator   │       │ - 2^N Unitary    │       │   (Claude/Cursor)│
│ - 2^N Hilbert    │       │   Verification   │       │                  │
│   Verifier       │       │ - OpenQASM 3.0   │       │                  │
└──────────────────┘       └──────────────────┘       └──────────────────┘
         │                          │                          │
         ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   rz-qir-plugin  │       │    qir-web       │       │  proofs/Lean 4   │
│ - librz_qir.so   │       │ - Cloudflare     │       │ - Unitary Group  │
│ - Native Rizin   │       │   Pages / Worker │       │   Invariance     │
│   C-ABI Plugin   │       │ - Client-Side    │       │   Theorems       │
│   Disassembler   │       │   WebAssembly    │       │                  │
└──────────────────┘       └──────────────────┘       └──────────────────┘
```

---

## 2. Core Capabilities

### 2.1 High-Precision LLVM QIR Parsing (`qir_core::parser`)
- **Zero-Copy Textual LLVM IR Parser (`.ll`)**: Direct decoding of LLVM IR syntax without external translators, strict typing of opaque pointers `ptr` / `%Qubit*`, and extraction of quantum intrinsics (`@__quantum__qis__*`).
- **Binary Bitcode Support (`.bc`)**: Detection of the magic bitcode container `0x42 0x43 0xc0 0xde` (`BC\xc0\xde`), with automatic decompression and bitstream parsing.
- **IEEE-754 64-bit Hex Floats**: Direct parsing of hexadecimal floating-point literals (e.g., `double 0x3FE921FB54442D18` $\to \pi/4$).
- **SSA Pointer Resolution**: Tracing of `inttoptr (i64 N to ptr)` expressions for both virtual and physical qubits.

### 2.2 Quantum Commutation Algebra & Deobfuscation (`qir_core::deobfuscator`)
Unlike superficial peephole optimizers, `qir-reversing` applies rigorous quantum commutation algebra:
- **Dead Qubit Telemetry Pruning (`DeadQubitPruning`)**: Reverse causal cone reconstruction from projective measurements `mz` to strip unobserved telemetry or decoy qubits.
- **Involution Pair Cancellation (`InvolutionCancellation`)**: Elimination of self-inverse gate pairs ($G^2 = I$) for $H, X, Y, Z, CX, CZ, \text{SWAP}, CCX$.
- **Commuting Gate Sliding**: Lookahead search with quantum commutator validation ($[A, B] = 0$):
  - Spatially separated gates: $[A \otimes I, I \otimes B] = 0$.
  - Diagonal phase gates ($Z$-family): $[Z, S] = [S, T] = [R_z(\alpha), R_z(\beta)] = 0$.
  - Commutation of $CX(c, t)$ with control-$Z$ and target-$X$.
  - Commutation of $CZ(c, t)$ with all $Z$-basis operations on either wire.
- **Continuous Lie Group Rotation Reduction**: Angle consolidation $R_z(\alpha) \cdot R_z(\beta) = R_z(\alpha + \beta)$ and inverse annihilation $R_z(\theta) \cdot R_z(-\theta) = I$.

### 2.3 $2^N$-Dimensional Hilbert Space Unitary Verification (`qir_core::unitary`)
- **Full Dense Matrix Simulation**: Exact unitary matrix computation $U \in \mathbb{C}^{2^N \times 2^N}$ using Kronecker tensor products.
- **Frobenius Distance Metric**: $\| U_{\text{orig}} - U_{\text{clean}} \|_F < 10^{-6}$.
- **Quantum Process Fidelity**: Global-phase-invariant metric ($e^{i\theta}$):
  $$F(U, V) = \frac{1}{d^2} \left| \text{Tr}(U^\dagger V) \right|^2$$
  A fidelity score of $F = 1.00000000$ guarantees physical transformation equivalence.
- **Unitary Matrix Invertibility Check**: $\| U \cdot U^\dagger - I \|_F < 10^{-12}$.

### 2.4 Formal Proofs in Lean 4 (`proofs/UnitaryEquivalence.lean`)
Machine-checked formal verification in Lean 4 specifying unitary group $U(2^N)$ invariance under involution reduction and phase rotation accumulation.

### 2.5 Native Rizin & Cutter Plugin (`plugins/rizin`)
- C-ABI shared library `librz_qir.so`.
- Exports the `RzAsmPlugin` structure for Rizin 0.9.1+.
- Direct decoding of quantum opcodes and operands into Cutter's interactive disassembly buffer.

### 2.6 Model Context Protocol (MCP) (`mcp/`)
- Native JSON-RPC 2.0 server `qir-rev-mcp` for LLM coding agents (Cursor, Claude Desktop, Antigravity).
- Exposes tools: `qir_disassemble`, `qir_deobfuscate`, `qir_render_circuit`, `qir_render_svg`, `qir_verify_unitary`, and `qir_export_qasm`.

### 2.7 Web Dashboard & Cloudflare Edge (`web/`)
- Client-side WebAssembly SPA running fully in-browser without sending circuit data across the network.
- Edge API support deployed on Cloudflare Workers.
- Available online at [qir-reversing.iqumnov.com](https://qir-reversing.iqumnov.com).

---

## 3. Repository Structure

```text
qir-reversing/
├── Cargo.toml                      # Rust Cargo Workspace root manifest
├── LICENSE                         # Apache License 2.0
├── install.sh                      # One-line toolchain installer
├── build_all.sh                    # Unified build, test, and verification pipeline
├── core/                           # Mathematical core (#![forbid(unsafe_code)])
│   ├── Cargo.toml
│   └── src/
│       ├── types.rs                # Qubit, ResultBit, QuantumGate definitions
│       ├── parser.rs               # LLVM IR and Bitcode parser
│       ├── deobfuscator.rs         # Algebraic commutation deobfuscator
│       ├── emitter.rs              # 2D ASCII, vector SVG, and OpenQASM 3.0
│       ├── unitary.rs              # Hilbert space simulator & Process Fidelity
│       ├── wasm_bridge.rs          # WebAssembly API bridge
│       ├── tests.rs                # Core unit tests
│       └── lib.rs                  # Library entry point
├── cli/                            # CLI executable (target/release/qir-rev)
│   ├── Cargo.toml
│   └── src/main.rs                 # Commands: disasm, circuit, deobf, verify, export
├── mcp/                            # Model Context Protocol server (target/release/qir-rev-mcp)
│   ├── Cargo.toml
│   └── src/main.rs                 # JSON-RPC 2.0 server implementation
├── plugins/rizin/                  # C-ABI plugin for Rizin (target/release/librz_qir.so)
│   ├── Cargo.toml
│   └── src/lib.rs                  # RzAsmPlugin implementation
├── proofs/                         # Formal verification proofs (Lean 4)
│   └── UnitaryEquivalence.lean     # Unitary group invariance theorems
├── demo_programs/                  # Test circuits and benchmark suites
│   ├── lr2_quantum_demo.ll / .bc   # Reference canonical circuit
│   ├── lr2_obfuscated.ll / .bc     # Obfuscated circuit (involutions, telemetry)
│   ├── grover_2qubit.ll            # 2-qubit Grover's search algorithm
│   └── generate_demos.py           # Demo generation script
├── samples/                        # Additional quantum test samples
│   ├── bell_state.ll / .bc         # Bell state preparation (H, CNOT, Rz, Mz)
│   ├── obfuscated_circuit.ll / .bc # Obfuscated test sample
│   └── generate_samples.py         # Sample generator
├── qir_plugin/                     # Desktop GUI and Cutter/Rizin dock
│   ├── qir_gui.py                  # Standalone desktop application
│   ├── cutter_qir_dock.py          # Interactive Quantum Dock for Cutter IDE
│   ├── qir_parser.py               # Python QIR parser
│   ├── qir_disasm.py               # Disassembler and circuit formatter
│   └── qir_deobfuscator.py         # Algebraic deobfuscator
├── tests/                          # Automated Python test suite
├── pyproject.toml                  # Python package configuration (PEP 621)
└── web/                            # Client-side Web interface and Cloudflare Workers
    ├── index.html                  # Responsive tactical SPA
    ├── styles.css                  # Monochromatic design system
    ├── worker.js                   # Cloudflare Workers Edge API
    ├── test_suite.js               # Node.js end-to-end test suite
    └── js/                         # Browser UI logic & WebAssembly loader
```

---

## 4. Build & Installation

### 4.1 Quick Install:

**Linux / macOS:**
```bash
curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex
```

**Via Cargo (Any Platform):**
```bash
cargo install --git https://github.com/Iqumnov/Qir-Reversing --bin qir-rev --bin qir-rev-mcp
```

### 4.2 Building from Source:
```bash
# Clone the repository
git clone https://github.com/Iqumnov/Qir-Reversing.git
cd Qir-Reversing

# Run unified build and test pipeline
./build_all.sh
```

### 4.3 CLI Usage Examples:

#### Disassemble QIR:
```bash
./target/release/qir-rev disasm samples/bell_state.ll
```

#### Render 2D ASCII Circuit Topology:
```bash
./target/release/qir-rev circuit samples/bell_state.ll
```

#### Deobfuscate & Export to OpenQASM 3.0:
```bash
./target/release/qir-rev deobf --qasm samples/obfuscated_circuit.ll
```

#### Formal Unitary Verification:
```bash
./target/release/qir-rev verify samples/obfuscated_circuit.ll
```

---

## 5. Verification Benchmarks

| Circuit Sample | Raw Opcodes | Cleaned Opcodes | Obfuscation Sites | Unitary Proof ($U \equiv U_{\text{clean}}$) | Process Fidelity ($F$) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `lr2_quantum_demo.ll` | 5 | 5 | 0 | Proven | $1.00000000$ |
| `lr2_obfuscated.ll` | 18 | 5 | 5 | Proven | $1.00000000$ |
| `bell_state.ll` | 5 | 5 | 0 | Proven | $1.00000000$ |
| `obfuscated_circuit.ll` | 18 | 5 | 5 | Proven | $1.00000000$ |

---

## License

This project is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.

---

# QIR Reversing (Русская версия)

Обратная разработка, деобфускация и формальная верификация квантового промежуточного представления LLVM QIR

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Лицензия: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.24528097.svg)](https://doi.org/10.5281/zenodo.24528097)
[![Формальная верификация: Lean 4](https://img.shields.io/badge/Formal%20Proof-Lean%204-purple.svg)](proofs/UnitaryEquivalence.lean)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Native%20Server-purple.svg)](https://modelcontextprotocol.io/)
[![Интерактивная веб-версия](https://img.shields.io/badge/Live%20Web-qir--reversing.iqumnov.com-emerald.svg)](https://qir-reversing.iqumnov.com)
[![Автор: Матвей Игумнов](https://img.shields.io/badge/Author-Matvei%20Igumnov-blue.svg)](https://github.com/Iqumnov)

> **Интерактивное веб-приложение**: [https://qir-reversing.iqumnov.com](https://qir-reversing.iqumnov.com)  
> Клиентский WebAssembly-анализатор текстового LLVM QIR (`.ll`) и бинарного биткода (`.bc`) без необходимости установки. Включает дисассемблирование в реальном времени, 2D визуализацию схем в ASCII/SVG, алгебраическую деобфускацию, экспорт в OpenQASM 3.0 и верификацию унитарной эквивалентности в $2^N$-мерном гильбертовом пространстве.

---

## 1. Архитектура платформы

Проект построен как модульный Cargo-воркспейс на безопасном Rust с запретом небезопасного кода (`#![forbid(unsafe_code)]`) в математическом ядре и компиляцией в нативные бинарные файлы CLI, модули WebAssembly и разделяемые библиотеки C-ABI:

```
┌────────────────────────────────────────────────────────────────────────┐
│                            QIR REVERSING                               │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   qir-core       │       │    qir-rev       │       │   qir-rev-mcp    │
│ - Парсер биткода │       │ - Дисассемблер   │       │ - Сервер MCP для │
│   и текстового IR│──────▶│ - 2D ASCII и SVG │       │   ИИ-агентов     │
│ - Деобфускатор   │       │ - Деобфускация   │       │   (Claude/Cursor)│
│ - Унитарный      │       │ - Верификация 2^N│       │                  │
│   верификатор 2^N│       │ - OpenQASM 3.0   │       │                  │
└──────────────────┘       └──────────────────┘       └──────────────────┘
         │                          │                          │
         ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   rz-qir-plugin  │       │    qir-web       │       │  proofs/Lean 4   │
│ - librz_qir.so   │       │ - Cloudflare     │       │ - Теоремы        │
│ - Нативный C-ABI │       │   Pages / Worker │       │   унитарной      │
│   плагин Rizin   │       │ - Клиентский     │       │   инвариантности │
│   дисассемблера  │       │   WebAssembly    │       │                  │
└──────────────────┘       └──────────────────┘       └──────────────────┘
```

---

## 2. Основные возможности

### 2.1 Высокоточный парсер LLVM QIR (`qir_core::parser`)
- **Текстовый парсер без копирования (`.ll`)**: Прямое декодирование синтаксиса LLVM IR без сторонних трансляторов, строгое извлечение типов кубитов `%Qubit*`, битов измерений `%Result*` и параметров квантовых вентилей.
- **Парсер бинарного биткода (`.bc`)**: Потоковый лексический анализ блоков `MODULE_BLOCK`, таблиц идентификаторов и таблиц символов биткода LLVM.

### 2.2 Алгебраический деобфускатор коммутации (`qir_core::deobfuscator`)
- **Сворачивание инволюций**: Идентификация и устранение соседних самообратных пар вентилей через скользящее окно с учетом коммутационных соотношений Паули:
  $$H \cdot H = I, \quad X \cdot X = I, \quad Y \cdot Y = I, \quad Z \cdot Z = I, \quad CX \cdot CX = I, \quad CZ \cdot CZ = I$$
- **Консолидация непрерывных вращений групп Ли**: Слияние последовательных параметрических фазовых поворотов вокруг одной оси:
  $$R_z(\theta_1) \cdot R_z(\theta_2) = R_z(\theta_1 + \theta_2)$$
  Если $(\theta_1 + \theta_2) \equiv 0 \pmod{2\pi}$, поворот схлопывается в тождественный оператор $I$.
- **Отсечение мертвого квантового кода**: Построение обратного конуса причинности от инструкций измерения (`__quantum__qis__mz__body`) и удаление не влияющих на результат вспомогательных вентилей.

### 2.3 Верификатор унитарного гильбертова пространства $2^N$ (`qir_core::unitary`)
- **Плотный симулятор квантовых матриц**: Вычисление полного унитарного оператора $U_{\text{tot}} = \prod_{k=1}^m G_k$ в пространстве $(\mathbb{C}^2)^{\otimes N}$ с тензорным произведением Кронекера.
- **Метрика точности квантового процесса (Process Fidelity)**: Расчет квантовой точности между исходной и оптимизированной схемой:
  $$F(U_{\text{orig}}, U_{\text{opt}}) = \frac{1}{d^2} \left| \text{Tr}(U_{\text{orig}}^\dagger U_{\text{opt}}) \right|^2$$
  где $d = 2^N$. Точность $F = 1.00000000$ строго доказывает сохранение исходного квантового алгоритма.
- **Формальные доказательства в Lean 4**: Машинно-проверенные теоремы инвариантности унитарных групп Ли (`proofs/UnitaryEquivalence.lean`).

---

## 3. Быстрый старт и установка

### Установка одной строкой:
```bash
# Linux / macOS
curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash

# Windows PowerShell
irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex
```

### Сборка из исходников:
```bash
git clone https://github.com/Iqumnov/Qir-Reversing.git
cd Qir-Reversing
cargo build --release
```

### Примеры команд CLI:
```bash
# Дисассемблирование файла QIR
./target/release/qir-rev disasm samples/bell_state.ll

# Построение 2D ASCII топологии схемы
./target/release/qir-rev circuit samples/bell_state.ll

# Деобфускация и трансляция в OpenQASM 3.0
./target/release/qir-rev deobf --qasm samples/obfuscated_circuit.ll

# Полная математическая верификация унитарной эквивалентности
./target/release/qir-rev verify samples/obfuscated_circuit.ll
```

---

## 4. Результаты верификации

| Образец схемы | Базовые опкоды | После деобфускации | Места обфускации | Доказательство ($U \equiv U_{\text{clean}}$) | Точность процесса ($F$) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `lr2_quantum_demo.ll` | 5 | 5 | 0 | Доказано | $1.00000000$ |
| `lr2_obfuscated.ll` | 18 | 5 | 5 | Доказано | $1.00000000$ |
| `bell_state.ll` | 5 | 5 | 0 | Доказано | $1.00000000$ |
| `obfuscated_circuit.ll` | 18 | 5 | 5 | Доказано | $1.00000000$ |

---

## Лицензия

Проект распространяется под открытой лицензией **Apache License 2.0** — подробности в файле [LICENSE](LICENSE).
