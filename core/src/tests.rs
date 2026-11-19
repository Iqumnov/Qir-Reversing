use crate::deobfuscator::QirDeobfuscator;
use crate::emitter::QirEmitter;
use crate::parser::QirParser;

const CLEAN_SAMPLE: &str = r#"
%Qubit = type opaque
%Result = type opaque

define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(%Qubit* null)
  call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__rz__body(double 0x3FE921FB54442D18, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__mz__body(%Qubit* null, %Result* null)
  call void @__quantum__qis__mz__body(%Qubit* inttoptr (i64 1 to %Qubit*), %Result* inttoptr (i64 1 to %Result*))
  ret void
}

declare void @__quantum__qis__h__body(%Qubit*)
declare void @__quantum__qis__cnot__body(%Qubit*, %Qubit*)
declare void @__quantum__qis__rz__body(double, %Qubit*)
declare void @__quantum__qis__mz__body(%Qubit*, %Result*)

attributes #0 = { "entry_point" "required_num_qubits"="2" "required_num_results"="2" }
"#;

const OBFUSCATED_SAMPLE: &str = r#"
%Qubit = type opaque
%Result = type opaque

define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(%Qubit* inttoptr (i64 2 to %Qubit*))
  call void @__quantum__qis__x__body(%Qubit* inttoptr (i64 2 to %Qubit*))
  call void @__quantum__qis__h__body(%Qubit* null)
  call void @__quantum__qis__h__body(%Qubit* null)
  call void @__quantum__qis__h__body(%Qubit* null)
  call void @__quantum__qis__x__body(%Qubit* null)
  call void @__quantum__qis__x__body(%Qubit* null)
  call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__rz__body(double 0x3FF921FB54442D18, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__rz__body(double 0xBFF921FB54442D18, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__rz__body(double 0x3FE921FB54442D18, %Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__mz__body(%Qubit* null, %Result* null)
  call void @__quantum__qis__mz__body(%Qubit* inttoptr (i64 1 to %Qubit*), %Result* inttoptr (i64 1 to %Result*))
  ret void
}

attributes #0 = { "entry_point" "required_num_qubits"="3" "required_num_results"="2" }
"#;

#[test]
fn test_parse_clean_ir() {
    let mut parser = QirParser::new();
    let insts = parser
        .parse_ir(CLEAN_SAMPLE)
        .expect("Failed to parse clean IR");
    assert_eq!(insts.len(), 5);
    assert_eq!(parser.metadata.num_qubits, 2);
    assert_eq!(parser.metadata.num_results, 2);

    // Verify Bell state gate sequence
    assert_eq!(insts[0].gate.mnemonic(), "h q[0]");
    assert_eq!(insts[1].gate.mnemonic(), "cx q[0], q[1]");
    assert!(insts[2].gate.mnemonic().starts_with("rz(0.785398"));
}

#[test]
fn test_deobfuscate_quantum_anomalies() {
    let mut parser = QirParser::new();
    let obf_insts = parser
        .parse_ir(OBFUSCATED_SAMPLE)
        .expect("Failed to parse obf IR");
    assert_eq!(obf_insts.len(), 15);
    assert_eq!(parser.metadata.num_qubits, 3);

    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&obf_insts);

    // All dead qubit operations on q[2] must be pruned
    for inst in &cleaned {
        for q in inst.gate.active_qubits() {
            assert_ne!(q.0, 2, "Dead qubit q[2] was not pruned!");
        }
    }

    // Cleaned circuit must match canonical Bell state instructions exactly
    assert_eq!(cleaned.len(), 5);
    assert_eq!(cleaned[0].gate.mnemonic(), "h q[0]");
    assert_eq!(cleaned[1].gate.mnemonic(), "cx q[0], q[1]");
    assert!(cleaned[2].gate.mnemonic().starts_with("rz(0.785398"));

    // Obfuscation ratio must be positive
    assert!(deobf.obfuscation_ratio > 50.0);
    assert!(!deobf.findings.is_empty());
}

#[test]
fn test_ascii_and_qasm_emitters() {
    let mut parser = QirParser::new();
    let insts = parser
        .parse_ir(CLEAN_SAMPLE)
        .expect("Failed to parse clean IR");

    let ascii = QirEmitter::render_ascii_circuit(&insts);
    assert!(ascii.contains("q[0]: ──[H]───●───"));
    assert!(ascii.contains("q[1]: ────────X───"));

    let qasm = QirEmitter::to_openqasm(&insts, 2, 2);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("cx q[0], q[1];"));
    assert!(qasm.contains("r[0] = measure q[0];"));
}

