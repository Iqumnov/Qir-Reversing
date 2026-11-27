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
> Zero-install, client-side WebAssembly analyzer for binary LLVM QIR bitcode (`.bc` / `0x0B17C0DE` wrappers) and textual LLVM IR (`.ll`) without external `llvm-dis` or Python dependencies. Includes real-time disassembly, 2D ASCII/SVG circuit visualization, algebraic deobfuscation, OpenQASM 3.0 export, and unitary equivalence verification up to 64 qubits via dense simulation and canonical QMDD alternating checking.

---

## 1. Platform Architecture

The platform is designed as a modular Rust Cargo workspace with zero memory overhead, `#![deny(unsafe_code)]` at the `qir-core` crate root (with `#![allow(unsafe_code)]` strictly isolated to the `wasm_bridge` FFI boundary), and multi-target compilation into native CLI binaries, WebAssembly modules, and C-ABI shared libraries for reverse engineering frameworks:

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
│   Deobfuscator   │       │ - Dense 2^N &    │       │   (Claude/Cursor)│
│ - 2^N / QMDD     │       │   QMDD Verify    │       │                  │
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

### 2.1 Pure-Rust LLVM QIR & Bitcode Parsing (`qir_core::bitcode`, `qir_core::parser`)
- **Zero-Copy Textual LLVM IR Parser (`.ll`)**: Direct decoding of LLVM IR syntax without external translators, strict typing of opaque pointers `ptr` / `%Qubit*`, and extraction of quantum intrinsics (`@__quantum__qis__*`).
- **Pure-Rust / WASM Binary Bitcode Decoder (`.bc`)**: Built-in `#![deny(unsafe_code)]` LLVM Bitstream reader decoding `BC\xc0\xde` containers and `0x0B17C0DE` wrappers directly in native CLI and in-browser WebAssembly (with optional `llvm-dis`/`pyqir` fallback on native builds).
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

### 2.3 $2^N$-Dimensional & QMDD Unitary Verification (`qir_core::unitary`, `qir_core::dd`)
- **Hybrid Exact Matrix + Canonical QMDD Engine**: Full dense $2^N \times 2^N$ unitary simulation up to 12 qubits (10 in WASM), seamlessly transitioning to canonical Quantum Multiple-Valued Decision Diagrams (QMDD) with alternating $G \to \mathbb{I} \leftarrow H^\dagger$ equivalence checking up to 64 qubits.
- **Phase-Aligned Frobenius Distance Metric**: $\min_{\phi} \| U_{\text{orig}} - e^{i\phi} U_{\text{clean}} \|_F < 10^{-6} \sqrt{d}$ where $d = 2^N$ (computed via cancellation-free normalized DD traces/norms at high qubit counts).
- **Quantum Process Fidelity**: Global-phase-invariant metric ($e^{i\theta}$):
  $$F(U, V) = \frac{1}{d^2} \left| \text{Tr}(U^\dagger V) \right|^2$$
  A fidelity score of $F = 1.00000000$ guarantees physical transformation equivalence.
- **Unitary Matrix Invertibility Check**: Exact dense $\| U \cdot U^\dagger - I \|_F < 10^{-9}$ and scale-aware DD reversibility check.

### 2.4 Formal Proofs in Lean 4 (`proofs/UnitaryEquivalence.lean`)
Constructive, kernel-checked Lean 4 proofs (`0` axioms, `0` `sorry`) over the exact cyclotomic ring $\mathbb{Z}[\zeta_8]$ verifying commutative ring axioms, universal diagonal commutativity (`diag_comm`), $2\times 2$/$4\times 4$/$8\times 8$ involutions, the Clifford+T $\mathbb{Z}_8$ group homomorphism (`phaseZ8_add_all`), Clifford conjugation identities, commutation/non-commutation separation theorems, $SU(2)$ spinor $4\pi$-periodicity, and the inductive soundness proof for arbitrary-depth lookahead cancellation (`lookahead_cancel_word`).

