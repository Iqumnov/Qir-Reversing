#!/usr/bin/env python3
r"""
Quantum Intermediate Representation (QIR) Demo Generator
Student: Igumnov (Group 231-351)
Course: Reverse Engineering of Quantum Bytecode

Generates:
1. Clean Demo Program containing 4 core constructs from Lab 2 (LR2):
   - Superposition (Hadamard gate)
   - Entanglement (Bell state $|\Phi^+\rangle = (|00\rangle + |11\rangle)/\sqrt{2}$ via H + CNOT)
   - Phase rotation ($R_z(\pi/4)$)
   - Measurement in Z-basis ($M_z$) and output recording
2. Obfuscated Demo Program:
   - Involutory Identity Insertion ($H \cdot H = I$, $X \cdot X = I$, $CNOT \cdot CNOT = I$)
   - Dead Qubit Injection ($q_2$ ghost qubit with dummy operations)
   - Redundant Inverse Rotations ($R_z(\theta) \cdot R_z(-\theta) = I$)
"""

import os
import math
import pyqir

OUT_DIR = os.path.dirname(os.path.abspath(__file__))

def generate_clean_demo():
    """Constructs clean LR2 Quantum Demonstration Program."""
    # 2 qubits, 2 results
    mod = pyqir.SimpleModule("lr2_quantum_demo", num_qubits=2, num_results=2)
    qis = pyqir.BasicQisBuilder(mod.builder)
    q0 = mod.qubits[0]
    q1 = mod.qubits[1]
    r0 = mod.results[0]
    r1 = mod.results[1]

    # Construct 1: Superposition (Hadamard gate)
    qis.h(q0)

    # Construct 2: Entanglement (CNOT gate -> Bell state (|00> + |11>)/sqrt(2))
    qis.cx(q0, q1)

    # Construct 3: Phase Rotation (Rz with angle pi/4 = 0.785398)
    qis.rz(math.pi / 4.0, q1)

    # Construct 4: Measurement in computational basis Z
    qis.mz(q0, r0)
    qis.mz(q1, r1)

    # Emit textual LLVM IR and binary LLVM Bitcode
    ll_path = os.path.join(OUT_DIR, "lr2_quantum_demo.ll")
    bc_path = os.path.join(OUT_DIR, "lr2_quantum_demo.bc")

    with open(ll_path, "w") as f:
        f.write(mod.ir())

    with open(bc_path, "wb") as f:
        f.write(mod.bitcode())

    print(f"[✓] Generated Clean QIR Demo:")
    print(f"    - LLVM IR:      {ll_path} ({os.path.getsize(ll_path)} bytes)")
    print(f"    - LLVM Bitcode: {bc_path} ({os.path.getsize(bc_path)} bytes)")


def generate_obfuscated_demo():
    """Constructs Obfuscated LR2 Quantum Program with Anti-Reversing traps."""
    # 3 qubits (q0, q1 real; q2 is a dead/phantom qubit injected for obfuscation), 2 results
    mod = pyqir.SimpleModule("lr2_obfuscated", num_qubits=3, num_results=2)
    qis = pyqir.BasicQisBuilder(mod.builder)
    q0 = mod.qubits[0]
    q1 = mod.qubits[1]
    q2 = mod.qubits[2]  # Dead/ghost qubit!
    r0 = mod.results[0]
    r1 = mod.results[1]

    # --- OBFUSCATION LAYER 1: Dead Qubit Initialization & Dummy Entropy ---
    qis.h(q2)
    qis.x(q2)
    qis.rz(1.570796, q2)

    # --- OBFUSCATION LAYER 2: Involutory Identity Insertion (H * H = I) ---
    qis.h(q0)
    qis.h(q0)  # Cancels previous H

    # --- REAL LOGIC: Construct 1 (Hadamard Superposition) ---
    qis.h(q0)

    # --- OBFUSCATION LAYER 3: Involutory Identity Insertion (X * X = I) ---
    qis.x(q1)
    qis.x(q1)  # Cancels previous X

    # --- REAL LOGIC: Construct 2 (Entanglement CNOT) ---
    qis.cx(q0, q1)

    # --- OBFUSCATION LAYER 4: Duplicate CNOT Cancellation (CNOT * CNOT = I) ---
    qis.cx(q0, q1)
    qis.cx(q0, q1)  # Cancels previous CNOT

    # --- OBFUSCATION LAYER 5: Dummy Inverse Rotations (Rz(theta) * Rz(-theta) = I) ---
    dummy_theta = 0.628318
    qis.rz(dummy_theta, q0)
    qis.rz(-dummy_theta, q0)  # Cancels previous Rz

    # --- REAL LOGIC: Construct 3 (Phase Rotation Rz(pi/4)) ---
    qis.rz(math.pi / 4.0, q1)

    # --- OBFUSCATION LAYER 6: Dead Qubit additional noise ---
    qis.h(q2)
    qis.y(q2)

    # --- REAL LOGIC: Construct 4 (Measurement) ---
    # Note: q2 is NEVER measured! Only q0 and q1 are measured!
    qis.mz(q0, r0)
    qis.mz(q1, r1)

    # Emit textual LLVM IR and binary LLVM Bitcode
    ll_path = os.path.join(OUT_DIR, "lr2_obfuscated.ll")
    bc_path = os.path.join(OUT_DIR, "lr2_obfuscated.bc")

    with open(ll_path, "w") as f:
        f.write(mod.ir())

    with open(bc_path, "wb") as f:
        f.write(mod.bitcode())

    print(f"[✓] Generated Obfuscated QIR Demo:")
    print(f"    - LLVM IR:      {ll_path} ({os.path.getsize(ll_path)} bytes)")
    print(f"    - LLVM Bitcode: {bc_path} ({os.path.getsize(bc_path)} bytes)")


if __name__ == "__main__":
    generate_clean_demo()
    generate_obfuscated_demo()

