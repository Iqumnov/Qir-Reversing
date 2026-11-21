#!/usr/bin/env python3
"""
QIR (Quantum Intermediate Representation) LLVM Bytecode & IR Parser
Author: Matvei Igumnov <matvei@iqumnov.com>

Enhanced Multi-Architecture Parser:
- Parses binary LLVM Bitcode (.bc) and textual LLVM IR (.ll)
- Extracts quantum gate calls (@__quantum__qis__*)
- Extracts runtime lifecycle calls (@__quantum__rt__*)
- Supports 1-qubit, 2-qubit, and 3-qubit (Toffoli/CCX) gates
- Tracks adjoint/conjugate gates (s_adj, t_adj)
- Identifies basic blocks and classical control flow (br, ret)
"""

import os
import re
import struct
from typing import List, Optional, Any, Dict, Tuple

try:
    import pyqir
except ImportError:
    pyqir = None


class QuantumInstruction:
    """Represents a decoded quantum gate, runtime operation, or control-flow node."""
    def __init__(
        self,
        op: str,
        qubits: Optional[List[int]] = None,
        results: Optional[List[int]] = None,
        params: Optional[List[float]] = None,
        raw_call: str = "",
        is_control_flow: bool = False,
        label: str = ""
    ) -> None:
        self.op: str = op                                    # e.g., 'h', 'x', 'y', 'z', 'cx', 'rz', 'mz', 'qalloc', 'br'
        self.qubits: List[int] = list(qubits) if qubits is not None else []
        self.results: List[int] = list(results) if results is not None else []
        self.params: List[float] = list(params) if params is not None else []
        self.raw_call: str = str(raw_call)
        self.is_control_flow: bool = bool(is_control_flow)
        self.label: str = str(label)
        self.address: int = 0
        self.synthetic_bytes: bytes = b""

    def mnemonic(self) -> str:
        """Returns clean OpenQASM / QIR-ASM mnemonic."""
        if self.is_control_flow:
            if self.label:
                return f"{self.label}:"
            return f"{self.op}"

        if self.op in ('h', 'x', 'y', 'z', 's', 't'):
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"{self.op} {q}"
        elif self.op in ('s_adj', 's_dag', 'sdg'):
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"sdg {q}"
        elif self.op in ('t_adj', 't_dag', 'tdg'):
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"tdg {q}"
        elif self.op in ('cnot', 'cx'):
            return f"cx q[{self.qubits[0]}], q[{self.qubits[1]}]"
        elif self.op == 'cz':
            return f"cz q[{self.qubits[0]}], q[{self.qubits[1]}]"
        elif self.op == 'swap':
            return f"swap q[{self.qubits[0]}], q[{self.qubits[1]}]"
        elif self.op in ('ccx', 'toffoli'):
            return f"ccx q[{self.qubits[0]}], q[{self.qubits[1]}], q[{self.qubits[2]}]"
        elif self.op in ('rx', 'ry', 'rz'):
            param_str = f"{self.params[0]:.6f}" if self.params else "0.0"
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"{self.op}({param_str}) {q}"
        elif self.op == 'mz':
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"mz {q} -> {r}"
        elif self.op == 'qalloc':
            return f"qalloc -> q[{self.qubits[0]}]"
        elif self.op == 'qfree':
            return f"qfree q[{self.qubits[0]}]"
        elif self.op == 'record_output':
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"record_output {r}"
        elif self.op == 'read_result':
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"read_result {r}"
        elif self.op == 'reset':
            return f"reset q[{self.qubits[0]}]"
        elif self.op == 'barrier':
            return "barrier"
        return f"{self.op} " + ", ".join(f"q[{q}]" for q in self.qubits)


