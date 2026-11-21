#!/usr/bin/env python3
"""
Quantum Intermediate Representation (QIR) Deobfuscator & Anti-Reversing Engine
Author: Matvei Igumnov <matvei@iqumnov.com>

Advanced Quantum Circuit DAG & Commutativity Deobfuscator:
1. Multi-pass Fixed-Point Optimization
2. Per-Qubit Wire Commutativity Tracking (Cancels interleaved gates across disjoint qubits)
3. Involutory Identity Reduction (H*H = I, X*X = I, CNOT*CNOT = I, CZ*CZ = I)
4. Phase Rotation Commutativity & Annihilation (Rz(θ1)*Rz(θ2) = Rz(θ1+θ2), cancel at 2πk)
5. Adjoint Inversion (S*S_adj = I, T*T_adj = I)
6. Dead / Phantom Qubit Injection Elimination
"""

import sys
import os
import math
import copy
from typing import List, Optional, Dict, Tuple, Set, Any

# Ensure local plugin directory is always in sys.path
_PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
if _PLUGIN_DIR not in sys.path:
    sys.path.insert(0, _PLUGIN_DIR)

try:
    from qir_parser import QuantumInstruction  # type: ignore
except ImportError:
    from qir_plugin.qir_parser import QuantumInstruction  # type: ignore


class ObfuscationFinding:
    """Represents a detected obfuscation artifact."""
    def __init__(
        self,
        pattern_type: str,
        description: str,
        gate_indices: Optional[List[int]] = None,
        severity: str = "MEDIUM"
    ) -> None:
        self.pattern_type: str = pattern_type
        self.description: str = description
        self.gate_indices: List[int] = gate_indices if gate_indices is not None else []
        self.severity: str = severity

    def __str__(self) -> str:
        return f"[{self.severity}] {self.pattern_type}: {self.description}"