#[test]
fn test_unitary_matrix_soundness() {
    use crate::types::{QuantumGate, Qubit};
    use crate::unitary::UnitaryMatrix;

    let mut u_h2 = UnitaryMatrix::identity(1);
    u_h2.apply_gate(&QuantumGate::H(Qubit(0)));
    u_h2.apply_gate(&QuantumGate::H(Qubit(0)));
    let id1 = UnitaryMatrix::identity(1);
    assert!(
        u_h2.frobenius_distance(&id1) < 1e-12,
        "H*H must equal Identity"
    );

    let mut u_cx2 = UnitaryMatrix::identity(2);
    u_cx2.apply_gate(&QuantumGate::CX(Qubit(0), Qubit(1)));
    u_cx2.apply_gate(&QuantumGate::CX(Qubit(0), Qubit(1)));
    let id2 = UnitaryMatrix::identity(2);
    assert!(
        u_cx2.frobenius_distance(&id2) < 1e-12,
        "CX*CX must equal Identity"
    );

    // Verify T * T = S
    let mut u_t2 = UnitaryMatrix::identity(1);
    u_t2.apply_gate(&QuantumGate::T(Qubit(0)));
    u_t2.apply_gate(&QuantumGate::T(Qubit(0)));
    let mut u_s = UnitaryMatrix::identity(1);
    u_s.apply_gate(&QuantumGate::S(Qubit(0)));
    assert!(
        u_t2.frobenius_distance(&u_s) < 1e-12,
        "T*T must equal S gate"
    );

    // Verify Reversibility theorem: U * U† = I
    let u_adj = u_t2.adjoint();
    let u_id = u_t2.multiply(&u_adj);
    assert!(
        u_id.frobenius_distance(&id1) < 1e-12,
        "U * U† must equal Identity"
    );
}

#[test]
fn test_process_fidelity_and_unitary_equivalence() {
    use crate::unitary::UnitaryVerifier;

    let mut parser = QirParser::new();
    let clean_insts = parser.parse_ir(CLEAN_SAMPLE).expect("clean IR");
    let mut parser2 = QirParser::new();
    let obf_insts = parser2.parse_ir(OBFUSCATED_SAMPLE).expect("obf IR");

    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&obf_insts);

    let res = UnitaryVerifier::verify_equivalence(&clean_insts, &cleaned, 2);
    assert!(
        res.is_equivalent,
        "Deobfuscated circuit must be unitarily equivalent to clean circuit"
    );
    assert!(
        (1.0 - res.fidelity).abs() < 1e-6,
        "Process fidelity must equal 1.0, got: {}",
        res.fidelity
    );
    assert!(
        res.frobenius_error < 1e-6,
        "Frobenius error must be zero, got: {}",
        res.frobenius_error
    );
}

#[test]
fn test_multiqubit_ascii_vertical_routing_and_svg() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};

    let insts = vec![QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(2)),
        "cx q[0], q[2]",
    )];

    let ascii = QirEmitter::render_ascii_circuit(&insts);
    assert!(ascii.contains("q[0]: ───●───"));
    assert!(
        ascii.contains("q[1]: ───┼───"),
        "Intervening wire q[1] must route vertical control wire with ┼"
    );
    assert!(ascii.contains("q[2]: ───X───"));

    let svg = QirEmitter::render_svg_circuit(&insts, 3);
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>\n"));
    assert!(svg.contains("q[0]"));
    assert!(svg.contains("q[2]"));
}

