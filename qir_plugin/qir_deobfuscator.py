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

if __package__:
    from .qir_parser import QuantumInstruction  # type: ignore
else:
    _PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
    if _PLUGIN_DIR not in sys.path:
        sys.path.insert(0, _PLUGIN_DIR)
    from qir_parser import QuantumInstruction  # type: ignore


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


_UNITARY_SINGLE = frozenset({
    'h', 'x', 'y', 'z', 's', 'sdg', 's_adj', 's_dag',
    't', 'tdg', 't_adj', 't_dag', 'rx', 'ry', 'rz',
})
_UNITARY_MULTI = frozenset({'cx', 'cnot', 'cz', 'swap', 'ccx', 'toffoli'})
_UNITARY_OPS = _UNITARY_SINGLE | _UNITARY_MULTI
_Z_FAMILY = frozenset({'z', 's', 'sdg', 's_adj', 's_dag', 't', 'tdg', 't_adj', 't_dag', 'rz'})
_X_FAMILY = frozenset({'x', 'rx'})
_Y_FAMILY = frozenset({'y', 'ry'})
_Z8_MAP = {'t': 1, 's': 2, 'z': 4, 'sdg': 6, 's_adj': 6, 's_dag': 6, 'tdg': 7, 't_adj': 7, 't_dag': 7}
_Z8_INV = {1: 't', 2: 's', 4: 'z', 6: 'sdg', 7: 'tdg'}


