#!/usr/bin/env python3
"""
Quantum Intermediate Representation (QIR) Disassembler & Circuit Graph Engine
Author: Matvei Igumnov <matvei@iqumnov.com>

Translates binary QIR Bytecode / LLVM IR into:
1. Formatted Disassembly Listing (Address | Hex | Mnemonic | Semantic Comment)
2. ASCII 2D Quantum Circuit Matrix Diagram
3. OpenQASM 3.0 standard export (.qasm)
"""

import sys
import os
import math
from typing import List, Optional, Any, Dict, Tuple

if __package__:
    from .qir_parser import QIRParser, QuantumInstruction  # type: ignore
else:
    _PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
    if _PLUGIN_DIR not in sys.path:
        sys.path.insert(0, _PLUGIN_DIR)
    from qir_parser import QIRParser, QuantumInstruction  # type: ignore


MAX_RENDERABLE_QUBITS = 256


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
        lines.append(f"; QIR Disassembly Listing")
        lines.append(f"; Target File      : {fname}")
        lines.append(f"; Quantum Profile  : Custom QIR Profile (Profile: Base / Adaptive)")
        lines.append(f"; Allocated Qubits : {self.parser.num_qubits}")
        lines.append(f"; Results Registers: {self.parser.num_results}")
        lines.append(f"; Entry Point      : @{self.parser.entry_point or 'main'}")
        opcode_count = sum(1 for inst in instructions if not inst.is_control_flow)
        lines.append(f"; Total Opcodes    : {opcode_count}")
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
        q0 = inst.qubits[0] if len(inst.qubits) > 0 else 0
        q1 = inst.qubits[1] if len(inst.qubits) > 1 else 0
        q2 = inst.qubits[2] if len(inst.qubits) > 2 else 0
        r0 = inst.results[0] if len(inst.results) > 0 else 0
        if inst.op == 'h':
            return f"Superposition: |0> -> (|0>+|1>)/√2 on q[{q0}]"
        elif inst.op in ('cx', 'cnot'):
            return f"Entanglement: Controlled-NOT (ctrl=q[{q0}], tgt=q[{q1}])"
        elif inst.op == 'cz':
            return f"Controlled-Z phase flip (ctrl=q[{q0}], tgt=q[{q1}])"
        elif inst.op == 'swap':
            return f"SWAP quantum state between q[{q0}] and q[{q1}]"
        elif inst.op in ('ccx', 'toffoli'):
            return f"Toffoli (CCX) gate on ({q0}, {q1}) -> tgt={q2}"
        elif inst.op == 'x':
            return f"Pauli-X (NOT / Bit-Flip) on q[{q0}]"
        elif inst.op == 'y':
            return f"Pauli-Y (Bit+Phase Flip) on q[{q0}]"
        elif inst.op == 'z':
            return f"Pauli-Z (Phase-Flip) on q[{q0}]"
        elif inst.op == 's':
            return f"Phase S gate (+π/2 phase) on q[{q0}]"
        elif inst.op in ('s_adj', 's_dag', 'sdg'):
            return f"Adjoint Phase S† gate (-π/2 phase) on q[{q0}]"
        elif inst.op == 't':
            return f"Phase T gate (+π/4 phase) on q[{q0}]"
        elif inst.op in ('t_adj', 't_dag', 'tdg'):
            return f"Adjoint Phase T† gate (-π/4 phase) on q[{q0}]"
        elif inst.op == 'rz':
            theta = inst.params[0] if inst.params else 0.0
            return f"Phase Rotation Rz({theta:.4f} rad) on q[{q0}]"
        elif inst.op == 'rx':
            theta = inst.params[0] if inst.params else 0.0
            return f"Rotation Rx({theta:.4f} rad) on q[{q0}]"
        elif inst.op == 'ry':
            theta = inst.params[0] if inst.params else 0.0
            return f"Rotation Ry({theta:.4f} rad) on q[{q0}]"
        elif inst.op in ('mz', 'mresetz'):
            return f"Projective Z-measurement: collapse q[{q0}] -> classical r[{r0}]"
        elif inst.op == 'reset':
            return f"State Reset: project q[{q0}] back to ground state |0>"
        elif inst.op == 'barrier':
            return f"Circuit scheduling barrier on {len(inst.qubits)} qubit(s)"
        elif inst.op in ('qalloc', 'qalloc_array'):
            return f"Runtime qubit allocation -> q[{q0}]"
        elif inst.op == 'qfree':
            return f"Runtime qubit deallocation for q[{q0}]"
        elif inst.op == 'record_output':
            return f"Record measurement result r[{r0}]"
        elif inst.op == 'read_result':
            return f"Read classical measurement result r[{r0}]"
        return f"Custom quantum intrinsic: {inst.op}"

    def render_circuit_diagram(self, instructions: Optional[List[QuantumInstruction]] = None) -> str:
        """Builds 2D ASCII Quantum Circuit Matrix diagram."""
        target_insts: List[QuantumInstruction] = instructions if instructions is not None else self.parser.instructions

        max_q = max((max(inst.qubits) + 1 for inst in target_insts if inst.qubits), default=0)
        max_r = max((max(inst.results) + 1 for inst in target_insts if inst.results), default=0)
        if instructions is None:
            num_q: int = max(self.parser.num_qubits, max_q, 1)
            num_r: int = max(self.parser.num_results, max_r, 1)
        else:
            num_q = max(max_q, 1)
            num_r = max(max_r, 1)

        if num_q > MAX_RENDERABLE_QUBITS or num_r > MAX_RENDERABLE_QUBITS:
            return (
                f"; Register count (qubits={num_q}, results={num_r}) "
                f"exceeds ASCII diagram rendering limit ({MAX_RENDERABLE_QUBITS})\n"
            )

        qubit_wires: List[List[str]] = [[] for _ in range(num_q)]

        for inst in target_insts:
            if inst.is_control_flow:
                continue

            if inst.op in ('h', 'x', 'y', 'z', 's', 't') and inst.qubits:
                q = inst.qubits[0]
                box = f"─[{inst.op.upper()[:1]}]─"
                for i in range(num_q):
                    qubit_wires[i].append(box if i == q else "─────")

            elif inst.op in ('s_adj', 's_dag', 'sdg') and inst.qubits:
                q = inst.qubits[0]
                for i in range(num_q):
                    qubit_wires[i].append("[S†]─" if i == q else "─────")

            elif inst.op in ('t_adj', 't_dag', 'tdg') and inst.qubits:
                q = inst.qubits[0]
                for i in range(num_q):
                    qubit_wires[i].append("[T†]─" if i == q else "─────")

            elif inst.op == 'reset' and inst.qubits:
                q = inst.qubits[0]
                for i in range(num_q):
                    qubit_wires[i].append("[|0>]" if i == q else "─────")

            elif inst.op in ('rx', 'ry', 'rz') and inst.qubits:
                q = inst.qubits[0]
                box = f"[{inst.op.capitalize()}]─"
                for i in range(num_q):
                    qubit_wires[i].append(box if i == q else "─────")

            elif inst.op in ('cx', 'cnot', 'cz', 'swap') and len(inst.qubits) >= 2:
                q1, q2 = inst.qubits[0], inst.qubits[1]
                min_idx = min(q1, q2)
                max_idx = max(q1, q2)
                m1 = "──●──" if inst.op in ('cx', 'cnot', 'cz') else "──×──"
                m2 = "──X──" if inst.op in ('cx', 'cnot') else ("──Z──" if inst.op == 'cz' else "──×──")
                for i in range(num_q):
                    if i == q1:
                        qubit_wires[i].append(m1)
                    elif i == q2:
                        qubit_wires[i].append(m2)
                    elif min_idx < i < max_idx:
                        qubit_wires[i].append("──┼──")
                    else:
                        qubit_wires[i].append("─────")

            elif inst.op in ('ccx', 'toffoli') and len(inst.qubits) >= 3:
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

            elif inst.op in ('mz', 'mresetz') and inst.qubits:
                q = inst.qubits[0]
                for i in range(num_q):
                    qubit_wires[i].append("─[M]─" if i == q else "─────")
            elif inst.op == 'barrier':
                for i in range(num_q):
                    if not inst.qubits or i in inst.qubits:
                        qubit_wires[i].append("──║──")
                    else:
                        qubit_wires[i].append("─────")
            else:
                for i in range(num_q):
                    qubit_wires[i].append("─[?]─" if i in inst.qubits else "─────")

        # Format rows with aligned register labels
        lbl_w = max(len(f"q[{num_q - 1}]: "), len(f"r[{num_r - 1}]: "))
        diagram_lines: List[str] = []
        diagram_lines.append("; ── QUANTUM CIRCUIT TIMELINE DIAGRAM ──────────────────────────────────────")
        for i in range(num_q):
            prefix = f"q[{i}]: ".ljust(lbl_w) + "──"
            wire_str = "─".join(qubit_wires[i]) if qubit_wires[i] else "────────────────────"
            diagram_lines.append(f"{prefix}{wire_str}──")

        # Classical registers
        has_mz = any(inst.op in ('mz', 'mresetz') or inst.results for inst in target_insts)
        if has_mz:
            wire_width = max((len(line) for line in diagram_lines[1:]), default=20)
            for j in range(num_r):
                prefix = f"r[{j}]: ".ljust(lbl_w) + "══"
                c_wire = "═" * max(20, wire_width - len(prefix))
                diagram_lines.append(f"{prefix}{c_wire}")

        diagram_lines.append("; ──────────────────────────────────────────────────────────────────────────")
        return "\n".join(diagram_lines)

    def export_openqasm(self, instructions: Optional[List[QuantumInstruction]] = None, out_path: Optional[str] = None) -> str:
        """Exports decompiled quantum operations to standard OpenQASM 3.0 format."""
        target_insts: List[QuantumInstruction] = instructions if instructions is not None else self.parser.instructions

        max_q = max((max(inst.qubits) + 1 for inst in target_insts if inst.qubits), default=0)
        max_r_explicit = max((max(inst.results) + 1 for inst in target_insts if inst.results), default=0)
        max_r_mresetz = max(
            (inst.qubits[0] + 1 for inst in target_insts if inst.op == 'mresetz' and not inst.results and inst.qubits),
            default=0,
        )
        max_r = max(max_r_explicit, max_r_mresetz)
        if instructions is None:
            num_q: int = max(self.parser.num_qubits, max_q, 1)
            num_r: int = max(self.parser.num_results, max_r)
        else:
            num_q = max(max_q, 1)
            num_r = max_r

        lines: List[str] = [
            "// OpenQASM 3.0 generated by QIR Reversing",
            "OPENQASM 3.0;",
            'include "stdgates.inc";',
            "",
            f"qubit[{num_q}] q;",
        ]
        if num_r > 0:
            lines.append(f"bit[{num_r}] r;")
        lines.append("")

        for inst in target_insts:
            if inst.is_control_flow:
                cf = inst.raw_call.replace("\r", " ").replace("\n", " ").strip()
                if cf:
                    lines.append(f"// control-flow: {cf}")
                continue
            if inst.op in ('h', 'x', 'y', 'z', 's', 't') and inst.qubits:
                lines.append(f"{inst.op} q[{inst.qubits[0]}];")
            elif inst.op in ('s_adj', 's_dag', 'sdg') and inst.qubits:
                lines.append(f"sdg q[{inst.qubits[0]}];")
            elif inst.op in ('t_adj', 't_dag', 'tdg') and inst.qubits:
                lines.append(f"tdg q[{inst.qubits[0]}];")
            elif inst.op in ('cx', 'cnot') and len(inst.qubits) >= 2:
                lines.append(f"cx q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op == 'cz' and len(inst.qubits) >= 2:
                lines.append(f"cz q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op == 'swap' and len(inst.qubits) >= 2:
                lines.append(f"swap q[{inst.qubits[0]}], q[{inst.qubits[1]}];")
            elif inst.op in ('ccx', 'toffoli') and len(inst.qubits) >= 3:
                lines.append(f"ccx q[{inst.qubits[0]}], q[{inst.qubits[1]}], q[{inst.qubits[2]}];")
            elif inst.op in ('rx', 'ry', 'rz') and inst.qubits:
                param = inst.params[0] if inst.params else 0.0
                if not math.isfinite(param):
                    lines.append(f"// invalid non-finite {inst.op} q[{inst.qubits[0]}];")
                else:
                    if abs(param) == 0.0:
                        param = 0.0
                    lines.append(f"{inst.op}({param}) q[{inst.qubits[0]}];")
            elif inst.op == 'mz' and inst.qubits:
                r_idx = inst.results[0] if inst.results else 0
                lines.append(f"r[{r_idx}] = measure q[{inst.qubits[0]}];")
            elif inst.op == 'mresetz' and inst.qubits:
                r_idx = inst.results[0] if inst.results else inst.qubits[0]
                lines.append(f"r[{r_idx}] = measure q[{inst.qubits[0]}];")
                lines.append(f"reset q[{inst.qubits[0]}];")
            elif inst.op == 'reset' and inst.qubits:
                lines.append(f"reset q[{inst.qubits[0]}];")
            elif inst.op == 'barrier':
                if inst.qubits:
                    lines.append("barrier " + ", ".join(f"q[{q}]" for q in inst.qubits) + ";")
                else:
                    lines.append("barrier;")
            elif inst.op not in ('qalloc', 'qalloc_array', 'qfree', 'record_output'):
                qs = ", ".join(f"q[{q}]" for q in inst.qubits)
                lines.append(f"// {inst.op} {qs}".rstrip())

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
