#!/usr/bin/env python3
"""
Unit Tests for QIR Parser
Author: Matvei Igumnov <matvei@iqumnov.com>
"""

import unittest
import os
import sys

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(BASE_DIR, "qir_plugin"))

from qir_parser import QIRParser

class TestQIRParser(unittest.TestCase):
    def setUp(self):
        self.parser = QIRParser()
        self.clean_bc = os.path.join(BASE_DIR, "demo_programs", "lr2_quantum_demo.bc")
        self.obf_bc = os.path.join(BASE_DIR, "demo_programs", "lr2_obfuscated.bc")

    def test_clean_demo_parsing(self):
        self.assertTrue(os.path.exists(self.clean_bc), "Clean demo bc must exist")
        insts = self.parser.parse_file(self.clean_bc)
        gates = [i for i in insts if not i.is_control_flow]
        self.assertEqual(len(gates), 5, "Clean demo must contain exactly 5 quantum operations")

        # Verify quantum constructs:
        # Construct 1: Hadamard on q0
        self.assertEqual(gates[0].op, "h")
        self.assertEqual(gates[0].qubits, [0])

        # Construct 2: CNOT on (q0, q1)
        self.assertEqual(gates[1].op, "cnot")
        self.assertEqual(gates[1].qubits, [0, 1])

        # Construct 3: Rz on q1 with pi/4
        self.assertEqual(gates[2].op, "rz")
        self.assertEqual(gates[2].qubits, [1])
        self.assertAlmostEqual(gates[2].params[0], 0.785398, places=4)

        # Construct 4: Measurement in Z
        self.assertEqual(gates[3].op, "mz")
        self.assertEqual(gates[3].qubits, [0])
        self.assertEqual(gates[3].results, [0])

        self.assertEqual(gates[4].op, "mz")
        self.assertEqual(gates[4].qubits, [1])
        self.assertEqual(gates[4].results, [1])

    def test_obfuscated_demo_parsing(self):
        self.assertTrue(os.path.exists(self.obf_bc), "Obfuscated demo bc must exist")
        insts = self.parser.parse_file(self.obf_bc)
        gates = [i for i in insts if not i.is_control_flow]
        self.assertEqual(len(gates), 18, "Obfuscated demo must contain 18 raw quantum gates")

        # Dead qubit operations on q2 detected
        q2_ops = [i for i in gates if 2 in i.qubits]
        self.assertEqual(len(q2_ops), 5, "5 dummy operations on dead qubit q2")


if __name__ == "__main__":
    unittest.main()