class QIRDeobfuscator:
    """Analytical engine for detecting and eliminating quantum obfuscation."""
    _ROT_EPS = 1e-6

    def __init__(self) -> None:
        self.findings: List[ObfuscationFinding] = []
        self.cleaned_instructions: List[QuantumInstruction] = []
        self.obfuscation_ratio: float = 0.0

    @classmethod
    def _is_identity_angle(cls, theta: float) -> bool:
        if not math.isfinite(theta):
            return False
        norm = theta % math.tau
        return norm < cls._ROT_EPS or abs(norm - math.tau) < cls._ROT_EPS

    @staticmethod
    def _wrap_angle_signed_pi(theta: float) -> float:
        norm = ((theta + math.pi) % math.tau) - math.pi
        if abs(norm + math.pi) < 1e-12:
            return math.pi
        return 0.0 if abs(norm) == 0.0 else norm

    def analyze(self, instructions: List[QuantumInstruction]) -> List[ObfuscationFinding]:
        """Full static analysis run to catalog all obfuscation artifacts."""
        self.deobfuscate(instructions)
        return self.findings

    def _can_cancel(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Checks if two single-qubit involutory gates cancel out to Identity (G^2 = I)."""
        if g1.qubits != g2.qubits:
            return False
        # Involutory: H*H=I, X*X=I, Y*Y=I, Z*Z=I
        return g1.op in ('h', 'x', 'y', 'z') and g1.op == g2.op

    def _multi_gate_key(self, op: str, qubits: List[int]) -> Optional[Tuple[Any, ...]]:
        """Canonical key for involutory multi-qubit gates."""
        if op in ('cx', 'cnot') and len(qubits) == 2:
            return ('cx', qubits[0], qubits[1])
        if op in ('cz', 'swap') and len(qubits) == 2:
            return (op, min(qubits[0], qubits[1]), max(qubits[0], qubits[1]))
        if op in ('ccx', 'toffoli') and len(qubits) == 3:
            c0, c1 = min(qubits[0], qubits[1]), max(qubits[0], qubits[1])
            return ('ccx', c0, c1, qubits[2])
        return None

    def _can_cancel_multi(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Checks if two multi-qubit involutory gates cancel to Identity."""
        k1 = self._multi_gate_key(g1.op, g1.qubits)
        k2 = self._multi_gate_key(g2.op, g2.qubits)
        return k1 is not None and k1 == k2

    def _can_cancel_rotations(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Checks if two rotation gates around the same axis cancel to 0 mod 2π."""
        if g1.qubits != g2.qubits or g1.op != g2.op or g1.op not in ('rx', 'ry', 'rz'):
            return False
        if not g1.params or not g2.params:
            return False
        t1, t2 = g1.params[0], g2.params[0]
        if not (math.isfinite(t1) and math.isfinite(t2)):
            return False
        theta_sum: float = t1 + t2
        return math.isfinite(theta_sum) and self._is_identity_angle(theta_sum)

    def _try_consolidate_discrete_phase(
        self, g1: QuantumInstruction, g2: QuantumInstruction
    ) -> Tuple[bool, Optional[QuantumInstruction]]:
        """Consolidate Z_8 discrete phase gates on the same qubit; returns (matched, replacement_or_none)."""
        if len(g1.qubits) != 1 or g1.qubits != g2.qubits:
            return (False, None)
        v1 = _Z8_MAP.get(g1.op)
        v2 = _Z8_MAP.get(g2.op)
        if v1 is None or v2 is None:
            return (False, None)
        s = (v1 + v2) % 8
        if s == 0:
            return (True, None)
        new_op = _Z8_INV.get(s)
        if new_op is None:
            return (False, None)
        return (True, QuantumInstruction(new_op, qubits=list(g1.qubits), raw_call=f"{new_op} q[{g1.qubits[0]}]"))

    def _try_merge_rotations(
        self, g1: QuantumInstruction, g2: QuantumInstruction
    ) -> Tuple[bool, Optional[QuantumInstruction]]:
        """Merge two rotations along the same axis/qubit; returns (matched, merged_or_none)."""
        if g1.qubits != g2.qubits or g1.op != g2.op or g1.op not in ('rx', 'ry', 'rz'):
            return (False, None)
        if not g1.params or not g2.params:
            return (False, None)
        t1, t2 = g1.params[0], g2.params[0]
        if not (math.isfinite(t1) and math.isfinite(t2)):
            return (False, None)
        total = t1 + t2
        if not math.isfinite(total):
            return (False, None)
        if self._is_identity_angle(total):
            return (True, None)
        norm = self._wrap_angle_signed_pi(total)
        return (True, QuantumInstruction(
            g1.op, qubits=list(g1.qubits), params=[norm], raw_call=f"{g1.op}({norm:.6f}) q[{g1.qubits[0]}]"
        ))

    def _gates_interfere(self, g1: QuantumInstruction, g2: QuantumInstruction) -> bool:
        """Returns True if g1 and g2 do NOT commute."""
        if g1.is_control_flow or g2.is_control_flow:
            return True
        if (g1.op == 'barrier' and not g1.qubits) or (g2.op == 'barrier' and not g2.qubits):
            return True
        if g1.op not in _UNITARY_OPS or g2.op not in _UNITARY_OPS:
            if (g1.op == 'barrier' or g2.op == 'barrier') and not (set(g1.qubits) & set(g2.qubits)):
                return False
            return True
        shared = set(g1.qubits) & set(g2.qubits)
        if not shared:
            return False
        for q in shared:
            if g1.op in _Z_FAMILY and g2.op in _Z_FAMILY:
                continue
            if g1.op in _X_FAMILY and g2.op in _X_FAMILY:
                continue
            if g1.op in _Y_FAMILY and g2.op in _Y_FAMILY:
                continue
            if g1.op in ('cx', 'cnot') and len(g1.qubits) == 2 and g1.qubits[0] != g1.qubits[1]:
                if g2.op in _Z_FAMILY and q == g1.qubits[0]:
                    continue
                if g2.op in _X_FAMILY and q == g1.qubits[1]:
                    continue
            if g2.op in ('cx', 'cnot') and len(g2.qubits) == 2 and g2.qubits[0] != g2.qubits[1]:
                if g1.op in _Z_FAMILY and q == g2.qubits[0]:
                    continue
                if g1.op in _X_FAMILY and q == g2.qubits[1]:
                    continue
            if g1.op == 'cz' and g2.op == 'cz':
                continue
            if g1.op == 'cz' and g2.op in _Z_FAMILY:
                continue
            if g2.op == 'cz' and g1.op in _Z_FAMILY:
                continue
            if (
                g1.op in ('cx', 'cnot') and g2.op in ('cx', 'cnot')
                and len(g1.qubits) == 2 and len(g2.qubits) == 2
                and g1.qubits[0] != g1.qubits[1] and g2.qubits[0] != g2.qubits[1]
            ):
                if g1.qubits[0] == g2.qubits[0] or g1.qubits[1] == g2.qubits[1]:
                    continue
            if g1.op in ('ccx', 'toffoli') and len(g1.qubits) == 3 and len(set(g1.qubits)) == 3:
                if g2.op in _Z_FAMILY and q in (g1.qubits[0], g1.qubits[1]):
                    continue
                if g2.op in _X_FAMILY and q == g1.qubits[2]:
                    continue
            if g2.op in ('ccx', 'toffoli') and len(g2.qubits) == 3 and len(set(g2.qubits)) == 3:
                if g1.op in _Z_FAMILY and q in (g2.qubits[0], g2.qubits[1]):
                    continue
                if g1.op in _X_FAMILY and q == g2.qubits[2]:
                    continue
            if (
                g1.op in ('ccx', 'toffoli') and g2.op in ('ccx', 'toffoli')
                and len(g1.qubits) == 3 and len(g2.qubits) == 3
                and len(set(g1.qubits)) == 3 and len(set(g2.qubits)) == 3
            ):
                if {g1.qubits[0], g1.qubits[1]} == {g2.qubits[0], g2.qubits[1]}:
                    continue
            return True
        return False

    def deobfuscate(self, instructions: List[QuantumInstruction]) -> List[QuantumInstruction]:
        """Multi-pass iterative circuit deobfuscation until fixed point."""
        self.findings = []
        orig_copy: List[QuantumInstruction] = [copy.deepcopy(inst) for inst in instructions]
        orig_gate_count = sum(1 for inst in orig_copy if not inst.is_control_flow)
        if orig_gate_count == 0:
            self.obfuscation_ratio = 0.0
            self.cleaned_instructions = orig_copy
            return self.cleaned_instructions

        # 1. Eliminate Dead Qubits via backwards causal entanglement cone
        measured_qubits: Set[int] = set()
        for inst in orig_copy:
            if not inst.is_control_flow and (inst.op in ('mz', 'mresetz') or inst.results):
                measured_qubits.update(inst.qubits)

        dead_qubits: Set[int] = set()
        if measured_qubits:
            influencing_qubits: Set[int] = set(measured_qubits)
            changed_cone: bool = True
            while changed_cone:
                changed_cone = False
                for inst in reversed(orig_copy):
                    if (
                        not inst.is_control_flow
                        and inst.op not in ('barrier', 'qalloc', 'qalloc_array', 'qfree', 'record_output')
                        and len(inst.qubits) >= 2
                    ):
                        q_set: Set[int] = set(inst.qubits)
                        if q_set & influencing_qubits and not q_set.issubset(influencing_qubits):
                            influencing_qubits.update(q_set)
                            changed_cone = True

            all_qubits: Set[int] = set()
            for inst in orig_copy:
                if not inst.is_control_flow:
                    all_qubits.update(inst.qubits)
            dead_qubits = all_qubits - influencing_qubits

            for dq in sorted(dead_qubits):
                dead_indices = [
                    idx for idx, inst in enumerate(orig_copy)
                    if not inst.is_control_flow and inst.op != 'barrier' and dq in inst.qubits
                ]
                if not dead_indices:
                    dead_indices = [
                        idx for idx, inst in enumerate(orig_copy)
                        if not inst.is_control_flow and inst.op == 'barrier' and dq in inst.qubits
                    ]
                self.findings.append(ObfuscationFinding(
                    pattern_type="DEAD_QUBIT_INJECTION",
                    description=f"Qubit q[{dq}] undergoes operations but is NEVER measured and never couples to measured observables.",
                    gate_indices=dead_indices,
                    severity="HIGH",
                ))

        indexed_stream: List[Tuple[int, QuantumInstruction]] = []
        for idx, inst in enumerate(orig_copy):
            if inst.is_control_flow:
                indexed_stream.append((idx, inst))
            elif inst.op == 'barrier':
                if not inst.qubits:
                    indexed_stream.append((idx, inst))
                else:
                    live_qs = [q for q in inst.qubits if q not in dead_qubits]
                    if live_qs:
                        inst.qubits = live_qs
                        indexed_stream.append((idx, inst))
            elif not any(q in dead_qubits for q in inst.qubits):
                indexed_stream.append((idx, inst))

        # 2. Iterative DAG-based Peephole Pass until true fixed point
        changed: bool = True
        while changed:
            changed = False
            i: int = 0
            while i < len(indexed_stream):
                orig_i, curr = indexed_stream[i]
                found_match: bool = False

                if curr.is_control_flow or curr.op not in _UNITARY_OPS:
                    i += 1
                    continue

                if curr.op in ('rx', 'ry', 'rz') and curr.params and self._is_identity_angle(curr.params[0]):
                    self.findings.append(ObfuscationFinding(
                        pattern_type="TRIVIAL_IDENTITY_ROTATION",
                        description=f"Trivial identity rotation (angle ≡ 0 mod 2π) removed: {curr.mnemonic()} = I",
                        gate_indices=[orig_i],
                        severity="HIGH",
                    ))
                    indexed_stream.pop(i)
                    changed = True
                    break

                for j in range(i + 1, len(indexed_stream)):
                    orig_j, nxt = indexed_stream[j]

                    # Never commute quantum gates across control-flow or non-barrier non-unitary boundaries
                    if nxt.is_control_flow:
                        break
                    if nxt.op not in _UNITARY_OPS:
                        if self._gates_interfere(curr, nxt):
                            break
                        continue

                    if self._can_cancel(curr, nxt) or self._can_cancel_multi(curr, nxt):
                        self.findings.append(ObfuscationFinding(
                            pattern_type="IDENTITY_INVOLUTION_PAIR",
                            description=f"Commuting self-inverting pair: {curr.mnemonic()} · {nxt.mnemonic()} = I",
                            gate_indices=[orig_i, orig_j],
                            severity="HIGH",
                        ))
                        indexed_stream.pop(j)
                        indexed_stream.pop(i)
                        changed = True
                        found_match = True
                        break

                    if self._can_cancel_rotations(curr, nxt):
                        self.findings.append(ObfuscationFinding(
                            pattern_type="INVERSE_ROTATION_CANCELLATION",
                            description=f"Commuting inverse rotation: {curr.mnemonic()} · {nxt.mnemonic()} = I",
                            gate_indices=[orig_i, orig_j],
                            severity="HIGH",
                        ))
                        indexed_stream.pop(j)
                        indexed_stream.pop(i)
                        changed = True
                        found_match = True
                        break

                    if len(curr.qubits) == 1 and curr.qubits == nxt.qubits:
                        z8_matched, z8_repl = self._try_consolidate_discrete_phase(curr, nxt)
                        if z8_matched:
                            if z8_repl is None:
                                self.findings.append(ObfuscationFinding(
                                    pattern_type="DISCRETE_PHASE_CANCELLATION",
                                    description=f"Discrete phase inverse cancellation: {curr.mnemonic()} · {nxt.mnemonic()} = I",
                                    gate_indices=[orig_i, orig_j],
                                    severity="HIGH",
                                ))
                                indexed_stream.pop(j)
                                indexed_stream.pop(i)
                            else:
                                self.findings.append(ObfuscationFinding(
                                    pattern_type="DISCRETE_PHASE_CONSOLIDATION",
                                    description=f"Consolidated discrete Clifford+T phases: {curr.mnemonic()} · {nxt.mnemonic()} -> {z8_repl.mnemonic()}",
                                    gate_indices=[orig_i, orig_j],
                                    severity="MEDIUM",
                                ))
                                indexed_stream[i] = (orig_i, z8_repl)
                                indexed_stream.pop(j)
                            changed = True
                            found_match = True
                            break

                        rot_matched, merged_rot = self._try_merge_rotations(curr, nxt)
                        if rot_matched:
                            if merged_rot is None:
                                self.findings.append(ObfuscationFinding(
                                    pattern_type="INVERSE_ROTATION_CANCELLATION",
                                    description=f"Commuting inverse rotation: {curr.mnemonic()} · {nxt.mnemonic()} = I",
                                    gate_indices=[orig_i, orig_j],
                                    severity="HIGH",
                                ))
                                indexed_stream.pop(j)
                                indexed_stream.pop(i)
                            else:
                                self.findings.append(ObfuscationFinding(
                                    pattern_type="CONTINUOUS_ROTATION_CONSOLIDATION",
                                    description=f"Consolidated rotations: {curr.mnemonic()} + {nxt.mnemonic()} -> {merged_rot.mnemonic()}",
                                    gate_indices=[orig_i, orig_j],
                                    severity="MEDIUM",
                                ))
                                indexed_stream[i] = (orig_i, merged_rot)
                                indexed_stream.pop(j)
                            changed = True
                            found_match = True
                            break

                    if self._gates_interfere(curr, nxt):
                        break

                if found_match:
                    break
                i += 1

        current = [inst for _, inst in indexed_stream]
        clean_gate_count = sum(1 for inst in current if not inst.is_control_flow)
        self.obfuscation_ratio = ((orig_gate_count - clean_gate_count) / orig_gate_count) * 100.0

        # Reassign sequential synthetic addresses and ensure merged instructions have valid 4-byte opcodes
        addr: int = 0
        for inst in current:
            if not inst.synthetic_bytes:
                inst.synthetic_bytes = self._encode_synthetic_bytes(inst)
            inst.address = addr
            if not (inst.is_control_flow and inst.label):
                addr += len(inst.synthetic_bytes)

        self.cleaned_instructions = current
        return self.cleaned_instructions

    @staticmethod
    def _encode_synthetic_bytes(inst: QuantumInstruction) -> bytes:
        """Encodes a 4-byte synthetic opcode for reconstructed or merged instructions."""
        if __package__:
            from .qir_parser import QIRParser  # type: ignore
        else:
            from qir_parser import QIRParser  # type: ignore
        return QIRParser.encode_synthetic_bytes(inst)

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
        cleaned = deobf.deobfuscate(raw_insts)
        print(deobf.format_report())

        n_clean = sum(1 for i in cleaned if not i.is_control_flow)
        n_raw = sum(1 for i in raw_insts if not i.is_control_flow)
        print(f"\n[+] Cleaned Instructions ({n_clean} remaining of {n_raw}):")
        dis = QIRDisassembler(parser)
        print(dis.format_disassembly(cleaned, filepath=sys.argv[1] + " [DEOBFUSCATED]"))
        print()
        print(dis.render_circuit_diagram(cleaned))
    else:
        print("Usage: python3 qir_deobfuscator.py <path_to_bc_or_ll>")


def create_cutter_plugin():
    return None
