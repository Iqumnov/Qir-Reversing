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

import math
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
            q0 = f"q[{self.qubits[0]}]" if len(self.qubits) > 0 else "q[?]"
            q1 = f"q[{self.qubits[1]}]" if len(self.qubits) > 1 else "q[?]"
            return f"cx {q0}, {q1}"
        elif self.op == 'cz':
            q0 = f"q[{self.qubits[0]}]" if len(self.qubits) > 0 else "q[?]"
            q1 = f"q[{self.qubits[1]}]" if len(self.qubits) > 1 else "q[?]"
            return f"cz {q0}, {q1}"
        elif self.op == 'swap':
            q0 = f"q[{self.qubits[0]}]" if len(self.qubits) > 0 else "q[?]"
            q1 = f"q[{self.qubits[1]}]" if len(self.qubits) > 1 else "q[?]"
            return f"swap {q0}, {q1}"
        elif self.op in ('ccx', 'toffoli'):
            q0 = f"q[{self.qubits[0]}]" if len(self.qubits) > 0 else "q[?]"
            q1 = f"q[{self.qubits[1]}]" if len(self.qubits) > 1 else "q[?]"
            q2 = f"q[{self.qubits[2]}]" if len(self.qubits) > 2 else "q[?]"
            return f"ccx {q0}, {q1}, {q2}"
        elif self.op in ('rx', 'ry', 'rz'):
            param_str = f"{self.params[0]:.6f}" if self.params else "0.0"
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"{self.op}({param_str}) {q}"
        elif self.op in ('mz', 'mresetz'):
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"{self.op} {q} -> {r}"
        elif self.op == 'qalloc':
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"qalloc -> {q}"
        elif self.op == 'qfree':
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"qfree {q}"
        elif self.op == 'record_output':
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"record_output {r}"
        elif self.op == 'read_result':
            r = f"r[{self.results[0]}]" if self.results else "r[?]"
            return f"read_result {r}"
        elif self.op == 'reset':
            q = f"q[{self.qubits[0]}]" if self.qubits else "q[?]"
            return f"reset {q}"
        elif self.op == 'barrier':
            if not self.qubits:
                return "barrier"
            return "barrier " + ", ".join(f"q[{q}]" for q in self.qubits)
        parts: List[str] = [f"q[{q}]" for q in self.qubits]
        parts.extend(f"r[{r}]" for r in self.results)
        parts.extend(f"{p:.4f}" for p in self.params)
        return f"{self.op} " + ", ".join(parts)


def _split_llvm_args(args_str: str) -> List[str]:
    """Splits comma-separated LLVM call arguments while respecting nested parentheses and quotes."""
    args: List[str] = []
    depth = 0
    in_quotes = False
    escaped = False
    start = 0
    for idx, ch in enumerate(args_str):
        if in_quotes:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_quotes = False
            continue
        if ch == '"':
            in_quotes = True
        elif ch == '(':
            depth += 1
        elif ch == ')':
            if depth > 0:
                depth -= 1
        elif ch == ',' and depth == 0:
            part = args_str[start:idx].strip()
            if part:
                args.append(part)
            start = idx + 1
    tail = args_str[start:].strip()
    if tail:
        args.append(tail)
    return args


def _extract_balanced_call_args(line: str, marker: str) -> Optional[Tuple[str, str]]:
    """Extracts (function_suffix, inside_parens_args) for a call/invoke line with arbitrary nesting."""
    m = re.match(
        r'^(?:%[a-zA-Z0-9_.-]+\s*=\s*)?(?:(?:tail|musttail|notail)\s+)?(?:call|invoke)\s+[^@\n]+?'
        + re.escape(marker)
        + r'([a-zA-Z0-9_]+)\s*\(',
        line,
    )
    if not m:
        return None
    fn_name = m.group(1)
    open_idx = m.end() - 1
    depth = 0
    in_quotes = False
    escaped = False
    for idx in range(open_idx, len(line)):
        ch = line[idx]
        if in_quotes:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_quotes = False
            continue
        if ch == '"':
            in_quotes = True
        elif ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
            if depth == 0:
                return (fn_name, line[open_idx + 1 : idx])
    return None


