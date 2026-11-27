#!/usr/bin/env python3
"""
Unit Tests for QIR Deobfuscator
Author: Matvei Igumnov <matvei@iqumnov.com>
"""

import unittest
import os
import sys

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if BASE_DIR not in sys.path:
    sys.path.insert(0, BASE_DIR)
sys.path.insert(0, os.path.join(BASE_DIR, "qir_plugin"))

from qir_parser import QIRParser
from qir_deobfuscator import QIRDeobfuscator

class TestQIRDeobfuscator(unittest.TestCase):
    def setUp(self):
        self.parser = QIRParser()
        self.deobf = QIRDeobfuscator()
        self.obf_bc = os.path.join(BASE_DIR, "demo_programs", "lr2_obfuscated.bc")
        self.clean_bc = os.path.join(BASE_DIR, "demo_programs", "lr2_quantum_demo.bc")

    def test_deobfuscation_correctness(self):
        # 1. Parse obfuscated program
        raw_insts = self.parser.parse_file(self.obf_bc)
        raw_gates = [i for i in raw_insts if not i.is_control_flow]
        self.assertEqual(len(raw_gates), 18)

        # 2. Analyze
        findings = self.deobf.analyze(raw_gates)
        self.assertGreaterEqual(len(findings), 4, "Must detect at least 4 obfuscation patterns")

        # 3. Deobfuscate
        cleaned = self.deobf.deobfuscate(raw_gates)
        self.assertEqual(len(cleaned), 5, "Deobfuscated circuit must reduce to exactly 5 canonical gates")

        # 4. Verify gate-by-gate equivalence with clean reference
        clean_insts = self.parser.parse_file(self.clean_bc)
        clean_ref = [i for i in clean_insts if not i.is_control_flow]
        self.assertEqual(len(cleaned), len(clean_ref))

        for c, ref in zip(cleaned, clean_ref):
            self.assertEqual(c.op, ref.op, f"Op mismatch: {c.op} vs {ref.op}")
            self.assertEqual(c.qubits, ref.qubits, f"Qubits mismatch: {c.qubits} vs {ref.qubits}")
            self.assertEqual(c.results, ref.results, f"Results mismatch: {c.results} vs {ref.results}")

        print("\n[OK] Deobfuscation verified: 100% equivalence to reference clean circuit achieved.")

    def test_rotation_modulo_cancellation(self):
        """Verify mathematical precision of rotation gate cancellation mod 2π."""
        import math
        from qir_parser import QuantumInstruction

        # Case 1: Exact positive opposite angles (+θ and -θ)
        g1 = QuantumInstruction(op="rz", qubits=[0], params=[1.2345])
        g2 = QuantumInstruction(op="rz", qubits=[0], params=[-1.2345])
        self.assertTrue(self.deobf._can_cancel_rotations(g1, g2))

        # Case 2: Sum equals 2π with floating-point variance
        g3 = QuantumInstruction(op="rx", qubits=[1], params=[1.5])
        g4 = QuantumInstruction(op="rx", qubits=[1], params=[2.0 * math.pi - 1.5 + 1e-7])
        self.assertTrue(self.deobf._can_cancel_rotations(g3, g4))

        # Case 3: Negative 2π sum (-2π)
        g5 = QuantumInstruction(op="ry", qubits=[2], params=[-1.0])
        g6 = QuantumInstruction(op="ry", qubits=[2], params=[-(2.0 * math.pi - 1.0)])
        self.assertTrue(self.deobf._can_cancel_rotations(g5, g6))

        # Case 4: Non-canceling angles
        g7 = QuantumInstruction(op="rz", qubits=[0], params=[0.5])
        g8 = QuantumInstruction(op="rz", qubits=[0], params=[0.5])
        self.assertFalse(self.deobf._can_cancel_rotations(g7, g8))

    def test_unitary_circuit_without_mz_and_symmetric_multi_qubit_cancellations(self):
        """Unitary subcircuits without mz must not drop qubits, and symmetric CZ/SWAP/CCX pairs cancel."""
        from qir_parser import QuantumInstruction

        unitary = [
            QuantumInstruction(op="h", qubits=[0]),
            QuantumInstruction(op="cz", qubits=[0, 1]),
            QuantumInstruction(op="cz", qubits=[1, 0]),
            QuantumInstruction(op="swap", qubits=[1, 2]),
            QuantumInstruction(op="swap", qubits=[2, 1]),
            QuantumInstruction(op="ccx", qubits=[0, 1, 2]),
            QuantumInstruction(op="ccx", qubits=[1, 0, 2]),
        ]
        cleaned = self.deobf.deobfuscate(unitary)
        self.assertEqual(len(cleaned), 1)
        self.assertEqual(cleaned[0].op, "h")
        self.assertEqual(cleaned[0].qubits, [0])

        # Non-commuting single-qubit barrier must block false CNOT cancellation
        blocked = [
            QuantumInstruction(op="cnot", qubits=[0, 1]),
            QuantumInstruction(op="h", qubits=[0]),
            QuantumInstruction(op="cnot", qubits=[0, 1]),
            QuantumInstruction(op="mz", qubits=[0], results=[0]),
            QuantumInstruction(op="mz", qubits=[1], results=[1]),
        ]
        findings = self.deobf.analyze(blocked)
        self.assertFalse(any(f.pattern_type == "IDENTITY_INVOLUTION_PAIR" for f in findings))
        self.assertEqual(len(self.deobf.deobfuscate(blocked)), 5)

    def test_parser_signed_hex_aliases_and_reuse_isolation(self):
        """Verify signed negative hex floats, sdg/tdg/reset/tail call parsing, and parser reuse isolation."""
        ir_a = r"""
define void @prog_a() #0 {
entry:
  %5 = inttoptr i64 2 to %Qubit*
  tail call void @__quantum__qis__rz__body(double -0x3FE921FB54442D18, %Qubit* %5)
  call void @__quantum__qis__sdg__body(%Qubit* %5)
  call void @__quantum__qis__tdg__body(%Qubit* %5)
  call void @__quantum__qis__reset__body(%Qubit* %5)
  ret void
}
attributes #0 = { "required_num_qubits"="4" "required_num_results"="2" }
"""
        ir_b = r"""
define void @prog_b() #0 {
entry:
  call void @__quantum__qis__h__body(%Qubit* null)
  ret void
}
"""
        insts_a = self.parser.parse_text_string(ir_a)
        gates_a = [i for i in insts_a if not i.is_control_flow]
        self.assertEqual(len(gates_a), 4)
        self.assertLess(gates_a[0].params[0], 0.0)
        self.assertEqual(gates_a[0].qubits, [2])
        self.assertEqual(gates_a[1].mnemonic(), "sdg q[2]")
        self.assertEqual(gates_a[2].mnemonic(), "tdg q[2]")
        self.assertEqual(gates_a[3].mnemonic(), "reset q[2]")

        insts_b = self.parser.parse_text_string(ir_b)
        gates_b = [i for i in insts_b if not i.is_control_flow]
        self.assertEqual(len(gates_b), 1)
        self.assertEqual(self.parser.num_qubits, 1)
        self.assertEqual(self.parser.num_results, 1)
        self.assertEqual(self.parser.entry_point, "prog_b")

    def test_barrier_comments_z8_and_commutation(self):
        """Verify inline comments, opaque ptr, 0x50 barrier opcode, Z_8 consolidation, and commutation."""
        ir = r"""
define void @helper() {
  ret void
}
define void @main() #0 {
entry:
  %q.0 = call ptr @__quantum__rt__qubit_allocate() ; alloc q[0]
  call void @__quantum__qis__t__body(ptr %q.0)
  call void @__quantum__qis__t__body(ptr %q.0)
  call void @__quantum__qis__barrier__body(ptr %q.0) ; sync barrier
  call void @__quantum__qis__s__body(ptr %q.0)
  call void @__quantum__qis__z__body(ptr %q.0)
  call void @__quantum__qis__s__body(ptr %q.0)
  call void @__quantum__qis__mz__body(ptr %q.0, ptr null)
  ret void
}
attributes #0 = { "entry_point" "required_num_qubits"="1" "required_num_results"="1" }
"""
        insts = self.parser.parse_text_string(ir)
        self.assertEqual(self.parser.entry_point, "main")
        barriers = [i for i in insts if i.op == "barrier"]
        self.assertEqual(len(barriers), 1)
        self.assertEqual(barriers[0].synthetic_bytes[0], 0x50)

        cleaned = self.deobf.deobfuscate(insts)
        ops = [i.op for i in cleaned if not i.is_control_flow]
        # Before barrier: t + t -> s. After barrier: s + z + s -> z + s + s = z + z = I (collapsed!)
        self.assertEqual(ops, ["qalloc", "s", "barrier", "mz"])
        self.assertTrue(
            any(f.pattern_type == "DISCRETE_PHASE_CONSOLIDATION" for f in self.deobf.findings),
            "Multi-pass discrete phase consolidation must be recorded in findings",
        )
        self.assertGreater(self.deobf.obfuscation_ratio, 0.0)

    def test_malformed_empty_operands_and_trivial_rotations(self):
        """Empty operand gates never raise IndexError, and trivial 2pi rotations are stripped."""
        import math
        from qir_parser import QuantumInstruction
        from qir_disasm import QIRDisassembler

        malformed = [
            QuantumInstruction(op="cnot", qubits=[]),
            QuantumInstruction(op="ccx", qubits=[0]),
            QuantumInstruction(op="reset", qubits=[]),
            QuantumInstruction(op="rz", qubits=[0], params=[float("nan")]),
        ]
        dis = QIRDisassembler()
        _ = dis.format_disassembly(malformed)
        _ = dis.render_circuit_diagram(malformed)
        _ = dis.export_openqasm(malformed)
        for m in malformed:
            _ = m.mnemonic()
            _ = QIRParser.encode_synthetic_bytes(m)

        # Trivial 2π rotations collapse completely
        triv = [
            QuantumInstruction(op="h", qubits=[0]),
            QuantumInstruction(op="rz", qubits=[0], params=[0.0]),
            QuantumInstruction(op="rx", qubits=[0], params=[math.tau]),
            QuantumInstruction(op="mz", qubits=[0], results=[0]),
        ]
        cleaned = self.deobf.deobfuscate(triv)
        self.assertEqual([i.op for i in cleaned], ["h", "mz"])
        self.assertTrue(any(f.pattern_type == "TRIVIAL_IDENTITY_ROTATION" for f in self.deobf.findings))

    def test_adjoint_rotations_short_hex_and_discrete_phase_classification(self):
        """Adjoint rotations negate theta, short hex 0x0 parses, and S/Sdg classify as DISCRETE_PHASE_CANCELLATION."""
        from qir_parser import QIRParser, QuantumInstruction

        ir = """
define void @main() {
entry:
  call void @__quantum__qis__rx__adj(double 1.250000e+00, %Qubit* null)
  call void @__quantum__qis__ry__adj(double -5.000000e-01, %Qubit* null)
  call void @__quantum__qis__rz__adj(double 0x0, %Qubit* null)
  ret void
}
"""
        insts = QIRParser().parse_text_string(ir)
        gates = [i for i in insts if not i.is_control_flow]
        self.assertEqual([i.op for i in gates], ["rx", "ry", "rz"])
        self.assertAlmostEqual(gates[0].params[0], -1.25, places=6)
        self.assertAlmostEqual(gates[1].params[0], 0.5, places=6)
        self.assertAlmostEqual(gates[2].params[0], 0.0, places=12)

        phase_pairs = [
            QuantumInstruction(op="s", qubits=[0]),
            QuantumInstruction(op="sdg", qubits=[0]),
            QuantumInstruction(op="t", qubits=[0]),
            QuantumInstruction(op="tdg", qubits=[0]),
        ]
        cleaned_pairs = self.deobf.deobfuscate(phase_pairs)
        self.assertEqual(cleaned_pairs, [])
        self.assertEqual(len(self.deobf.findings), 2)
        self.assertTrue(
            all(f.pattern_type == "DISCRETE_PHASE_CANCELLATION" for f in self.deobf.findings),
            f"Unexpected finding types: {[f.pattern_type for f in self.deobf.findings]}",
        )

        phase_seq = [
            QuantumInstruction(op="s", qubits=[0]),
            QuantumInstruction(op="z", qubits=[0]),
            QuantumInstruction(op="t", qubits=[0]),
            QuantumInstruction(op="tdg", qubits=[0]),
            QuantumInstruction(op="sdg", qubits=[0]),
        ]
        cleaned = self.deobf.deobfuscate(phase_seq)
        self.assertEqual([i.op for i in cleaned], ["z"])

    def test_release_parity_barriers_qalloc_array_and_package_identity(self):
        """Verify disjoint targeted barrier commutation, qalloc_array isolation, mresetz, and single module load."""
        import qir_plugin
        import qir_plugin.qir_parser as pkg_parser
        from qir_parser import QuantumInstruction, QIRParser
        from qir_disasm import QIRDisassembler

        # 1. Importing package must resolve cleanly to a single QIRParser class
        self.assertIsInstance(qir_plugin.QIRDisassembler().parser, qir_plugin.QIRParser)
        self.assertIs(pkg_parser.QuantumInstruction, qir_plugin.qir_parser.QuantumInstruction)

        # 2. Targeted barrier q[1] must allow disjoint h q[0] · h q[0] cancellation (matching Rust)
        ir = r"""
define void @main() #0 {
entry:
  %qarr = call ptr @__quantum__rt__qubit_allocate_array(i64 3)
  %q0 = inttoptr i64 0 to ptr
  %q0_alias = bitcast ptr %q0 to ptr
  %q1 = inttoptr i64 1 to ptr
  %q2 = inttoptr i64 2 to ptr
  %r2 = inttoptr i64 2 to ptr
  call void @__quantum__qis__h__body(ptr %q0_alias)
  call void @__quantum__qis__barrier__body(ptr %q1)
  call void @__quantum__qis__h__body(ptr %q0)
  call void @__quantum__qis__s_dag__body(ptr %q0)
  call void @__quantum__qis__t_dag__body(ptr %q0)
  call void @__quantum__qis__x__body(ptr %q2)
  call void @__quantum__qis__mz__body(ptr %q0, ptr %r2)
  call void @__quantum__qis__mz__body(ptr %q1, ptr null)
  ret void
}
"""
        insts = self.parser.parse_text_string(ir)
        cleaned = self.deobf.deobfuscate(insts)
        ops = [i.op for i in cleaned if not i.is_control_flow]
        # Dead q[2] pruned despite qalloc_array([0,1,2]); h q[0] pairs cancel past live barrier q[1]
        self.assertNotIn("x", ops)
        self.assertIn("barrier", ops)
        self.assertIn("sdg", ops)
        self.assertIn("tdg", ops)
        self.assertEqual(ops.count("h"), 0)
        qasm = QIRDisassembler().export_openqasm(cleaned)
        self.assertIn("sdg q[0];", qasm)
        self.assertIn("tdg q[0];", qasm)
        self.assertIn("barrier q[1];", qasm)
        mz_insts = [i for i in cleaned if i.op == "mz"]
        self.assertEqual(mz_insts[0].results, [2])

        # 3. Runtime qalloc/qfree/record_output use canonical 0x40/0x41/0x42 opcodes
        rt_ir = """
define void @main() {
  %q = call %Qubit* @__quantum__rt__qubit_allocate()
  call void @__quantum__rt__qubit_release(%Qubit* %q)
  call void @__quantum__rt__result_record_output(%Result* null)
  ret void
}
"""
        rt_insts = QIRParser().parse_text_string(rt_ir)
        self.assertEqual(rt_insts[0].synthetic_bytes[0], 0x40)
        self.assertEqual(rt_insts[1].synthetic_bytes[0], 0x41)
        self.assertEqual(rt_insts[2].synthetic_bytes[0], 0x42)

        # 4. OOM guard on huge result indices in ASCII renderer
        huge = [QuantumInstruction(op="mz", qubits=[0], results=[999999])]
        self.assertIn("exceeds ASCII diagram rendering limit", QIRDisassembler().render_circuit_diagram(huge))

        # 5. Multi-token define linkage, return-attributed calls, mresetz OpenQASM, and non-label Total Opcodes count
        mr_ir = r"""
define internal void @helper_entry() #0 {
entry:
  %q = call noundef ptr @__quantum__rt__qubit_allocate()
  call void @__quantum__qis__mresetz__body(ptr %q, ptr inttoptr (i64 2 to ptr))
  ret void
}
attributes #0 = { "entry_point" }
"""
        mr_parser = QIRParser()
        mr_insts = mr_parser.parse_text_string(mr_ir)
        self.assertEqual(mr_parser.entry_point, "helper_entry")
        dis = QIRDisassembler(mr_parser)
        dis_txt = dis.format_disassembly(mr_insts)
        self.assertIn("Total Opcodes    : 2", dis_txt)
        mr_qasm = dis.export_openqasm(mr_insts)
        self.assertIn("bit[3] r;", mr_qasm)
        self.assertIn("r[2] = measure q[0];", mr_qasm)
        self.assertIn("reset q[0];", mr_qasm)
        self.assertIn("r[2]:", dis.render_circuit_diagram(mr_insts))

    def test_rz_plugin_import_without_rzpipe_and_wrapper_detection(self):
        """rz_qir_plugin imports offline cleanly even without rzpipe, and 0x0B17C0DE header triggers bitcode path."""
        import importlib
        import rz_qir_plugin
        import tempfile

        importlib.reload(rz_qir_plugin)
        self.assertTrue(hasattr(rz_qir_plugin, "RzQIRPlugin"))
        if rz_qir_plugin.rzpipe is None:
            plugin = rz_qir_plugin.RzQIRPlugin(self.clean_bc)
            with self.assertRaises(RuntimeError):
                plugin.attach_rizin()

        # Verify 0x0B17C0DE LLVM wrapper header routes to parse_bitcode rather than UTF-8 text parsing
        wrapper_hdr = b"\xde\xc0\x17\x0b" + b"\x00" * 16 + b"BC\xc0\xde\x00\x00"
        called = []
        parser = QIRParser()
        orig_parse_bc = parser.parse_bitcode
        parser.parse_bitcode = lambda fp, raw_bytes=None: called.append(len(raw_bytes or b"")) or []
        try:
            with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tf:
                tf.write(wrapper_hdr)
                tmp_path = tf.name
            try:
                parser.parse_file(tmp_path)
            finally:
                os.unlink(tmp_path)
        finally:
            parser.parse_bitcode = orig_parse_bc
        self.assertEqual(called, [len(wrapper_hdr)])

    def test_python_parser_edge_hardening_and_qasm_parity(self):
        """Verify NaN/Inf/SSA float rejection, dotted SSA, nested parens, 4-byte BC magic, and mresetz QASM."""
        from qir_parser import QIRParser
        from qir_disasm import QIRDisassembler

        p = QIRParser()
        with self.assertRaises(ValueError):
            p._parse_float_param("double nan")
        with self.assertRaises(ValueError):
            p._parse_float_param("double inf")
        with self.assertRaises(ValueError):
            p._parse_float_param("double %theta1")
        self.assertAlmostEqual(p._parse_float_param("double - 0x3FE921FB54442D18"), -0.7853981633974483, places=12)

        # Dotted SSA %1.tmp must NOT parse as Qubit(1)
        self.assertEqual(p._parse_qubit_id("%1.tmp"), 0)
        self.assertEqual(p._parse_qubit_id("ptr %2"), 2)

        # Deeply nested parentheses + i8* binding + single-arg mresetz OpenQASM sizing
        complex_ir = r"""
define void @f1() {
  %q = inttoptr i64 1 to i8*
  call void @__quantum__qis__h__body(ptr %q)
  ret void
}
define void @f2() {
  call void @__quantum__qis__h__body(ptr inttoptr (i64 ptrtoint (ptr @g to i64) to ptr))
  call void @__quantum__qis__barrier__body(ptr inttoptr (i64 0 to ptr), ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__mresetz__body(ptr inttoptr (i64 2 to ptr))
  ret void
}
"""
        insts = p.parse_text_string(complex_ir)
        gates = [i for i in insts if not i.is_control_flow]
        self.assertEqual([g.op for g in gates], ["h", "h", "barrier", "mresetz"])
        self.assertEqual(gates[2].mnemonic(), "barrier q[0], q[2]")
        # Fence between @f1 and @f2 prevented H(q0)/H(q1) mixups and isolated basic blocks
        fences = [i for i in insts if i.is_control_flow]
        self.assertTrue(any(f.op == "barrier" for f in fences))
        qasm = QIRDisassembler(p).export_openqasm(gates)
        self.assertIn("bit[3] r;", qasm)
        self.assertIn("r[2] = measure q[2];", qasm)


if __name__ == "__main__":
    unittest.main()
