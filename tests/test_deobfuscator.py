#!/usr/bin/env python3
"""
Unit Tests for QIR Deobfuscator
Author: Matvei Igumnov <matvei@iqumnov.com>
"""

import unittest
import os
import sys

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
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


if __name__ == "__main__":
    unittest.main()