class QIRParser:
    """Full parser for LLVM QIR Bytecode and Textual IR."""
    def __init__(self) -> None:
        self.module_name: str = ""
        self.num_qubits: int = 0
        self.num_results: int = 0
        self.entry_point: str = ""
        self.last_decoded_ir: Optional[str] = None
        self.instructions: List[QuantumInstruction] = []
        self.var_qubits: Dict[str, int] = {}
        self.var_results: Dict[str, int] = {}
        self.var_opaque_ptrs: Dict[str, int] = {}

    @property
    def quantum_gates(self) -> List[QuantumInstruction]:
        """Returns only quantum operations, excluding classical control flow labels."""
        return [i for i in self.instructions if not i.is_control_flow]

    def parse_file(self, filepath: str) -> List[QuantumInstruction]:
        """Dispatches to bitcode or text parser based on a single file read (FIFO-safe)."""
        with open(filepath, "rb") as f:
            raw_bytes = f.read()

        if (
            raw_bytes[:4] == b"\x42\x43\xc0\xde"
            or (len(raw_bytes) >= 20 and raw_bytes[:4] == b"\xde\xc0\x17\x0b")
        ):
            return self.parse_bitcode(filepath, raw_bytes=raw_bytes)
        return self.parse_text_string(raw_bytes.decode("utf-8", errors="replace"))

    def parse_bitcode(self, filepath: str, raw_bytes: Optional[bytes] = None) -> List[QuantumInstruction]:
        """Parses binary LLVM bitcode (.bc) via PyQIR, qir-rev, llvm-dis, or companion .ll."""
        if raw_bytes is None:
            with open(filepath, "rb") as f:
                raw_bytes = f.read()

        try:
            if pyqir is not None:
                mod = pyqir.Module.from_bitcode(pyqir.Context(), raw_bytes)
                return self._parse_pyqir_module(mod)
        except Exception:
            pass

        # Fallback 1: Pure-Rust qir-rev CLI (--emit-ir) if built locally or installed
        try:
            import shutil
            import subprocess
            import tempfile
            repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            cli_name = "qir-rev.exe" if os.name == "nt" else "qir-rev"
            cli_candidates = [
                os.path.join(repo_root, "target", "release", cli_name),
                os.path.expanduser(f"~/.local/bin/{cli_name}"),
                os.path.expanduser(f"~/.cargo/bin/{cli_name}"),
                os.path.expanduser(f"~/.qir-rev/bin/{cli_name}"),
                shutil.which("qir-rev"),
                shutil.which("qrev"),
            ]
            for cli_bin in cli_candidates:
                if cli_bin and os.path.isfile(cli_bin):
                    tmp_path = None
                    try:
                        target_arg = filepath
                        if not os.path.isfile(filepath):
                            with tempfile.NamedTemporaryFile(suffix=".bc", delete=False) as tf:
                                tf.write(raw_bytes)
                                tmp_path = tf.name
                            target_arg = tmp_path
                        res = subprocess.run(
                            [cli_bin, "disasm", "--emit-ir", target_arg],
                            stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL,
                            check=False,
                        )
                        if res.returncode == 0 and res.stdout:
                            decoded_ir = res.stdout.decode("utf-8", errors="replace")
                            self.last_decoded_ir = decoded_ir
                            return self.parse_text_string(decoded_ir)
                    finally:
                        if tmp_path and os.path.exists(tmp_path):
                            try:
                                os.unlink(tmp_path)
                            except OSError:
                                pass
        except Exception:
            pass

        # Fallback 2: llvm-dis CLI via stdin bytes
        try:
            import subprocess
            res = subprocess.run(
                ["llvm-dis", "-o", "-", "-"],
                input=raw_bytes,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            if res.returncode == 0 and res.stdout:
                decoded_ir = res.stdout.decode("utf-8", errors="replace")
                self.last_decoded_ir = decoded_ir
                return self.parse_text_string(decoded_ir)
        except Exception:
            pass

        # Fallback 3: companion textual .ll if present
        ll_cand = os.path.splitext(filepath)[0] + ".ll"
        if os.path.exists(ll_cand) and os.path.abspath(ll_cand) != os.path.abspath(filepath):
            return self.parse_text_ir(ll_cand)

        # Fallback 4: readable QIR ASCII call extraction only if it yields actual operations
        try:
            extracted = "".join(chr(b) if 32 <= b < 127 or b in (10, 13, 9) else " " for b in raw_bytes)
            if "@__quantum__qis__" in extracted:
                parsed = self.parse_text_string(extracted)
                if parsed:
                    return parsed
        except Exception:
            pass

        raise RuntimeError(
            f"Failed to parse LLVM bitcode '{filepath}': PyQIR / qir-rev / llvm-dis / companion .ll unavailable."
        )

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
        ir_str = str(mod)
        self.last_decoded_ir = ir_str
        return self.parse_text_string(ir_str)

    def _strip_ir_comment(self, line: str) -> str:
        """Strip trailing LLVM IR comments outside string literals."""
        in_quotes = False
        escape = False
        for idx, ch in enumerate(line):
            if escape:
                escape = False
                continue
            if ch == '\\' and in_quotes:
                escape = True
                continue
            if ch == '"':
                in_quotes = not in_quotes
            elif ch == ';' and not in_quotes:
                return line[:idx]
        return line

    def _parse_qubit_id(self, arg_str: str) -> int:
        """Extracts integer qubit index from LLVM pointer or SSA arg."""
        for tok in re.findall(r'(%[a-zA-Z0-9_.-]+)', arg_str):
            if tok in self.var_qubits:
                return self.var_qubits[tok]
            if tok in self.var_opaque_ptrs:
                return self.var_opaque_ptrs[tok]
        if re.search(r'(?:^|\s)null(?=\s|$)', arg_str):
            return 0
        m = re.search(r'inttoptr\s*\(\s*i(?:64|32|16|8)\s+(\d+)\s+to\s+(?:ptr|%[A-Za-z0-9_.-]+\*)\s*\)', arg_str)
        if m:
            return int(m.group(1))
        # Match pure numeric SSA token (%2, avoiding %1.tmp or %2_foo)
        m = re.search(r'%(\d+)(?=[\s,)]|$)', arg_str)
        if m:
            return int(m.group(1))
        # Match explicit typed integer index (avoiding i64/Qubit64 width/type digits)
        m = re.search(r'\bi(?:64|32|16|8)\s+(\d+)(?=[\s,)]|$)', arg_str)
        if m:
            return int(m.group(1))
        return 0

    def _resolve_result_id(self, arg_str: str, fallback: Optional[int] = None) -> int:
        """Extracts integer Result index from LLVM pointer or SSA arg, scanning all '%' tokens."""
        for tok in re.findall(r'(%[a-zA-Z0-9_.-]+)', arg_str):
            if tok in self.var_results:
                return self.var_results[tok]
            if tok in self.var_opaque_ptrs:
                return self.var_opaque_ptrs[tok]
        if fallback is not None and not arg_str.strip():
            return fallback
        return self._parse_qubit_id(arg_str)

    def _parse_float_param(self, arg_str: str) -> float:
        """Parses double parameter, including signed hex float literals (e.g. -0x3FE921FB54442D18)."""
        s: str = arg_str.strip()
        m_hex = re.search(r'(?:(-\s*)|(?<![a-zA-Z0-9_.-]))0x([0-9a-fA-F]{1,16})(?=[\s,)]|$)', s)
        if m_hex:
            val_int = int(m_hex.group(2).zfill(16), 16)
            val_bytes = struct.pack(">Q", val_int)
            val = float(struct.unpack(">d", val_bytes)[0])
            if not math.isfinite(val):
                raise ValueError(f"Non-finite hex float literal in '{arg_str}'")
            if m_hex.group(1) and val > 0.0:
                val = -val
            return val
        if '0x' in s:
            raise ValueError(f"Malformed hex float literal in '{arg_str}'")
        cleaned = re.sub(r'^[\s,]*(?:double|float|half|bfloat)\b\s*', '', s)
        if re.search(r'(?i)(?<![a-zA-Z0-9_.-])(?:[+-]?inf(?:inity)?|s?nan)(?![a-zA-Z0-9_.-])', cleaned):
            raise ValueError(f"Non-finite float literal in '{arg_str}'")
        if cleaned.startswith('%'):
            raise ValueError(f"Unresolved SSA float operand '{arg_str}'")
        m_dec = re.match(r'^([-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)(?=[\s,)]|$)', cleaned)
        if m_dec:
            val = float(m_dec.group(1))
            if not math.isfinite(val):
                raise ValueError(f"Non-finite float literal in '{arg_str}'")
            return val
        raise ValueError(f"Invalid float literal '{arg_str}'")

    def parse_text_string(self, content: str) -> List[QuantumInstruction]:
        """Regex and AST parser for LLVM QIR text lines."""
        self.num_qubits = 0
        self.num_results = 0
        self.entry_point = ""
        self.last_decoded_ir = content
        self.instructions = []
        self.var_qubits = {}
        self.var_results = {}
        self.var_opaque_ptrs = {}
        next_alloc_q = 0
        next_alloc_r = 0
        c_str: str = str(content)

        attr_groups: Dict[str, str] = {}
        for gid, body in re.findall(r'attributes\s+#(\d+)\s*=\s*\{([^}]*)\}', c_str):
            attr_groups[gid] = body

        entry_attr_ids = {gid for gid, body in attr_groups.items() if '"entry_point"' in body or '"EntryPoint"' in body}
        defines = re.findall(
            r'define\s+[^@\n{]+\s+@(?:"([^"]+)"|([^\s(]+))\s*\([^\n{]*\)(?:\s+[^#\n{]*#(\d+))?',
            c_str,
        )
        chosen_attr_id: Optional[str] = None
        for quoted_name, raw_name, attr_id in defines:
            fn_name = quoted_name or raw_name
            if attr_id and attr_id in entry_attr_ids:
                self.entry_point = fn_name
                chosen_attr_id = attr_id
                break
        if not self.entry_point and defines:
            quoted_name, raw_name, attr_id = defines[0]
            self.entry_point = quoted_name or raw_name
            chosen_attr_id = attr_id or None

        attr_scope = attr_groups.get(chosen_attr_id, c_str) if chosen_attr_id else c_str
        m_q = re.search(r'"required_num_qubits"="(\d+)"', attr_scope) or re.search(r'"required_num_qubits"="(\d+)"', c_str)
        if m_q:
            self.num_qubits = int(m_q.group(1))

        m_r = re.search(r'"required_num_results"="(\d+)"', attr_scope) or re.search(r'"required_num_results"="(\d+)"', c_str)
        if m_r:
            self.num_results = int(m_r.group(1))

        addr: int = 0
        has_emitted_in_fn = False
        label_pattern = re.compile(r'^([a-zA-Z0-9_.-]+):')
        branch_pattern = re.compile(r'br\s+i1\s+([^,]+),\s+label\s+([%a-zA-Z0-9_.-]+),\s+label\s+([%a-zA-Z0-9_.-]+)')

        for line in c_str.splitlines():
            s_line: str = self._strip_ir_comment(line).strip()
            if not s_line:
                continue

            if s_line.startswith("define "):
                if has_emitted_in_fn:
                    fence = QuantumInstruction("barrier", is_control_flow=True, raw_call=s_line)
                    fence.address = addr
                    fence.synthetic_bytes = b"\x90\x00\x00\x00"
                    self.instructions.append(fence)
                    has_emitted_in_fn = False
                self.var_qubits.clear()
                self.var_results.clear()
                self.var_opaque_ptrs.clear()
                next_alloc_q = 0
                next_alloc_r = 0
                continue

            # Track standalone SSA pointer bindings (%1 = inttoptr i64 5 to ptr / %Qubit* / i8*)
            is_call_line = bool(re.match(r'^(?:%[a-zA-Z0-9_.-]+\s*=\s*)?(?:(?:tail|musttail|notail)\s+)?(?:call|invoke)\s+', s_line))
            m_ptr_bind = (
                None
                if is_call_line
                else re.match(r'(%[a-zA-Z0-9_.-]+)\s*=\s*inttoptr\s*\(?\s*i(?:64|32|16|8)\s+(\d+)\s+to\s+(%[A-Za-z0-9_.-]+\*|ptr|i8\*)', s_line)
            )
            if m_ptr_bind:
                var_name, idx_str, target_ty = m_ptr_bind.groups()
                idx_val = int(idx_str)
                if 'Result' in target_ty:
                    self.var_results[var_name] = idx_val
                elif 'Qubit' in target_ty:
                    self.var_qubits[var_name] = idx_val
                else:
                    self.var_opaque_ptrs[var_name] = idx_val
                continue

            m_alias = re.match(r'(%[a-zA-Z0-9_.-]+)\s*=\s*(?:bitcast|addrspacecast|getelementptr)\b(.*)$', s_line)
            if m_alias:
                dst_var, expr_rest = m_alias.groups()
                for tok in re.findall(r'(%[a-zA-Z0-9_.-]+)', expr_rest):
                    if tok in self.var_qubits:
                        self.var_qubits[dst_var] = self.var_qubits[tok]
                        break
                    if tok in self.var_results:
                        self.var_results[dst_var] = self.var_results[tok]
                        break
                    if tok in self.var_opaque_ptrs:
                        self.var_opaque_ptrs[dst_var] = self.var_opaque_ptrs[tok]
                        break
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

            # 2. Check for Branch / Switch instruction
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

            if re.match(r'(?:br\s+label|switch\s+)', s_line):
                inst = QuantumInstruction("br", is_control_flow=True, raw_call=s_line)
                inst.address = addr
                inst.synthetic_bytes = b"\xeb\x04\x00\x00"
                addr += 4
                self.instructions.append(inst)
                continue

            # 3. Check for Quantum Gate Calls: @__quantum__qis__...
            qis_call = _extract_balanced_call_args(s_line, "@__quantum__qis__") if is_call_line else None
            if qis_call:
                gate_raw, args_raw = qis_call
                gate_raw = gate_raw.lower()
                if gate_raw.endswith('__body'):
                    gate_raw = gate_raw[:-6]
                if gate_raw in ('s__adj', 's_adj', 's_dag', 'sdg'):
                    gate_raw = 'sdg'
                elif gate_raw in ('t__adj', 't_adj', 't_dag', 'tdg'):
                    gate_raw = 'tdg'
                elif gate_raw in ('sdg__adj', 'sdg_adj'):
                    gate_raw = 's'
                elif gate_raw in ('tdg__adj', 'tdg_adj'):
                    gate_raw = 't'
                elif gate_raw in ('h__adj', 'h_adj', 'x__adj', 'x_adj', 'y__adj', 'y_adj', 'z__adj', 'z_adj',
                                  'cnot__adj', 'cnot_adj', 'cx__adj', 'cx_adj', 'cz__adj', 'cz_adj',
                                  'swap__adj', 'swap_adj', 'ccx__adj', 'ccx_adj', 'toffoli__adj', 'toffoli_adj'):
                    gate_raw = gate_raw.replace('__adj', '').replace('_adj', '')
                elif gate_raw == 'm':
                    gate_raw = 'mz'
                args = _split_llvm_args(args_raw)

                qubits: List[int] = []
                results: List[int] = []
                params: List[float] = []

                if gate_raw in ('h', 'x', 'y', 'z', 's', 't', 'sdg', 'tdg', 'reset'):
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

                elif gate_raw in ('rx', 'ry', 'rz', 'rx__adj', 'ry__adj', 'rz__adj', 'rx_adj', 'ry_adj', 'rz_adj'):
                    is_adj = gate_raw.endswith('__adj') or gate_raw.endswith('_adj')
                    base_op = gate_raw[:2]
                    if len(args) >= 2:
                        theta = self._parse_float_param(args[0])
                        params.append(-theta if is_adj else theta)
                        qubits.append(self._parse_qubit_id(args[1]))
                    q_inst = QuantumInstruction(base_op, qubits=qubits, params=params, raw_call=s_line)

                elif gate_raw in ('mz', 'mresetz'):
                    if len(args) >= 2:
                        q_idx = self._parse_qubit_id(args[0])
                        r_idx = self._resolve_result_id(args[1])
                        qubits.append(q_idx)
                        results.append(r_idx)
                    elif len(args) == 1:
                        q_idx = self._parse_qubit_id(args[0])
                        qubits.append(q_idx)
                        lhs_m = re.match(r'(%[a-zA-Z0-9_.-]+)\s*=', s_line)
                        if lhs_m:
                            self.var_results[lhs_m.group(1)] = q_idx
                        results.append(q_idx)
                    q_inst = QuantumInstruction(gate_raw, qubits=qubits, results=results, raw_call=s_line)
                elif gate_raw == 'read_result':
                    if args:
                        results.append(self._resolve_result_id(args[0]))
                    q_inst = QuantumInstruction('read_result', results=results, raw_call=s_line)
                elif gate_raw == 'barrier':
                    qubits = [self._parse_qubit_id(a) for a in args]
                    q_inst = QuantumInstruction('barrier', qubits=qubits, raw_call=s_line)
                else:
                    for arg in args:
                        if ('double' in arg or 'float' in arg) and 'ptr' not in arg and 'Qubit' not in arg and 'Result' not in arg:
                            params.append(self._parse_float_param(arg))
                        elif 'Result' in arg or any(tok in self.var_results for tok in re.findall(r'(%[a-zA-Z0-9_.-]+)', arg)):
                            results.append(self._resolve_result_id(arg))
                        elif 'Qubit' in arg or 'ptr' in arg or 'null' in arg or 'inttoptr' in arg or any(tok in self.var_qubits for tok in re.findall(r'(%[a-zA-Z0-9_.-]+)', arg)):
                            qubits.append(self._parse_qubit_id(arg))
                    q_inst = QuantumInstruction(gate_raw, qubits=qubits, results=results, params=params, raw_call=s_line)

                q_inst.address = addr
                q_inst.synthetic_bytes = self._generate_synthetic_bytes(q_inst)
                addr += len(q_inst.synthetic_bytes)
                self.instructions.append(q_inst)
                has_emitted_in_fn = True
                continue

            # 4. Check for Runtime Lifecycle Calls: @__quantum__rt__...
            rt_call = _extract_balanced_call_args(s_line, "@__quantum__rt__") if is_call_line else None
            if rt_call:
                rt_func, args_raw = rt_call
                rt_func = rt_func.lower()
                args = _split_llvm_args(args_raw)

                if rt_func == 'qubit_allocate_array':
                    m_cnt = re.search(r'\bi(?:64|32|16|8)\s+(\d+)', args_raw)
                    count = int(m_cnt.group(1)) if m_cnt else 1
                    q_id = next_alloc_q
                    next_alloc_q += count
                    q_inst = QuantumInstruction('qalloc_array', qubits=list(range(q_id, q_id + count)), raw_call=s_line)
                elif 'qubit_allocate' in rt_func:
                    lhs_m = re.match(r'(%[a-zA-Z0-9_.-]+)\s*=', s_line)
                    q_id = next_alloc_q
                    if lhs_m:
                        self.var_qubits[lhs_m.group(1)] = q_id
                    next_alloc_q += 1
                    q_inst = QuantumInstruction('qalloc', qubits=[q_id], raw_call=s_line)
                elif 'qubit_release' in rt_func:
                    q_inst = QuantumInstruction('qfree', qubits=[self._parse_qubit_id(args[0]) if args else 0], raw_call=s_line)
                elif 'result_record' in rt_func:
                    r_arg = args[0] if args else ""
                    r_id = self._resolve_result_id(r_arg, fallback=next_alloc_r)
                    next_alloc_r += 1
                    q_inst = QuantumInstruction('record_output', results=[r_id], raw_call=s_line)
                else:
                    q_inst = QuantumInstruction(rt_func, raw_call=s_line)

                q_inst.address = addr
                q_inst.synthetic_bytes = self._generate_synthetic_bytes(q_inst)
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

    @staticmethod
    def encode_synthetic_bytes(inst: QuantumInstruction) -> bytes:
        """Generates canonical 4-byte synthetic opcodes aligned with Rust qir-core & Rizin C-ABI."""
        OP_MAP: Dict[str, int] = {
            'h': 0x01, 'x': 0x02, 'y': 0x03, 'z': 0x04,
            's': 0x05, 'sdg': 0x06, 's_adj': 0x06, 's_dag': 0x06,
            't': 0x07, 'tdg': 0x08, 't_adj': 0x08, 't_dag': 0x08,
            'cx': 0x10, 'cnot': 0x10, 'cz': 0x11, 'swap': 0x12,
            'ccx': 0x13, 'toffoli': 0x13,
            'rx': 0x20, 'ry': 0x21, 'rz': 0x22,
            'mz': 0x30, 'mresetz': 0x30, 'reset': 0x3f,
            'qalloc': 0x40, 'qalloc_array': 0x40, 'qfree': 0x41, 'record_output': 0x42,
            'barrier': 0x50,
        }
        opcode = OP_MAP.get(inst.op, 0xFF)
        b = bytearray([opcode])
        if inst.params and inst.qubits:
            b.append(inst.qubits[0] & 0xFF)
            theta = inst.params[0]
            if math.isfinite(theta):
                deg = int(round(((theta % math.tau) * 180.0 / math.pi))) % 360
            else:
                deg = 0
            b.extend(struct.pack("<H", deg))
        else:
            if inst.qubits:
                for q in inst.qubits[:3]:
                    b.append(q & 0xFF)
            if inst.results:
                for r in inst.results[:2]:
                    b.append(r & 0xFF)
        while len(b) < 4:
            b.append(0x00)
        return bytes(b[:4])

    def _generate_synthetic_bytes(self, inst: QuantumInstruction) -> bytes:
        return self.encode_synthetic_bytes(inst)


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
