#!/usr/bin/env python3
"""
Quantum Intermediate Representation (QIR) Disassembler & Circuit Graph Engine
Student: Igumnov (Group 231-351)
Course: Reverse Engineering of Quantum Bytecode

Translates binary QIR Bytecode / LLVM IR into:
1. Formatted Disassembly Listing (Address | Hex | Mnemonic | Semantic Comment)
2. ASCII 2D Quantum Circuit Matrix Diagram
3. OpenQASM 3.0 standard export (.qasm)
"""

import sys
import os
from typing import List, Optional, Any, Dict, Tuple

# Ensure local plugin directory is always in sys.path
_PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
if _PLUGIN_DIR not in sys.path:
    sys.path.insert(0, _PLUGIN_DIR)

try:
    from qir_parser import QIRParser, QuantumInstruction  # type: ignore
except ImportError:
    from qir_plugin.qir_parser import QIRParser, QuantumInstruction  # type: ignore


class QIRDisassembler:
    """Disassembler and Circuit visualizer for QIR quantum bytecode."""
    def __init__(self, parser: Optional[QIRParser] = None) -> None:
        self.parser: QIRParser = parser if parser is not None else QIRParser()

    def disassemble_file(self, filepath: str) -> str:
        """Loads and disassembles QIR file."""
        instructions: List[QuantumInstruction] = self.parser.parse_file(filepath)
        return self.format_disassembly(instructions, filepath)

    def format_disassembly(self, instructions: List[QuantumInstruction], filepath: str = "") -> str:
        """Formats instructions into standard reverse-engineering disassembly view."""
        lines: List[str] = []
        fname = os.path.basename(filepath) if filepath else "qir_bytecode"
        lines.append(f"; ═══════════════════════════════════════════════════════════════════════════")
        lines.append(f"; [QIR DISASSEMBLER] // LLVM QUANTUM BYTECODE REVERSE ENGINEERING")
        lines.append(f"; Target File      : {fname}")
        lines.append(f"; Quantum Profile  : Custom QIR Profile (Profile: Base / Adaptive)")
        lines.append(f"; Allocated Qubits : {self.parser.num_qubits}")
        lines.append(f"; Results Registers: {self.parser.num_results}")
        lines.append(f"; Entry Point      : @{self.parser.entry_point or 'main'}")
        lines.append(f"; Total Opcodes    : {len(instructions)}")
        lines.append(f"; ═══════════════════════════════════════════════════════════════════════════")
        lines.append(f"")
        lines.append(f"  {'OFFSET':<10} {'HEX BYTES':<14} {'MNEMONIC':<26} {'SEMANTIC ANNOTATION'}")
        lines.append(f"  " + "─" * 73)

        for inst in instructions:
            if inst.is_control_flow and inst.label:
                lines.append(f"\n{inst.label}:")
                continue

            hex_str = " ".join(f"{b:02x}" for b in inst.synthetic_bytes)
            mnem = inst.mnemonic()
            comment = self._get_semantic_comment(inst)
            lines.append(f"  0x{inst.address:06x}  {hex_str:<14} {mnem:<26} ; {comment}")

        lines.append(f"  " + "─" * 73)
        return "\n".join(lines)

    def _get_semantic_comment(self, inst: QuantumInstruction) -> str:
        """Generates semantic reverse engineering annotation for quantum gate."""
        if inst.is_control_flow:
            return f"Classical control-flow: {inst.raw_call}"
        if inst.op == 'h':
            return f"Superposition: |0> -> (|0>+|1>)/√2 on q[{inst.qubits[0]}]"
        elif inst.op in ('cx', 'cnot'):
            return f"Entanglement: Controlled-NOT (ctrl=q[{inst.qubits[0]}], tgt=q[{inst.qubits[1]}])"
        elif inst.op == 'cz':
            return f"Controlled-Z phase flip (ctrl=q[{inst.qubits[0]}], tgt=q[{inst.qubits[1]}])"
        elif inst.op == 'swap':
            return f"SWAP quantum state between q[{inst.qubits[0]}] and q[{inst.qubits[1]}]"
        elif inst.op in ('ccx', 'toffoli'):
            return f"Toffoli (CCX) gate on ({inst.qubits[0]}, {inst.qubits[1]}) -> tgt={inst.qubits[2]}"
        elif inst.op == 'x':
            return f"Pauli-X (NOT / Bit-Flip) on q[{inst.qubits[0]}]"
        elif inst.op == 'y':
            return f"Pauli-Y (Bit+Phase Flip) on q[{inst.qubits[0]}]"
        elif inst.op == 'z':
            return f"Pauli-Z (Phase-Flip) on q[{inst.qubits[0]}]"
        elif inst.op == 's':
            return f"Phase S gate (+π/2 phase) on q[{inst.qubits[0]}]"
        elif inst.op in ('s_adj', 'sdg'):
            return f"Adjoint Phase S† gate (-π/2 phase) on q[{inst.qubits[0]}]"
        elif inst.op == 't':
            return f"Phase T gate (+π/4 phase) on q[{inst.qubits[0]}]"
        elif inst.op in ('t_adj', 'tdg'):
            return f"Adjoint Phase T† gate (-π/4 phase) on q[{inst.qubits[0]}]"
        elif inst.op == 'rz':
            theta = inst.params[0] if inst.params else 0.0
            return f"Phase Rotation Rz({theta:.4f} rad) on q[{inst.qubits[0]}]"
        elif inst.op == 'rx':
            theta = inst.params[0] if inst.params else 0.0
            return f"Rotation Rx({theta:.4f} rad) on q[{inst.qubits[0]}]"
        elif inst.op == 'ry':
            theta = inst.params[0] if inst.params else 0.0
            return f"Rotation Ry({theta:.4f} rad) on q[{inst.qubits[0]}]"
        elif inst.op == 'mz':
            q = inst.qubits[0] if inst.qubits else 0
            r = inst.results[0] if inst.results else 0
            return f"Projective Z-measurement: collapse q[{q}] -> classical r[{r}]"
        elif inst.op == 'qalloc':
            return f"Runtime qubit allocation -> q[{inst.qubits[0]}]"
        elif inst.op == 'qfree':
            return f"Runtime qubit deallocation for q[{inst.qubits[0]}]"
        return "Quantum operation"

    def render_circuit_diagram(self, instructions: Optional[List[QuantumInstruction]] = None) -> str:
        """Builds 2D ASCII Quantum Circuit Matrix diagram."""
        target_insts: List[QuantumInstruction] = instructions if instructions is not None else self.parser.instructions

        num_q: int = max(self.parser.num_qubits, 1)
        num_r: int = max(self.parser.num_results, 1)

        qubit_wires: List[List[str]] = [[] for _ in range(num_q)]

        for inst in target_insts:
            if inst.is_control_flow:
                continue

            if inst.op in ('h', 'x', 'y', 'z', 's', 't', 's_adj', 't_adj'):
                q = inst.qubits[0]
                box = f"[{inst.op.upper()[:4]}]"
                for i in range(num_q):
                    qubit_wires[i].append(box if i == q else "─────")

            elif inst.op in ('rx', 'ry', 'rz'):
                q = inst.qubits[0]
                box = f"[{inst.op.capitalize()}]"
                for i in range(num_q):
                    qubit_wires[i].append(box if i == q else "─────")

            elif inst.op in ('cx', 'cnot'):
                c, t = inst.qubits[0], inst.qubits[1]
                min_idx = min(c, t)
                max_idx = max(c, t)
                for i in range(num_q):
                    if i == c:
                        qubit_wires[i].append("──●──")
                    elif i == t:
                        qubit_wires[i].append("──X──")
                    elif min_idx < i < max_idx:
                        qubit_wires[i].append("──┼──")
                    else:
                        qubit_wires[i].append("─────")

            elif inst.op in ('ccx', 'toffoli'):
                c1, c2, t = inst.qubits[0], inst.qubits[1], inst.qubits[2]
                min_idx = min(c1, c2, t)
                max_idx = max(c1, c2, t)
                for i in range(num_q):
                    if i in (c1, c2):
                        qubit_wires[i].append("──●──")
                    elif i == t:
                        qubit_wires[i].append("──X──")
                    elif min_idx < i < max_idx:
                        qubit_wires[i].append("──┼──")
                    else:
                        qubit_wires[i].append("─────")

            elif inst.op == 'mz':
                q = inst.qubits[0]
                for i in range(num_q):
                    qubit_wires[i].append("─[M]─" if i == q else "─────")
            else:
                for i in range(num_q):
                    qubit_wires[i].append("─────")

        # Format rows
        diagram_lines: List[str] = []
        diagram_lines.append("; ── QUANTUM CIRCUIT TIMELINE DIAGRAM ──────────────────────────────────────")
        for i in range(num_q):
            prefix = f"q[{i}]: ──"
            wire_str = "─".join(qubit_wires[i]) if qubit_wires[i] else "────────────────────"
            diagram_lines.append(f"{prefix}{wire_str}──")

        # Classical registers
        for j in range(num_r):
            prefix = f"r[{j}]: ══"
            c_wire = "═" * max(20, len(diagram_lines[-1]) - len(prefix))
            diagram_lines.append(f"{prefix}{c_wire}")

        diagram_lines.append("; ──────────────────────────────────────────────────────────────────────────")
        return "\n".join(diagram_lines)

    def export_openqasm(self, instructions: Optional[List[QuantumInstruction]] = None, out_path: Optional[str] = None) -> str:
        """Exports decompiled quantum operations to standard OpenQASM 3.0 format."""
        target_insts: List[QuantumInstruction] = instructions if instructions is not None else self.parser.instructions

        num_q: int = max(self.parser.num_qubits, 1)
        num_r: int = max(self.parser.num_results, 1)

        lines: List[str] = [
            "// OpenQASM 3.0 decompiled from LLVM QIR",
            "// Reverse Engineering Suite - Igumnov (231-351)",
            "OPENQASM 3.0;",
            'include "stdgates.inc";',
            "",
            f"qubit[{num_q}] q;",
            f"bit[{num_r}] r;",
            ""
        ]

        for inst in target_insts:
            if inst.is_control_flow:
                continue
            if inst.op in ('h', 'x', 'y', 'z', 's', 't'):
                lines.append(f"{inst.op} q[{inst.qubits[0]}];")
            elif inst.op in ('s_adj', 'sdg'):
                lines.append(f"sdg q[{inst.qubits[0]}];")
            elif inst.op in ('t_adj', 'tdg'):
                lines.append(f"tdg q[{inst.qubits[0]}];")
            elif inst.op in ('cx', 'cnot'):
                lines.append(f"cx q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op == 'cz':
                lines.append(f"cz q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op == 'swap':
                lines.append(f"swap q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op in ('ccx', 'toffoli'):
                lines.append(f"ccx q[{inst.qubits[0]}], q[{inst.qubits[1]}], q[{inst.qubits[2]}];")
            elif inst.op in ('rx', 'ry', 'rz'):
                param = inst.params[0] if inst.params else 0.0
                lines.append(f"{inst.op}({param:.6f}) q[{inst.qubits[0]}];")
            elif inst.op == 'mz':
                lines.append(f"r[{inst.results[0]}] = measure q[{inst.qubits[0]}];")

        content = "\n".join(lines)
        if out_path:
            with open(out_path, "w", encoding="utf-8") as f:
                f.write(content)
        return content


if __name__ == "__main__":
    if len(sys.argv) > 1:
        disasm = QIRDisassembler()
        out = disasm.disassemble_file(sys.argv[1])
        print(out)
        print()
        print(disasm.render_circuit_diagram())
    else:
        print("Usage: python3 qir_disasm.py <path_to_bc_or_ll>")


def create_cutter_plugin():
    return None