### 2.5 Native Rizin & Cutter Plugin (`plugins/rizin`)
- C-ABI shared library (`librz_qir.so` / `librz_qir.dylib` / `rz_qir.dll`).
- Exports the `RzAsmPlugin` structure for Rizin 0.9.1+.
- Direct decoding of quantum opcodes and operands into Cutter's interactive disassembly buffer.

### 2.6 Model Context Protocol (MCP) (`mcp/`)
- Native JSON-RPC 2.0 server `qir-rev-mcp` for LLM coding agents (Cursor, Claude Desktop, MCP clients).
- Exposes tools: `qir_decode_bitcode`, `qir_disassemble`, `qir_deobfuscate`, `qir_render_circuit`, `qir_render_svg`, `qir_verify_unitary`, and `qir_export_qasm` (supporting both `.ll` text and `bitcode_base64` `.bc` payloads, with QMDD verification up to 64 qubits).

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
├── CITATION.cff                    # Zenodo & academic citation metadata
├── install.sh / install.ps1        # Cross-platform standalone toolchain installers
├── build_all.sh / run_demo.sh      # Unified build, test, and verification pipelines
├── launch_cutter.sh / launch_gui.sh# Cutter IDE & Desktop GUI launchers
├── .github/workflows/              # CI & multi-platform release workflows
├── core/                           # Mathematical core (#![deny(unsafe_code)] at crate root)
│   ├── Cargo.toml
│   └── src/
│       ├── types.rs                # Qubit, ResultBit, QuantumGate definitions
│       ├── bitcode/                # Pure-Rust / WASM LLVM bitcode (.bc) decoder
│       ├── parser.rs               # LLVM IR and Bitcode parser
│       ├── deobfuscator.rs         # Algebraic commutation deobfuscator
│       ├── emitter.rs              # 2D ASCII, vector SVG, and OpenQASM 3.0
│       ├── unitary.rs              # Hilbert space simulator & Process Fidelity
│       ├── dd/                     # Canonical QMDD alternating verifier (<= 64 qubits)
│       ├── wasm_bridge.rs          # WebAssembly API bridge
│       ├── tests.rs                # Core unit tests
│       └── lib.rs                  # Library entry point
├── cli/                            # CLI executable (target/release/qir-rev)
│   ├── Cargo.toml
│   └── src/main.rs                 # Commands: disasm, circuit, deobf, verify, export
├── mcp/                            # Model Context Protocol server (target/release/qir-rev-mcp)
│   ├── Cargo.toml
│   └── src/main.rs                 # JSON-RPC 2.0 server implementation
├── plugins/
│   ├── rizin/                      # C-ABI plugin for Rizin (target/release/librz_qir.so)
│   └── dummy/                      # Minimal C-ABI reference probe module for Rizin
├── proofs/                         # Formal verification proofs (Lean 4)
│   └── UnitaryEquivalence.lean     # Unitary group invariance theorems
├── docs/                           # Architecture, engine specs, and CLI/MCP reference
├── demo_programs/                  # Reference LR2 & Grover benchmark circuits (.ll / .bc)
├── samples/                        # Standalone generated samples & generator script
├── qir_plugin/                     # Desktop GUI and Cutter/Rizin Python integration
│   ├── qir_gui.py                  # Standalone desktop application
│   ├── cutter_qir_dock.py          # Interactive Quantum Dock for Cutter IDE
│   ├── rz_qir_plugin.py            # Interactive Rizin pipe automation wrapper
│   ├── qir_parser.py               # Python QIR parser
│   ├── qir_disasm.py               # Disassembler and circuit formatter
│   └── qir_deobfuscator.py         # Algebraic deobfuscator
├── tests/                          # Automated Python test suite
├── pyproject.toml                  # Python package configuration (PEP 621)
└── web/                            # Client-side Web interface and Cloudflare Workers/Pages
    ├── index.html                  # Responsive tactical SPA
    ├── styles.css                  # Monochromatic design system
    ├── server.js                   # Local Node HTTP server & API proxy
    ├── worker.js / _worker.js      # Cloudflare Workers & Pages entrypoints
    ├── worker-core.js              # Shared Edge API router
    ├── wrangler.toml               # Cloudflare deployment manifest
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
cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp
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

