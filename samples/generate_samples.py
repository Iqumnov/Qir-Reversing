#!/usr/bin/env python3
r"""
Quantum Intermediate Representation (QIR) Sample Generator
Generates canonical Bell State and Obfuscated Quantum Test Circuits.
"""

import os
import math
import pyqir

OUT_DIR = os.path.dirname(os.path.abspath(__file__))

def generate_bell_state():
    mod = pyqir.SimpleModule("bell_state", num_qubits=2, num_results=2)
    qis = pyqir.BasicQisBuilder(mod.builder)
    q0 = mod.qubits[0]
    q1 = mod.qubits[1]
    r0 = mod.results[0]
    r1 = mod.results[1]

    # Superposition: H|0>
    qis.h(q0)
    # Entanglement: CNOT(q0, q1)
    qis.cx(q0, q1)
    # Phase Rotation: Rz(pi/4) on q1
    qis.rz(math.pi / 4.0, q1)
    # Measurement: Mz
    qis.mz(q0, r0)
    qis.mz(q1, r1)

    ll_path = os.path.join(OUT_DIR, "bell_state.ll")
    bc_path = os.path.join(OUT_DIR, "bell_state.bc")

    with open(ll_path, "w") as f:
        f.write(mod.ir())
    with open(bc_path, "wb") as f:
        f.write(mod.bitcode())

    print(f"[✓] Generated canonical sample: {ll_path} and {bc_path}")


def generate_obfuscated_sample():
    mod = pyqir.SimpleModule("obfuscated_circuit", num_qubits=3, num_results=2)
    qis = pyqir.BasicQisBuilder(mod.builder)
    q0 = mod.qubits[0]
    q1 = mod.qubits[1]
    q2 = mod.qubits[2]
    r0 = mod.results[0]
    r1 = mod.results[1]

    # Obfuscation 1: Involutory self-inverse pairs (H*H = I)
    qis.h(q0)
    qis.h(q0)
    qis.h(q0)  # Net effect: H(q0)

    # Obfuscation 2: Opposite phase rotation cancellations (Rz(theta)*Rz(-theta) = I)
    qis.rz(0.2 * math.pi, q0)
    qis.rz(-0.2 * math.pi, q0)

    # Obfuscation 3: Redundant Pauli-X pairs (X*X = I)
    qis.x(q1)
    qis.x(q1)

    # Entanglement
    qis.cx(q0, q1)

    # Obfuscation 4: Involutory CNOT pair
    qis.cx(q0, q1)
    qis.cx(q0, q1)

    # Obfuscation 5: Dead qubit telemetry injection on q2
    qis.h(q2)
    qis.x(q2)
    qis.rz(0.5, q2)

    # Canonical phase
    qis.rz(math.pi / 4.0, q1)

    # Measurement
    qis.mz(q0, r0)
    qis.mz(q1, r1)

    ll_path = os.path.join(OUT_DIR, "obfuscated_circuit.ll")
    bc_path = os.path.join(OUT_DIR, "obfuscated_circuit.bc")

    with open(ll_path, "w") as f:
        f.write(mod.ir())
    with open(bc_path, "wb") as f:
        f.write(mod.bitcode())

    print(f"[✓] Generated obfuscated sample: {ll_path} and {bc_path}")


if __name__ == "__main__":
    generate_bell_state()
    generate_obfuscated_sample()