#[test]
fn test_ssa_qubit_exact_token_matching() {
    // Verify that %1 does NOT collide with %10 or %11
    let ir = r#"
define void @main() {
entry:
  %1 = inttoptr i64 1 to %Qubit*
  %10 = inttoptr i64 10 to %Qubit*
  %11 = inttoptr i64 11 to %Qubit*
  call void @__quantum__qis__h__body(%Qubit* %10)
  call void @__quantum__qis__x__body(%Qubit* %1)
  call void @__quantum__qis__z__body(%Qubit* %11)
  ret void
}
"#;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse SSA IR");
    assert_eq!(insts.len(), 3);
    assert_eq!(insts[0].gate.active_qubits()[0].0, 10);
    assert_eq!(insts[1].gate.active_qubits()[0].0, 1);
    assert_eq!(insts[2].gate.active_qubits()[0].0, 11);
}

#[test]
fn test_advanced_quantum_commutation_and_ccx_involution() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};

    let insts = vec![
        QuantumInstruction::new(
            QuantumGate::CCX(Qubit(0), Qubit(1), Qubit(2)),
            "ccx q[0], q[1], q[2]",
        ),
        QuantumInstruction::new(
            QuantumGate::CCX(Qubit(1), Qubit(0), Qubit(2)),
            "ccx q[1], q[0], q[2]",
        ), // Symmetric controls
        QuantumInstruction::new(QuantumGate::CX(Qubit(0), Qubit(1)), "cx q[0], q[1]"),
        QuantumInstruction::new(QuantumGate::CX(Qubit(0), Qubit(2)), "cx q[0], q[2]"), // Same control, commutes!
        QuantumInstruction::new(QuantumGate::CX(Qubit(0), Qubit(1)), "cx q[0], q[1]"), // Cancels with first CX!
    ];

    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&insts);

    // CCX pair cancels to I, CX(0,1) pairs slide and cancel, leaving only CX(0,2)
    assert_eq!(cleaned.len(), 1);
    assert_eq!(cleaned[0].gate.mnemonic(), "cx q[0], q[2]");
}

#[test]
fn test_verifier_dimension_guard_and_safe_bounds() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    // 1. Hilbert space dimension guard (> 14 qubits)
    let insts = vec![QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]")];
    let res = UnitaryVerifier::verify_equivalence(&insts, &insts, 20);
    assert!(!res.is_equivalent);
    assert!(res.error_message.is_some());
    assert!(res
        .error_message
        .unwrap()
        .contains("exceeds classical matrix memory limits"));

    // 2. Safe gate bounds on empty/small space without underflow
    let mut u = crate::unitary::UnitaryMatrix::identity(1);
    u.apply_gate(&QuantumGate::CX(Qubit(0), Qubit(5))); // Qubit 5 >= 1
    assert_eq!(u.dim, 2); // Unaltered, no panic!
}

#[test]
fn test_qir_intrinsic_aliases_and_tail_call() {
    let ir = r#"
define void @main() {
entry:
  tail call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*))
  musttail call void @__quantum__qis__toffoli__body(%Qubit* null, %Qubit* inttoptr (i64 1 to %Qubit*), %Qubit* inttoptr (i64 2 to %Qubit*))
  call void @__quantum__qis__sdg__body(%Qubit* inttoptr (i64 1 to %Qubit*))
  call void @__quantum__qis__tdg__body(%Qubit* inttoptr (i64 2 to %Qubit*))
  call void @__quantum__qis__m__body(%Qubit* null, %Result* null)
  ret void
}
"#;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse IR with aliases");
    assert_eq!(insts.len(), 5);
    assert_eq!(insts[0].gate.mnemonic(), "cx q[0], q[1]");
    assert_eq!(insts[1].gate.mnemonic(), "ccx q[0], q[1], q[2]");
    assert_eq!(insts[2].gate.mnemonic(), "sdg q[1]");
    assert_eq!(insts[3].gate.mnemonic(), "tdg q[2]");
    assert_eq!(insts[4].gate.mnemonic(), "mz q[0] -> r[0]");
}