#### Deobfuscate & Export to OpenQASM 3.0 / Vector SVG:
```bash
./target/release/qir-rev deobf --qasm samples/obfuscated_circuit.ll
./target/release/qir-rev export samples/obfuscated_circuit.ll --format svg > circuit.svg
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

This project is licensed under the Apache License 2.0 (see [LICENSE](LICENSE)), except the Rizin architecture plugin crate (`plugins/rizin`), which uses `LGPL-3.0-or-later` to match the upstream Rizin plugin ABI headers.

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
> Клиентский WebAssembly-анализатор бинарного LLVM-биткода (`.bc` / обёрток `0x0B17C0DE`) и текстового QIR IR (`.ll`) без внешних зависимостей `llvm-dis` и Python. Включает дизассемблирование в реальном времени, 2D-визуализацию схем в ASCII/SVG, алгебраическую деобфускацию, экспорт в OpenQASM 3.0 и верификацию унитарной эквивалентности до 64 кубитов (матричный симулятор + канонические QMDD).

---

## 1. Архитектура платформы

Проект построен как модульный Cargo-воркспейс на безопасном Rust с запретом небезопасного кода (`#![deny(unsafe_code)]` в корне крейта `qir-core` и `#![allow(unsafe_code)]` строго в `wasm_bridge` FFI) и компиляцией в нативные бинарные файлы CLI, модули WebAssembly и разделяемые библиотеки C-ABI:

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
│ - Верификатор    │       │ - Верификация    │       │                  │
│   2^N / QMDD     │       │   2^N и QMDD     │       │                  │
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

### 2.1 Парсер LLVM QIR и бинарного биткода (`qir_core::bitcode`, `qir_core::parser`)
- **Текстовый парсер без копирования (`.ll`)**: Прямое декодирование синтаксиса LLVM IR без сторонних трансляторов, строгое извлечение типов кубитов `%Qubit*` / `ptr`, битов измерений `%Result*` и параметров квантовых вентилей.
- **Чисто-Rust/WASM декодер бинарного биткода (`.bc`)**: Встроенный ридер потоков LLVM Bitstream (`BC\xc0\xde` и обёрток `0x0B17C0DE`) без `unsafe`, работающий как в CLI, так и напрямую в браузерном WebAssembly и Cloudflare Workers (с опциональным резервом `llvm-dis`/`pyqir` в нативной сборке).

### 2.2 Алгебраический деобфускатор коммутации (`qir_core::deobfuscator`)
- **Сворачивание инволюций**: Идентификация и устранение самообратных пар вентилей через скользящее окно с учетом коммутационных соотношений:
  $$H^2 = I, \quad X^2 = I, \quad Y^2 = I, \quad Z^2 = I, \quad CX^2 = I, \quad CZ^2 = I, \quad \text{SWAP}^2 = I, \quad CCX^2 = I$$
- **Дискретная фазовая группа Клиффорда+T ($\mathbb{Z}_8$)** и **непрерывные вращения групп Ли**: Слияние фазовых гейтов $\{T, S, Z, S^\dagger, T^\dagger\}$ и параметрических поворотов $R_{x,y,z}(\theta_1) \cdot R_{x,y,z}(\theta_2) = R_{x,y,z}(\theta_1 + \theta_2)$ (со схлопыванием при сумме $\equiv 0 \pmod{2\pi}$ с точностью до глобальной фазы).
- **Отсечение мертвого квантового кода**: Построение обратного конуса причинности от инструкций измерения (`__quantum__qis__mz__body`, `mresetz`) и удаление не влияющих на результат вспомогательных вентилей.

