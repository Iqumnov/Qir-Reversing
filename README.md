# qir-reversing

Quantum Intermediate Representation Reverse Engineering, Deobfuscation, and Formal Verification

[![Rust](https://img.shields.io/badge/Rust-1.100%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/Iqumnov/Qir-Reversing/actions/workflows/ci.yml/badge.svg)](https://github.com/Iqumnov/Qir-Reversing/actions)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Formal Verification: Lean 4](https://img.shields.io/badge/Formal%20Proof-Lean%204-purple.svg)](proofs/UnitaryEquivalence.lean)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Native%20Server-purple.svg)](https://modelcontextprotocol.io/)
[![Author: Matvei Igumnov](https://img.shields.io/badge/Author-Matvei%20Igumnov-blue.svg)](https://github.com/Iqumnov)

* **Студент:** Игумнов Матвей
* **Группа:** 231-351  
* **Дисциплина:** Обратная разработка (Reverse Engineering)  
* **Проект:** Высокопроизводительная платформа статического анализа, дизассемблирования, топологической реконструкции, алгебраической деобфускации и унитарной верификации квантового байт-кода LLVM QIR

---

## 1. Архитектура платформы

Платформа реализована как модульный **Rust Cargo Workspace** с нулевыми накладными расходами памяти, гарантией `#![forbid(unsafe_code)]` в математическом ядре и компиляцией в нативные исполняемые файлы, WebAssembly-модули и C-ABI библиотеки для реверс-инструментов:

```
┌────────────────────────────────────────────────────────────────────────┐
│                            QIR-REVERSING                               │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   qir-core       │       │    qir-cli       │       │    qir-mcp       │
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

## 2. Ключевые возможности

### 2.1 Высокоточный парсинг LLVM QIR (`qir_core::parser`)
- **Zero-copy парсер текстового LLVM IR (`.ll`)**: Прямое декодирование синтаксиса LLVM IR без сторонних трансляторов, строгое типизирование указателей `ptr` / `%Qubit*`, извлечение квантовых интринсиков `@__quantum__qis__*`.
- **Поддержка бинарных контейнеров LLVM Bitcode (`.bc`)**: Распознавание заголовка `0x42 0x43 0xc0 0xde` (`BC\xc0\xde`), автоматическая интеграция с `llvm-dis` и декомпиляторами bitstream.
- **Поддержка IEEE-754 64-bit Hex Floats**: Прямое чтение шестнадцатеричных вещественных литералов (например, `double 0x3FE921FB54442D18` $\to \pi/4$).
- **Разрешение SSA-указателей**: Отслеживание выражений `inttoptr (i64 N to ptr)` для виртуальных и физических кубитов.

### 2.2 Квантовая алгебра коммутаторов и деобфускация (`qir_core::deobfuscator`)
В отличие от тривиальных peephole-оптимизаторов, qir-reversing опирается на строгую физико-математическую коммутационную алгебру:
- **Отсечение фантомных кубитов (`DeadQubitPruning`)**: Построение обратного конуса причинности от проективных измерений `mz` и удаление не влияющих на результат кубитов телеметрии.
- **Устранение пар инволюций (`InvolutionCancellation`)**: Устранение самообратных вентилей ($G^2 = I$) для $H, X, Y, Z, CX, CZ, \text{SWAP}, CCX$.
- **Проскальзывание коммутирующих вентилей**: Lookahead-поиск с проверкой квантового коммутатора $[A, B] = 0$:
  - Пространственно разделенные вентили: $[A \otimes I, I \otimes B] = 0$.
  - Диагональные фазовые вентили ($Z$-семейство): $[Z, S] = [S, T] = [R_z(\alpha), R_z(\beta)] = 0$.
  - Коммутация $CX(c, t)$ с $Z_c$ и $X_t$.
  - Коммутация $CZ(c, t)$ со всеми вентилями $Z$-базиса.
- **Редукция непрерывных вращений группы Ли**: Сложение углов $R_z(\alpha) \cdot R_z(\beta) = R_z(\alpha + \beta)$ и аннигиляция взаимно-обратных поворотов $R_z(\theta) \cdot R_z(-\theta) = I$.

### 2.3 Верификация в $2^N$-мерном гильбертовом пространстве (`qir_core::unitary`)
- **Полное матричное моделирование**: Точное вычисление унитарной матрицы $U \in \mathbb{C}^{2^N \times 2^N}$.
- **Метрика расстояния Фробениуса**: $\| U_{\text{orig}} - U_{\text{clean}} \|_F < 10^{-6}$.
- **Квантовая точность процесса (Process Fidelity)**: Инвариантная к глобальному фазовому сдвигу $e^{i\theta}$ метрика:
  $$F(U, V) = \frac{1}{d^2} \left| \text{Tr}(U^\dagger V) \right|^2$$
  Значение $F = 1.00000000$ математически доказывает эквивалентность физического преобразования.
- **Проверка теоремы унитарности и обратимости**: $\| U \cdot U^\dagger - I \|_F < 10^{-12}$.

### 2.4 Формальные доказательства в Lean 4 (`proofs/UnitaryEquivalence.lean`)
Формальная спецификация инвариантности группы $U(2^N)$ при устранении пар инволюций и фазовых вращений.

### 2.5 Нативный плагин Rizin & Cutter (`plugins/rizin`)
- C-ABI разделяемая библиотека `librz_qir.so`.
- Экспорт структуры `RzAsmPlugin` для Rizin 0.9.1.
- Декодирование синтетических квантовых опкодов и операндов напрямую в буфер ассемблера Cutter.

### 2.6 Model Context Protocol (MCP) (`mcp/`)
- Нативный JSON-RPC 2.0 сервер `qir-mcp` для подключения к Cursor, Claude Desktop, Antigravity.
- Предоставляет инструменты: `qir_disassemble`, `qir_deobfuscate`, `qir_render_circuit`, `qir_render_svg`, `qir_verify_unitary`, `qir_export_qasm`.

### 2.7 Web Dashboard & Cloudflare Pages / Workers (`web/`)
- Монохромный тактический интерфейс.
- Поддержка исполнения в браузере (клиентский SPA) и на Cloudflare Edge Workers.

---

## 3. Структура проекта

```text
qir_reversing_project/
├── Cargo.toml                      # Корневой манифест Rust Cargo Workspace
├── LICENSE                         # Apache License 2.0
├── install.sh                      # Автоматический скрипт установки инструментария
├── build_all.sh                    # Единый конвейер сборки, тестов и верификации
├── core/                           # Математическое ядро (#![forbid(unsafe_code)])
│   ├── Cargo.toml
│   └── src/
│       ├── types.rs                # Типы Qubit, ResultBit, QuantumGate
│       ├── parser.rs               # LLVM IR и Bitcode парсер
│       ├── deobfuscator.rs         # Алгебраический деобфускатор
│       ├── emitter.rs              # 2D ASCII, векторный SVG, OpenQASM 3.0
│       ├── unitary.rs              # Гильбертов симулятор и Process Fidelity
│       ├── wasm_bridge.rs          # WebAssembly API мост
│       ├── tests.rs                # Математические юнит-тесты ядра
│       └── lib.rs                  # Точка входа библиотеки
├── cli/                            # CLI интерфейс (target/release/qir)
│   ├── Cargo.toml
│   └── src/main.rs                 # Команды: disasm, circuit, deobf, verify, export
├── mcp/                            # Model Context Protocol сервер (target/release/qir-mcp)
│   ├── Cargo.toml
│   └── src/main.rs                 # JSON-RPC 2.0 обработчик
├── plugins/rizin/                  # C-ABI плагин для Rizin (target/release/librz_qir.so)
│   ├── Cargo.toml
│   └── src/lib.rs                  # RzAsmPlugin реализация
├── proofs/                         # Формальные математические доказательства (Lean 4)
│   └── UnitaryEquivalence.lean     # Теоремы инвариантности унитарной группы
├── docs/                           # Документация (Diataxis Framework)
│   ├── architecture/               # Архитектура подсистем
│   ├── engines/                    # Теория квантовых коммутаторов и гильбертовых матриц
│   ├── getting-started/            # Руководство по быстрому старту
│   └── reference/                  # Справочник CLI и MCP
├── demo_programs/                  # Лабораторные программы (LR2)
│   ├── lr2_quantum_demo.ll         # Чистый образец (H, CNOT, Rz, Mz)
│   ├── lr2_quantum_demo.bc         # Бинарный байт-код
│   ├── lr2_obfuscated.ll           # Обфусцированный образец (инволюции, фантомный q[2])
│   ├── lr2_obfuscated.bc           # Бинарный обфусцированный байт-код
│   └── generate_demos.py           # Генератор образцов
├── samples/                        # Эталонные квантовые бенчмарки (Bell, Grover, Obfuscated)
├── qir_plugin/                     # Десктопное приложение GUI и док Cutter/Rizin
│   ├── qir_gui.py                  # Автономное тактическое десктопное приложение
│   ├── cutter_qir_dock.py          # Интерактивный Quantum Dock для Cutter IDE
│   ├── qir_parser.py               # Python QIR парсер
│   ├── qir_disasm.py               # Дизассемблер и генератор схем
│   └── qir_deobfuscator.py         # Алгебраический деобфускатор
├── tests/                          # Набор автоматических Python-тестов
├── pyproject.toml                  # Конфигурация Python-пакета (PEP 621)
└── web/                            # Клиентский Web-интерфейс и Cloudflare Workers
    ├── index.html
    ├── styles.css
    ├── server.js
    ├── wrangler.toml
    └── _worker.js
```

---

## 4. Сборка и запуск

### 4.1 Быстрая установка локального инструментария (Quick Install):
```bash
# Однострочный установщик:
curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash

# Либо напрямую через Cargo:
cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp
```

### 4.2 Единая сборка всего стека из исходного кода:
```bash
./build_all.sh
```

### 4.3 Примеры вызова CLI:

#### Дизассемблирование:
```bash
./target/release/qir disasm demo_programs/lr2_quantum_demo.ll
```

#### Визуализация топологии цепи (2D ASCII):
```bash
./target/release/qir circuit demo_programs/lr2_quantum_demo.ll
```

#### Деобфускация с экспортом в OpenQASM 3.0:
```bash
./target/release/qir deobf --qasm demo_programs/lr2_obfuscated.ll
```

#### Формальная математическая верификация:
```bash
./target/release/qir verify demo_programs/lr2_obfuscated.ll
```

---

## 5. Лабораторная верификация (ЛР2)

| Тестовый образец | Исходные вентили | Оптимизированные | Обнаружено аномалий | Доказательство унитарности | Process Fidelity |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `lr2_quantum_demo.ll` | 5 | 5 | 0 | $U \equiv U_{\text{clean}}$ | $1.00000000$ |
| `lr2_obfuscated.ll` | 18 | 5 | 5 | $U_{\text{orig}} \equiv U_{\text{clean}}$ | $1.00000000$ |