#[test]
fn test_discrete_phase_clifford_t_consolidation() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    // Test T + T -> S
    let insts_tt = vec![
        QuantumInstruction::new(QuantumGate::T(Qubit(0)), "t q[0]"),
        QuantumInstruction::new(QuantumGate::T(Qubit(0)), "t q[0]"),
    ];
    let mut deobf = QirDeobfuscator::new();
    let clean_tt = deobf.deobfuscate(&insts_tt);
    assert_eq!(clean_tt.len(), 1);
    assert_eq!(clean_tt[0].gate.mnemonic(), "s q[0]");

    // Unitary equivalence check: T*T == S
    let res_tt = UnitaryVerifier::verify_equivalence(&insts_tt, &clean_tt, 1);
    assert!(res_tt.is_equivalent);
    assert!(res_tt.frobenius_error < 1e-12);

    // Test S + S -> Z
    let insts_ss = vec![
        QuantumInstruction::new(QuantumGate::S(Qubit(0)), "s q[0]"),
        QuantumInstruction::new(QuantumGate::S(Qubit(0)), "s q[0]"),
    ];
    let mut deobf2 = QirDeobfuscator::new();
    let clean_ss = deobf2.deobfuscate(&insts_ss);
    assert_eq!(clean_ss.len(), 1);
    assert_eq!(clean_ss[0].gate.mnemonic(), "z q[0]");

    let res_ss = UnitaryVerifier::verify_equivalence(&insts_ss, &clean_ss, 1);
    assert!(res_ss.is_equivalent);
    assert!(res_ss.frobenius_error < 1e-12);

    // Test Sdg + Z -> S
    let insts_sdgz = vec![
        QuantumInstruction::new(QuantumGate::Sdg(Qubit(0)), "sdg q[0]"),
        QuantumInstruction::new(QuantumGate::Z(Qubit(0)), "z q[0]"),
    ];
    let mut deobf3 = QirDeobfuscator::new();
    let clean_sdgz = deobf3.deobfuscate(&insts_sdgz);
    assert_eq!(clean_sdgz.len(), 1);
    assert_eq!(clean_sdgz[0].gate.mnemonic(), "s q[0]");

    let res_sdgz = UnitaryVerifier::verify_equivalence(&insts_sdgz, &clean_sdgz, 1);
    assert!(res_sdgz.is_equivalent);
    assert!(res_sdgz.frobenius_error < 1e-12);
}

#[test]
fn test_negative_hex_and_scientific_floats() {
    use crate::types::QuantumGate;

    let ir = r#"
define void @main() {
entry:
  call void @__quantum__qis__rz__body(double -0x3FE921FB54442D18, %Qubit* null)
  call void @__quantum__qis__rx__body(double -6.283180e-01, %Qubit* null)
  call void @__quantum__qis__ry__body(double (1.5707963), %Qubit* null)
  ret void
}
"#;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse floats");
    assert_eq!(insts.len(), 3);

    if let QuantumGate::Rz(theta, _) = insts[0].gate {
        assert!(theta < 0.0, "Negative hex float must be negative");
        assert!((theta - (-std::f64::consts::FRAC_PI_4)).abs() < 1e-6);
    } else {
        panic!("Expected Rz gate");
    }

    if let QuantumGate::Rx(theta, _) = insts[1].gate {
        assert!(
            theta < 0.0,
            "Scientific notation must preserve negative sign"
        );
        assert!((theta - (-0.628318)).abs() < 1e-5);
    } else {
        panic!("Expected Rx gate");
    }

    if let QuantumGate::Ry(theta, _) = insts[2].gate {
        assert!((theta - std::f64::consts::FRAC_PI_2).abs() < 1e-5);
    } else {
        panic!("Expected Ry gate");
    }
}