### 2.3 Верификатор унитарной эквивалентности $2^N$ и QMDD (`qir_core::unitary`, `qir_core::dd`)
- **Гибридный движок: точная матрица + канонические QMDD**: Полный расчёт унитарного оператора $2^N \times 2^N$ до 12 кубитов (10 в WASM) и автоматический переход к альтернирующей проверке $G \to \mathbb{I} \leftarrow H^\dagger$ на канонических диаграммах решений (QMDD) до 64 кубитов (с проверкой обратимости $\| U \cdot U^\dagger - I \|_F$).
- **Фазово-согласованная норма Фробениуса и Process Fidelity**: Оценка $\min_{\phi} \| U_{\text{orig}} - e^{i\phi} U_{\text{clean}} \|_F < 10^{-6}\sqrt{d}$ и точности процесса:
  $$F(U_{\text{orig}}, U_{\text{opt}}) = \frac{1}{d^2} \left| \text{Tr}(U_{\text{orig}}^\dagger U_{\text{opt}}) \right|^2$$
  где $d = 2^N$. Точность $F = 1.00000000$ строго доказывает сохранение физического квантового алгоритма.
- **Формальные доказательства в Lean 4**: Машинно-проверенные конструктивные теоремы (без `axiom` и `sorry`) в кольце $\mathbb{Z}[\zeta_8]$ (включая полные кольцевые аксиомы `Z8`, универсальную коммутативность диагональных матриц `diag_comm`, инволюции $2\times 2, 4\times 4, 8\times 8$, коммутативность перекрывающихся $CZ$-гейтов `cz_overlap_commute`, группу Клиффорда+T $\mathbb{Z}_8$, сопряжение Клиффорда, $4\pi$-периодичность спиноров и индуктивную теорему `lookahead_cancel_word` для lookahead-сокращения произвольной глубины в `proofs/UnitaryEquivalence.lean`).

---

## 3. Быстрый старт и установка

### Установка одной строкой или через Cargo:
```bash
# Linux / macOS
curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash

# Windows PowerShell
irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex

# Через Cargo (CLI + MCP)
cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp
```

### Сборка из исходников:
```bash
git clone https://github.com/Iqumnov/Qir-Reversing.git
cd Qir-Reversing
./build_all.sh
```

### Примеры команд CLI:
```bash
# Дизассемблирование файла QIR
./target/release/qir-rev disasm samples/bell_state.ll

# Построение 2D ASCII топологии схемы
./target/release/qir-rev circuit samples/bell_state.ll

# Деобфускация и трансляция в OpenQASM 3.0
./target/release/qir-rev deobf --qasm samples/obfuscated_circuit.ll

# Экспорт канонической схемы в OpenQASM 3.0 или SVG
./target/release/qir-rev export samples/obfuscated_circuit.ll --format svg

# Полная математическая верификация унитарной эквивалентности
./target/release/qir-rev verify samples/obfuscated_circuit.ll
```

---

## 4. Результаты верификации

| Образец схемы | Исходные опкоды | После деобфускации | Места обфускации | Доказательство ($U \equiv U_{\text{clean}}$) | Точность процесса ($F$) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `lr2_quantum_demo.ll` | 5 | 5 | 0 | Доказано | $1.00000000$ |
| `lr2_obfuscated.ll` | 18 | 5 | 5 | Доказано | $1.00000000$ |
| `bell_state.ll` | 5 | 5 | 0 | Доказано | $1.00000000$ |
| `obfuscated_circuit.ll` | 18 | 5 | 5 | Доказано | $1.00000000$ |

---

## Лицензия

Проект распространяется под открытой лицензией **Apache License 2.0** (подробности в файле [LICENSE](LICENSE)), за исключением крейта плагина Rizin (`plugins/rizin`), который лицензирован под `LGPL-3.0-or-later` для совместимости с ABI заголовочными файлами Rizin.