class QIRDeobfuscator:
    """Analytical engine for detecting and eliminating quantum obfuscation."""
    def __init__(self) -> None:
        self.findings: List[ObfuscationFinding] = []
        self.cleaned_instructions: List[QuantumInstruction] = []
        self.obfuscation_ratio: float = 0.0

    def analyze(self, instructions: List[QuantumInstruction]) -> List[ObfuscationFinding]:
        """Full static analysis run to catalog all obfuscation artifacts."""
        self.findings = []
        insts: List[QuantumInstruction] = [copy.deepcopy(inst) for inst in instructions]

        # 1. Dead Qubits Analysis (Qubit Injection)
        measured_qubits: Set[int] = set()
        for inst in insts:
            if inst.op == 'mz':
                measured_qubits.update(inst.qubits)

        influencing_qubits: Set[int] = set(measured_qubits)
        changed: bool = True
        while changed:
            changed = False
            for inst in insts:
                if inst.op in ('cx', 'cnot', 'cz', 'swap', 'ccx'):
                    q_set: Set[int] = set(inst.qubits)
                    if q_set & influencing_qubits and not q_set.issubset(influencing_qubits):
                        influencing_qubits.update(q_set)
                        changed = True

        all_qubits: Set[int] = set()
        for inst in insts:
            all_qubits.update(inst.qubits)

        dead_qubits: Set[int] = all_qubits - influencing_qubits
        if dead_qubits:
            for dq in sorted(dead_qubits):
                dead_indices: List[int] = [idx for idx, inst in enumerate(insts) if dq in inst.qubits]
                self.findings.append(ObfuscationFinding(
                    pattern_type="DEAD_QUBIT_INJECTION",
                    description=f"Qubit q[{dq}] undergoes operations but is NEVER measured and never couples to measured observables.",
                    gate_indices=dead_indices,
                    severity="HIGH"
                ))

        # 2. Commuting Wire Analysis for Involutory & Inverse pairs
        # Tracks active gates per qubit wire to find commuting cancelations
        wire_active: Dict[Any, Optional[Tuple[int, QuantumInstruction]]] = {}
        for idx, inst in enumerate(insts):
            if inst.is_control_flow:
                continue

            # Check single qubit gate cancelation across commutativity
            if len(inst.qubits) == 1:
                q: int = inst.qubits[0]
                prev_info = wire_active.get(q)
                if prev_info:
                    p_idx, p_inst = prev_info
                    # Check if p_inst and inst cancel
                    if self._can_cancel(p_inst, inst):
                        self.findings.append(ObfuscationFinding(
                            pattern_type="IDENTITY_INVOLUTION_PAIR",
                            description=f"Commuting self-inverting pair: {p_inst.op.upper()} * {inst.op.upper()} = I on q[{q}].",
                            gate_indices=[p_idx, idx],
                            severity="MEDIUM"
                        ))
                        wire_active[q] = None
                        continue
                    elif self._can_cancel_rotations(p_inst, inst):
                        self.findings.append(ObfuscationFinding(
                            pattern_type="INVERSE_ROTATION_CANCELLATION",
                            description=f"Commuting inverse rotation: {p_inst.op.upper()}({p_inst.params[0]:.4f}) * {inst.op.upper()}({inst.params[0]:.4f}) = I on q[{q}].",
                            gate_indices=[p_idx, idx],
                            severity="MEDIUM"
                        ))
                        wire_active[q] = None
                        continue

                wire_active[q] = (idx, inst)

            elif len(inst.qubits) >= 2:
                # Two-qubit gate: check exact duplicate 2-qubit involution
                q_pair: Tuple[int, ...] = tuple(inst.qubits)
                p_info = wire_active.get(q_pair)
                if p_info:
                    p_idx, p_inst = p_info
                    if p_inst.op == inst.op:
                        self.findings.append(ObfuscationFinding(
                            pattern_type="IDENTITY_INVOLUTION_PAIR",
                            description=f"Redundant 2-qubit involution: {inst.op.upper()} * {inst.op.upper()} = I on qubits {inst.qubits}.",
                            gate_indices=[p_idx, idx],
                            severity="MEDIUM"
                        ))
                        wire_active[q_pair] = None
                        for q_idx in inst.qubits:
                            wire_active[q_idx] = None
                        continue

                wire_active[q_pair] = (idx, inst)
                # Invalidate individual qubit wires because of 2-qubit barrier
                for q_idx in inst.qubits:
                    wire_active[q_idx] = (idx, inst)

        # Compute Obfuscation Ratio
        obfuscated_indices: Set[int] = set()
        for f in self.findings:
            obfuscated_indices.update(f.gate_indices)

        total_gates: int = len(instructions)
        self.obfuscation_ratio = (len(obfuscated_indices) / total_gates * 100.0) if total_gates > 0 else 0.0

        return self.findings

    def _can_cancel(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Checks if two single-qubit gates cancel out to Identity."""
        if g1.qubits != g2.qubits:
            return False
        # Involutory: H*H=I, X*X=I, Y*Y=I, Z*Z=I
        if g1.op in ('h', 'x', 'y', 'z') and g1.op == g2.op:
            return True
        # Adjoint pairs: S*S_adj=I, T*T_adj=I
        if (g1.op == 's' and g2.op in ('s_adj', 'sdg')) or (g2.op == 's' and g1.op in ('s_adj', 'sdg')):
            return True
        if (g1.op == 't' and g2.op in ('t_adj', 'tdg')) or (g2.op == 't' and g1.op in ('t_adj', 'tdg')):
            return True
        return False

    def _can_cancel_rotations(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Checks if two rotation gates around the same axis cancel to 0 mod 2π."""
        if g1.qubits != g2.qubits or g1.op != g2.op or g1.op not in ('rx', 'ry', 'rz'):
            return False
        if not g1.params or not g2.params:
            return False
        theta_sum: float = g1.params[0] + g2.params[0]
        return abs(theta_sum) < 1e-4 or abs(theta_sum % (2 * math.pi)) < 1e-4

    def deobfuscate(self, instructions: List[QuantumInstruction]) -> List[QuantumInstruction]:
        """Multi-pass iterative circuit deobfuscation until fixed point."""
        self.analyze(instructions)

        current: List[QuantumInstruction] = [copy.deepcopy(inst) for inst in instructions]

        # 1. Eliminate Dead Qubits completely
        dead_findings = [f for f in self.findings if f.pattern_type == "DEAD_QUBIT_INJECTION"]
        dead_indices: Set[int] = set()
        for df in dead_findings:
            dead_indices.update(df.gate_indices)

        current = [inst for idx, inst in enumerate(current) if idx not in dead_indices]

        # 2. Iterative DAG-based Peephole Pass
        changed: bool = True
        pass_count: int = 0
        while changed and pass_count < 10:
            pass_count += 1
            changed = False
            i: int = 0
            while i < len(current) - 1:
                # Look ahead for commuting partner
                curr: QuantumInstruction = current[i]
                found_match: bool = False
                intervening_qubits: Set[int] = set()

                for j in range(i + 1, len(current)):
                    nxt: QuantumInstruction = current[j]

                    # If nxt shares any qubit with curr
                    shared: Set[int] = set(curr.qubits) & set(nxt.qubits)

                    if shared:
                        # They interact on the same qubit wire!
                        # Check if they cancel
                        if (len(curr.qubits) == 1 and len(nxt.qubits) == 1 and
                            curr.qubits == nxt.qubits):
                            if self._can_cancel(curr, nxt) or self._can_cancel_rotations(curr, nxt):
                                current.pop(j)
                                current.pop(i)
                                changed = True
                                found_match = True
                                break
                        elif (curr.op in ('cx', 'cnot', 'cz', 'swap') and
                              curr.op == nxt.op and curr.qubits == nxt.qubits):
                            current.pop(j)
                            current.pop(i)
                            changed = True
                            found_match = True
                            break
                        else:
                            # Interacting non-canceling gate: blocks further commutation
                            break
                    else:
                        # Disjoint qubits: nxt commutes past curr, continue searching wire
                        intervening_qubits.update(nxt.qubits)

                if found_match:
                    continue
                i += 1

        # Reassign sequential synthetic addresses
        addr: int = 0
        for inst in current:
            inst.address = addr
            addr += len(inst.synthetic_bytes)

        self.cleaned_instructions = current
        return self.cleaned_instructions

    def format_report(self) -> str:
        """Generates structured deobfuscation audit report."""
        lines: List[str] = []
        lines.append(f"; ═══════════════════════════════════════════════════════════════════════════")
        lines.append(f"; QIR Deobfuscation Audit Report")
        lines.append(f"; Obfuscation Severity Index: {self.obfuscation_ratio:.1f}%")
        lines.append(f"; Total Artifacts Detected   : {len(self.findings)}")
        lines.append(f"; ═══════════════════════════════════════════════════════════════════════════")
        lines.append(f"")

        if not self.findings:
            lines.append("  [OK] Zero obfuscation artifacts detected. Clean canonical quantum bytecode.")
        else:
            for idx, f in enumerate(self.findings, 1):
                lines.append(f"  {idx}. [{f.severity}] {f.pattern_type}")
                lines.append(f"     Details : {f.description}")
                lines.append(f"     Opcodes : {', '.join(f'#{i}' for i in f.gate_indices)}")
                lines.append("")

        lines.append(f"; ───────────────────────────────────────────────────────────────────────────")
        return "\n".join(lines)


if __name__ == "__main__":
    try:
        from qir_parser import QIRParser  # type: ignore
        from qir_disasm import QIRDisassembler  # type: ignore
    except ImportError:
        from qir_plugin.qir_parser import QIRParser  # type: ignore
        from qir_plugin.qir_disasm import QIRDisassembler  # type: ignore

    if len(sys.argv) > 1:
        parser = QIRParser()
        raw_insts = parser.parse_file(sys.argv[1])
        deobf = QIRDeobfuscator()
        findings = deobf.analyze(raw_insts)
        print(deobf.format_report())

        cleaned = deobf.deobfuscate(raw_insts)
        print(f"\n[+] Cleaned Instructions ({len(cleaned)} remaining of {len(raw_insts)}):")
        dis = QIRDisassembler(parser)
        print(dis.format_disassembly(cleaned, filepath=sys.argv[1] + " [DEOBFUSCATED]"))
        print()
        print(dis.render_circuit_diagram(cleaned))
    else:
        print("Usage: python3 qir_deobfuscator.py <path_to_bc_or_ll>")


def create_cutter_plugin():
    return None