class QIRParser:
    """Full parser for LLVM QIR Bytecode and Textual IR."""
    def __init__(self) -> None:
        self.module_name: str = ""
        self.num_qubits: int = 0
        self.num_results: int = 0
        self.entry_point: str = ""
        self.instructions: List[QuantumInstruction] = []

    @property
    def quantum_gates(self) -> List[QuantumInstruction]:
        """Returns only quantum operations, excluding classical control flow labels."""
        return [i for i in self.instructions if not i.is_control_flow]

    def parse_file(self, filepath: str) -> List[QuantumInstruction]:
        """Dispatches to bitcode or text parser based on file signature."""
        with open(filepath, "rb") as f:
            header = f.read(4)

        if header == b"\x42\x43\xc0\xde" or header.startswith(b"BC"):
            return self.parse_bitcode(filepath)
        else:
            return self.parse_text_ir(filepath)

    def parse_bitcode(self, filepath: str) -> List[QuantumInstruction]:
        """Parses binary LLVM bitcode (.bc) utilizing PyQIR AST reader."""
        try:
            if pyqir is not None:
                with open(filepath, "rb") as f:
                    bc_bytes = f.read()
                mod = pyqir.Module.from_bitcode(pyqir.Context(), bc_bytes)
                return self._parse_pyqir_module(mod)
        except Exception:
            pass

        # Fallback 1: check if companion textual .ll exists
        ll_cand = os.path.splitext(filepath)[0] + ".ll"
        if os.path.exists(ll_cand):
            return self.parse_text_ir(ll_cand)

        # Fallback 2: llvm-dis CLI
        try:
            import subprocess
            res = subprocess.run(["llvm-dis", "-o", "-", filepath], stdout=subprocess.PIPE, text=True)
            if res.returncode == 0:
                return self.parse_text_string(res.stdout)
        except Exception:
            pass

        # Fallback 3: binary string extraction
        try:
            with open(filepath, "rb") as f:
                raw_bytes = f.read()
            extracted = "".join(chr(b) if 32 <= b < 127 or b in (10, 13, 9) else " " for b in raw_bytes)
            if "@__quantum__qis__" in extracted or "quantum" in extracted:
                return self.parse_text_string(extracted)
        except Exception:
            pass

        raise RuntimeError("Failed to parse LLVM bitcode: PyQIR reader not available.")

    def parse_text_ir(self, filepath: str) -> List[QuantumInstruction]:
        """Parses textual LLVM IR (.ll)."""
        with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
            content = f.read()
        return self.parse_text_string(content)

    def _parse_pyqir_module(self, mod: Any) -> List[QuantumInstruction]:
        """Extracts quantum structure from PyQIR Module object."""
        self.module_name = mod.source_filename or "qir_module"
        if mod.functions:
            try:
                self.num_qubits = pyqir.required_num_qubits(mod.functions[0]) or 0
                self.num_results = pyqir.required_num_results(mod.functions[0]) or 0
            except Exception:
                pass
        return self.parse_text_string(str(mod))

    def _parse_qubit_id(self, arg_str: str) -> int:
        """Extracts integer qubit index from LLVM pointer arg."""
        if "null" in arg_str:
            return 0
        m = re.search(r'inttoptr\s*\(\s*i64\s+(\d+)\s+to\s+ptr\)', arg_str)
        if m:
            return int(m.group(1))
        m = re.search(r'%[Qq](?:ubit)?(\d+)', arg_str)
        if m:
            return int(m.group(1))
        # Match standalone number after %
        m = re.search(r'%(\d+)', arg_str)
        if m:
            return int(m.group(1))
        # Match literal number
        m = re.search(r'\b(\d+)\b', arg_str)
        if m:
            val = int(m.group(1))
            if val != 64:  # Avoid matching i64 type
                return val
        return 0

    def _parse_float_param(self, arg_str: str) -> float:
        """Parses double parameter, including hex float literals (e.g. 0x3FE921FB54442D18)."""
        s: str = arg_str.strip()
        m_hex = re.search(r'0x[0-9a-fA-F]{16}', s)
        if m_hex:
            val_int = int(m_hex.group(0), 16)
            val_bytes = struct.pack(">Q", val_int)
            return float(struct.unpack(">d", val_bytes)[0])
        m_dec = re.search(r'[-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?', s)
        if m_dec:
            return float(m_dec.group(0))
        return 0.0

    def parse_text_string(self, content: str) -> List[QuantumInstruction]:
        """Regex and AST parser for LLVM QIR text lines."""
        self.instructions = []
        c_str: str = str(content)

        m_q = re.search(r'"required_num_qubits"="(\d+)"', c_str)
        if m_q:
            self.num_qubits = int(m_q.group(1))

        m_r = re.search(r'"required_num_results"="(\d+)"', c_str)
        if m_r:
            self.num_results = int(m_r.group(1))

        m_fn = re.search(r'define\s+void\s+@([^\s(]+)\s*\([^)]*\)\s*#\d+', c_str)
        if m_fn:
            self.entry_point = m_fn.group(1)

        addr: int = 0
        call_pattern_qis = re.compile(r'call\s+void\s+@__quantum__qis__([a-zA-Z0-9_]+)__body\s*\((.*)\)')
        call_pattern_rt = re.compile(r'call\s+(?:void|ptr|i1)\s+@__quantum__rt__([a-zA-Z0-9_]+)\s*\((.*)\)')
        label_pattern = re.compile(r'^([a-zA-Z0-9_.]+):')
        branch_pattern = re.compile(r'br\s+i1\s+([^,]+),\s+label\s+([%a-zA-Z0-9_.]+),\s+label\s+([%a-zA-Z0-9_.]+)')

        for line in c_str.splitlines():
            s_line: str = line.strip()
            if not s_line or s_line.startswith(';'):
                continue

            # 1. Check for Basic Block Label
            m_lbl = label_pattern.match(s_line)
            if m_lbl:
                lbl_name = m_lbl.group(1)
                inst = QuantumInstruction("label", is_control_flow=True, label=lbl_name, raw_call=s_line)
                inst.address = addr
                inst.synthetic_bytes = b"\x90\x00\x00\x00"
                self.instructions.append(inst)
                continue

            # 2. Check for Branch instruction
            m_br = branch_pattern.match(s_line)
            if m_br:
                cond, l_true, l_false = m_br.groups()
                inst = QuantumInstruction("br", is_control_flow=True,
                                          raw_call=f"br if({cond}) -> {l_true} else {l_false}")
                inst.address = addr
                inst.synthetic_bytes = b"\xeb\x04\x00\x00"
                addr += 4
                self.instructions.append(inst)
                continue

            # 3. Check for Quantum Gate Calls: @__quantum__qis__...
            m_qis = call_pattern_qis.search(s_line)
            if m_qis:
                gate_raw = m_qis.group(1).lower()
                args_raw = m_qis.group(2).strip()
                args = [a.strip() for a in re.split(r',\s*(?![^()]*\))', args_raw) if a.strip()]

                qubits: List[int] = []
                results: List[int] = []
                params: List[float] = []

                if gate_raw in ('h', 'x', 'y', 'z', 's', 't', 's_adj', 't_adj'):
                    if args:
                        qubits.append(self._parse_qubit_id(args[0]))
                    q_inst = QuantumInstruction(gate_raw, qubits=qubits, raw_call=s_line)

                elif gate_raw in ('cnot', 'cx', 'cz', 'swap'):
                    if len(args) >= 2:
                        qubits.append(self._parse_qubit_id(args[0]))
                        qubits.append(self._parse_qubit_id(args[1]))
                    q_inst = QuantumInstruction(gate_raw, qubits=qubits, raw_call=s_line)

                elif gate_raw in ('ccx', 'toffoli'):
                    if len(args) >= 3:
                        qubits.append(self._parse_qubit_id(args[0]))
                        qubits.append(self._parse_qubit_id(args[1]))
                        qubits.append(self._parse_qubit_id(args[2]))
                    q_inst = QuantumInstruction('ccx', qubits=qubits, raw_call=s_line)

                elif gate_raw in ('rx', 'ry', 'rz'):
                    if len(args) >= 2:
                        params.append(self._parse_float_param(args[0]))
                        qubits.append(self._parse_qubit_id(args[1]))
                    q_inst = QuantumInstruction(gate_raw, qubits=qubits, params=params, raw_call=s_line)

                elif gate_raw == 'mz':
                    if len(args) >= 2:
                        qubits.append(self._parse_qubit_id(args[0]))
                        results.append(self._parse_qubit_id(args[1]))
                    q_inst = QuantumInstruction('mz', qubits=qubits, results=results, raw_call=s_line)
                else:
                    q_inst = QuantumInstruction(gate_raw, raw_call=s_line)

                q_inst.address = addr
                q_inst.synthetic_bytes = self._generate_synthetic_bytes(q_inst)
                addr += len(q_inst.synthetic_bytes)
                self.instructions.append(q_inst)
                continue

            # 4. Check for Runtime Lifecycle Calls: @__quantum__rt__...
            m_rt = call_pattern_rt.search(s_line)
            if m_rt:
                rt_func = m_rt.group(1).lower()
                args_raw = m_rt.group(2).strip()
                args = [a.strip() for a in re.split(r',\s*(?![^()]*\))', args_raw) if a.strip()]

                if 'qubit_allocate' in rt_func:
                    q_inst = QuantumInstruction('qalloc', qubits=[0], raw_call=s_line)
                elif 'qubit_release' in rt_func:
                    q_inst = QuantumInstruction('qfree', qubits=[self._parse_qubit_id(args[0]) if args else 0], raw_call=s_line)
                elif 'result_record' in rt_func:
                    q_inst = QuantumInstruction('record_output', results=[self._parse_qubit_id(args[0]) if args else 0], raw_call=s_line)
                else:
                    q_inst = QuantumInstruction(rt_func, raw_call=s_line)

                q_inst.address = addr
                q_inst.synthetic_bytes = b"\x0f\x01\x00\x00"
                addr += len(q_inst.synthetic_bytes)
                self.instructions.append(q_inst)
                continue

        # Auto-compute counts if missing
        if not self.num_qubits and self.instructions:
            all_q = [max(inst.qubits) for inst in self.instructions if inst.qubits]
            self.num_qubits = (max(all_q) + 1) if all_q else 1

        if not self.num_results and self.instructions:
            all_r = [max(inst.results) for inst in self.instructions if inst.results]
            self.num_results = (max(all_r) + 1) if all_r else 1

        return self.instructions

    def _generate_synthetic_bytes(self, inst: QuantumInstruction) -> bytes:
        """Generates standard synthetic opcodes for disassembler representation."""
        OP_MAP: Dict[str, int] = {
            'h': 0x01, 'x': 0x02, 'y': 0x03, 'z': 0x04,
            's': 0x05, 's_adj': 0x06, 't': 0x07, 't_adj': 0x08,
            'cx': 0x10, 'cnot': 0x10, 'cz': 0x11, 'swap': 0x12,
            'ccx': 0x13, 'toffoli': 0x13,
            'rx': 0x20, 'ry': 0x21, 'rz': 0x22, 'mz': 0x30,
            'qalloc': 0x40, 'qfree': 0x41, 'record_output': 0x42
        }
        opcode = OP_MAP.get(inst.op, 0x00)
        b = bytearray([opcode])
        if inst.qubits:
            for q in inst.qubits[:2]:
                b.append(q & 0xFF)
        if inst.results:
            for r in inst.results[:2]:
                b.append(r & 0xFF)
        if inst.params:
            angle_int = int((inst.params[0] % 6.283185) * 1000) & 0xFFFF
            b.extend(struct.pack("<H", angle_int))
        while len(b) < 4:
            b.append(0x00)
        return bytes(b[:4])


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        parser = QIRParser()
        insts = parser.parse_file(sys.argv[1])
        print(f"Parsed {len(insts)} instructions from {sys.argv[1]}:")
        for i in insts:
            print(f"  0x{i.address:04x}: {i.mnemonic()}")
    else:
        print("Usage: python3 qir_parser.py <path_to_bc_or_ll>")


def create_cutter_plugin():
    return None
