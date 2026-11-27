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
    use crate::dd::MAX_DD_QUBITS;
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    // 1. Sub-64 qubit circuit (20 qubits > 12 dense threshold) is now proven via canonical QMDD
    let insts = vec![QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]")];
    let res_20 = UnitaryVerifier::verify_equivalence(&insts, &insts, 20);
    assert!(res_20.is_equivalent);
    assert!(res_20.error_message.is_none());
    assert!((res_20.fidelity - 1.0).abs() < 1e-9);

    // 2. Qubit count beyond MAX_DD_QUBITS is rejected with an explicit safety bound
    let res_over = UnitaryVerifier::verify_equivalence(&insts, &insts, MAX_DD_QUBITS + 1);
    assert!(!res_over.is_equivalent);
    assert!(res_over.error_message.is_some());
    assert!(res_over
        .error_message
        .unwrap()
        .contains("exceeds QMDD verification limit"));

    // 3. Safe gate bounds on empty/small space without underflow
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

#[test]
fn test_parser_reuse_isolation_signed_opcode_and_barrier_qasm() {
    use crate::types::{QuantumGate, QuantumInstruction};

    // 1. Reusing QirParser must reset metadata & SSA pointer maps between parse_ir calls
    //    AND named/numbered SSA tokens (%q_named, %9) after %Qubit* type prefix must resolve accurately.
    let ir_first = r#"
define void @first_entry() #0 {
entry:
  %9 = inttoptr i64 2 to %Qubit*
  %q_named = inttoptr i64 3 to %Qubit*
  call void @__quantum__qis__h__body(%Qubit* %9)
  call void @__quantum__qis__x__body(%Qubit* %q_named)
  ret void
}
attributes #0 = { "required_num_qubits"="4" "required_num_results"="3" }
"#;
    let ir_second = r#"
define void @second_entry() #0 {
entry:
  call void @__quantum__qis__x__body(%Qubit* null)
  ret void
}
attributes #0 = { "required_num_qubits"="1" "required_num_results"="0" }
"#;
    let mut parser = QirParser::new();
    let first = parser.parse_ir(ir_first).expect("first parse");
    assert_eq!(first[0].gate.active_qubits()[0].0, 2);
    assert_eq!(first[1].gate.active_qubits()[0].0, 3);
    assert_eq!(parser.metadata.num_qubits, 4);
    assert_eq!(parser.metadata.entry_point, "first_entry");

    let second = parser.parse_ir(ir_second).expect("second parse");
    assert_eq!(second.len(), 1);
    assert_eq!(parser.metadata.num_qubits, 1);
    assert_eq!(parser.metadata.num_results, 0);
    assert_eq!(parser.metadata.entry_point, "second_entry");

    // 2. Signed rotation synthetic opcode preserves 270° (-π/2) distinct from 90° (+π/2)
    let op_pos =
        QuantumGate::Rz(std::f64::consts::FRAC_PI_2, crate::types::Qubit(0)).synthetic_opcode();
    let op_neg =
        QuantumGate::Rz(-std::f64::consts::FRAC_PI_2, crate::types::Qubit(0)).synthetic_opcode();
    let deg_pos = (op_pos[2] as u16) | ((op_pos[3] as u16) << 8);
    let deg_neg = (op_neg[2] as u16) | ((op_neg[3] as u16) << 8);
    assert_eq!(deg_pos, 90);
    assert_eq!(deg_neg, 270);

    // 3. Empty barrier emits valid "barrier;" in OpenQASM 3.0
    let qasm = QirEmitter::to_openqasm(
        &[QuantumInstruction::new(
            QuantumGate::Barrier(vec![]),
            "barrier",
        )],
        1,
        0,
    );
    assert!(qasm.contains("barrier;\n"));

    // 4. CCX causal pruning keeps controls AND target live if control q[0] is measured
    use crate::types::Qubit;
    let ccx_ir = vec![
        QuantumInstruction::new(QuantumGate::X(Qubit(2)), "x q[2]"),
        QuantumInstruction::new(
            QuantumGate::CCX(Qubit(0), Qubit(1), Qubit(2)),
            "ccx q[0], q[1], q[2]",
        ),
        QuantumInstruction::new(
            QuantumGate::Mz(Qubit(0), crate::types::ResultBit(0)),
            "mz q[0] -> r[0]",
        ),
    ];
    let (live, dead) = QirDeobfuscator::new().prune_dead_qubits(&ccx_ir);
    assert!(
        dead.is_empty(),
        "CCX control measurement must keep all CCX wires live: {:?}",
        dead
    );
    assert_eq!(live.len(), 3);
}

#[test]
fn test_opaque_ptr_comments_and_control_flow_barriers() {
    use crate::parser::QirParser;
    use crate::unitary::UnitaryVerifier;

    let ir = r#"
; Leading comment (with parens)
define void @main() #0 {
entry:
  %q0 = call ptr @__quantum__rt__qubit_allocate() ; allocate qubit 0
  %q1 = inttoptr i64 1 to ptr
  call void @__quantum__qis__h__body(ptr %q0) ; inline comment
  call void @__quantum__qis__h__body(ptr %q0)
  br label %next
next:
  call void @__quantum__qis__h__body(ptr %q0)
  br label %tail
tail:
  call void @__quantum__qis__h__body(ptr %q0)
  call void @__quantum__qis__mz__body(ptr %q0, ptr null)
  ret void
}
attributes #0 = { "entry_point" "required_num_qubits"="2" "required_num_results"="1" }
"#;

    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse opaque ptr IR");
    let mut deobf = QirDeobfuscator::new();
    let cleaned = deobf.deobfuscate(&insts);

    // The two H gates in `entry` cancel, whereas H in `%next` and H in `%tail` MUST NOT cancel across `br`/`tail:`!
    let h_count = cleaned
        .iter()
        .filter(|i| matches!(i.gate, crate::types::QuantumGate::H(_)))
        .count();
    assert_eq!(
        h_count, 2,
        "Control-flow boundaries must block cross-block H*H cancellation"
    );

    // Non-unitary splitting rejects mismatched Mz ordering: [H, Mz, X, Mz] vs [H, X, Mz, Mz]
    use crate::types::{QuantumGate, QuantumInstruction, Qubit, ResultBit};
    let circ_a = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(0)), "mz q[0] -> r[0]"),
        QuantumInstruction::new(QuantumGate::X(Qubit(0)), "x q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(1)), "mz q[0] -> r[1]"),
    ];
    let circ_b = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::X(Qubit(0)), "x q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(0)), "mz q[0] -> r[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(1)), "mz q[0] -> r[1]"),
    ];
    let split_check = UnitaryVerifier::verify_equivalence(&circ_a, &circ_b, 1);
    assert!(
        !split_check.is_equivalent,
        "Verifier must split unitary segments across Mz boundaries"
    );

    // Global phase invariance: Rz(2pi) = -I == e^{i*pi} I
    let rz_2pi = vec![QuantumInstruction::new(
        QuantumGate::Rz(std::f64::consts::TAU, Qubit(0)),
        "rz(2pi) q[0]",
    )];
    let identity: Vec<QuantumInstruction> = vec![];
    let phase_res = UnitaryVerifier::verify_equivalence(&rz_2pi, &identity, 1);
    assert!(phase_res.is_equivalent);
    assert!(phase_res.frobenius_error < 1e-12);

    // Exact original indices survive dead-qubit pruning even when manual instructions all have address == 0
    let zero_addr_stream = vec![
        QuantumInstruction::new(QuantumGate::X(Qubit(2)), "x q[2]"),
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(0)), "mz q[0] -> r[0]"),
    ];
    let mut deobf_idx = QirDeobfuscator::new();
    let cleaned_idx = deobf_idx.deobfuscate(&zero_addr_stream);
    assert_eq!(cleaned_idx.len(), 1);
    assert_eq!(deobf_idx.findings[0].instruction_indices, vec![0]);
    assert_eq!(deobf_idx.findings[1].instruction_indices, vec![1, 2]);

    // Control-flow self-equivalence must pass (SegmentBoundary::ControlFlow matches identical cf gates)
    let cf_verify = UnitaryVerifier::verify_equivalence(&insts, &cleaned, 2);
    assert!(
        cf_verify.is_equivalent,
        "Identical control-flow fences must verify equivalent: {:?}",
        cf_verify
    );

    // Non-finite rotation parameters must fail unitary verification instead of being ignored by NaN.max/min
    let nan_circ = vec![QuantumInstruction::new(
        QuantumGate::Rz(f64::NAN, Qubit(0)),
        "rz(NaN) q[0]",
    )];
    let nan_res = UnitaryVerifier::verify_equivalence(&nan_circ, &[], 1);
    assert!(!nan_res.is_equivalent);
    assert!(nan_res.error_message.is_some());

    // Disjoint gate around mid-circuit Mz stays in its own causal unitary epoch so verifier matches
    let disjoint_mz = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(1), ResultBit(1)), "mz q[1] -> r[1]"),
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(0)), "mz q[0] -> r[0]"),
    ];
    let mut deobf_mz = QirDeobfuscator::new();
    let cleaned_mz = deobf_mz.deobfuscate(&disjoint_mz);
    let (live_mz, _) = deobf_mz.prune_dead_qubits(&disjoint_mz);
    let mz_verify = UnitaryVerifier::verify_equivalence(&live_mz, &cleaned_mz, 2);
    assert!(
        mz_verify.is_equivalent,
        "Deobfuscated circuit across Mz must verify: {:?}",
        mz_verify
    );

    // SVG Custom gate escaping with multi-byte UTF-8 and '<' must not panic or split XML entities
    let custom_circ = vec![QuantumInstruction::new(
        QuantumGate::Custom {
            name: "α<β>\"&custom_long".to_string(),
            qubits: vec![Qubit(0)],
            results: vec![],
            params: vec![],
        },
        "call void @custom()",
    )];
    let svg_out = QirEmitter::render_svg_circuit(&custom_circ, 1);
    assert!(svg_out.contains("&lt;") && svg_out.contains("&gt;") && svg_out.contains("&amp;"));

    // Unbalanced parens in intrinsic call must return Err(MalformedSyntax), never panic
    let mut bad_parser = QirParser::new();
    assert!(bad_parser
        .parse_ir("call void @__quantum__qis__h__body(ptr inttoptr (i64 0 to ptr) ; unclosed\n")
        .is_err());

    // Non-finite LLVM float literal (NaN / Inf) must return Err(InvalidFloatLiteral)
    assert!(bad_parser
        .parse_ir("call void @__quantum__qis__rz__body(double 0x7FF8000000000000, ptr null)\n")
        .is_err());

    // Function-boundary isolation + allocate_array + comment inside string literal + attribute boundary (#1 vs #10)
    let multi_fn_ir = r#"
define void @helper() #10 {
entry:
  %arr = call ptr @__quantum__rt__qubit_allocate_array(i64 2)
  %lit = private constant [4 x i8] c"a;b\00"
  ret void
}
define void @real_entry() #1 {
entry:
  %q0 = call ptr @__quantum__rt__qubit_allocate()
  call void @__quantum__qis__h__body(ptr %q0)
  call void @__quantum__qis__mz__body(ptr %q0, ptr null)
  ret void
}
attributes #1 = { "entry_point" }
attributes #10 = { "unrelated" }
"#;
    let mut mf_parser = QirParser::new();
    let mf_insts = mf_parser.parse_ir(multi_fn_ir).expect("multi-fn parse");
    assert_eq!(mf_parser.metadata.entry_point, "real_entry");
    assert_eq!(mf_insts.len(), 2);
    assert_eq!(mf_insts[0].gate, QuantumGate::H(Qubit(0)));

    // Huge qubit index SVG/ASCII guard must not OOM
    let huge_circ = vec![QuantumInstruction::new(
        QuantumGate::H(Qubit(u32::MAX - 1)),
        "h q[max]",
    )];
    assert!(QirEmitter::render_ascii_circuit(&huge_circ)
        .contains("exceeds ASCII diagram rendering limit"));
    assert!(QirEmitter::render_svg_circuit(&huge_circ, u32::MAX - 1)
        .contains("exceeds SVG rendering limit"));

    // Trivial identity rotations (0, 2π) are stripped and preserve unitary equivalence
    let triv_rot = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::Rz(0.0, Qubit(0)), "rz(0.0) q[0]"),
        QuantumInstruction::new(
            QuantumGate::Rx(std::f64::consts::TAU, Qubit(0)),
            "rx(2pi) q[0]",
        ),
        QuantumInstruction::new(QuantumGate::Mz(Qubit(0), ResultBit(0)), "mz q[0] -> r[0]"),
    ];
    let (live_triv, _) = QirDeobfuscator::new().prune_dead_qubits(&triv_rot);
    let mut deobf_triv = QirDeobfuscator::new();
    let cleaned_triv = deobf_triv.deobfuscate(&triv_rot);
    assert_eq!(cleaned_triv.len(), 2);
    let triv_verify = UnitaryVerifier::verify_equivalence(&live_triv, &cleaned_triv, 1);
    assert!(triv_verify.is_equivalent);

    // Degenerate self-operand multi-qubit gate must fail verification cleanly
    let bad_cx = vec![QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(0)),
        "cx q[0], q[0]",
    )];
    let dup_res = UnitaryVerifier::verify_equivalence(&bad_cx, &bad_cx, 1);
    assert!(!dup_res.is_equivalent);
    assert!(dup_res
        .error_message
        .as_deref()
        .unwrap_or("")
        .contains("distinct operand qubits"));

    // Typed pointer name containing digits does not misresolve to Qubit(64) when SSA var is unbound
    let unbound_ir = r#"
define void @main() {
entry:
  call void @__quantum__qis__h__body(%type.Qubit64* %unmapped)
  ret void
}
"#;
    let mut unbound_parser = QirParser::new();
    let unbound_insts = unbound_parser.parse_ir(unbound_ir).expect("parse unbound");
    assert_eq!(unbound_insts[0].gate, QuantumGate::H(Qubit(0)));
}

#[test]
fn test_adjoint_rotations_and_discrete_phase_classification() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    let ir = r#"
define void @main() {
entry:
  call void @__quantum__qis__rx__body(double 1.250000e+00, %Qubit* null)
  call void @__quantum__qis__rx__adj(double 1.250000e+00, %Qubit* null)
  call void @__quantum__qis__ry__adj(double -5.000000e-01, %Qubit* null)
  call void @__quantum__qis__rz__adj(double 0x3FE921FB54442D18, %Qubit* null)
  ret void
}
"#;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse adjoint rotations");
    assert_eq!(insts.len(), 4);
    match insts[1].gate {
        QuantumGate::Rx(theta, Qubit(0)) => assert!((theta + 1.25).abs() < 1e-12),
        ref other => panic!("expected Rx(-1.25), got {:?}", other),
    }
    match insts[2].gate {
        QuantumGate::Ry(theta, Qubit(0)) => assert!((theta - 0.5).abs() < 1e-12),
        ref other => panic!("expected Ry(0.5), got {:?}", other),
    }
    match insts[3].gate {
        QuantumGate::Rz(theta, Qubit(0)) => {
            assert!((theta + std::f64::consts::FRAC_PI_4).abs() < 1e-12)
        }
        ref other => panic!("expected Rz(-pi/4), got {:?}", other),
    }

    // S·Sdg and T·Tdg must be classified as DISCRETE_PHASE_CANCELLATION, never IDENTITY_INVOLUTION_PAIR
    let phase_pairs = vec![
        QuantumInstruction::new(QuantumGate::S(Qubit(0)), "s q[0]"),
        QuantumInstruction::new(QuantumGate::Sdg(Qubit(0)), "sdg q[0]"),
        QuantumInstruction::new(QuantumGate::T(Qubit(0)), "t q[0]"),
        QuantumInstruction::new(QuantumGate::Tdg(Qubit(0)), "tdg q[0]"),
    ];
    let mut deobf_phase = QirDeobfuscator::new();
    let cleaned_phase = deobf_phase.deobfuscate(&phase_pairs);
    assert!(cleaned_phase.is_empty());
    assert!(deobf_phase
        .findings
        .iter()
        .all(|f| f.pattern_type == "DISCRETE_PHASE_CANCELLATION"));
    assert_eq!(deobf_phase.findings.len(), 2);

    // Multi-depth commuting cancellation: S · Z · T · Tdg · Sdg -> Z
    let nested = vec![
        QuantumInstruction::new(QuantumGate::S(Qubit(0)), "s q[0]"),
        QuantumInstruction::new(QuantumGate::Z(Qubit(0)), "z q[0]"),
        QuantumInstruction::new(QuantumGate::T(Qubit(0)), "t q[0]"),
        QuantumInstruction::new(QuantumGate::Tdg(Qubit(0)), "tdg q[0]"),
        QuantumInstruction::new(QuantumGate::Sdg(Qubit(0)), "sdg q[0]"),
    ];
    let mut deobf_nested = QirDeobfuscator::new();
    let cleaned_nested = deobf_nested.deobfuscate(&nested);
    assert_eq!(cleaned_nested.len(), 1);
    assert_eq!(cleaned_nested[0].gate, QuantumGate::Z(Qubit(0)));
    let eq_nested = UnitaryVerifier::verify_equivalence(&nested, &cleaned_nested, 1);
    assert!(eq_nested.is_equivalent);
    assert!(eq_nested.frobenius_error < 1e-12);
}

#[test]
fn test_release_hardening_guards_and_soundness() {
    use crate::emitter::QirEmitter;
    use crate::types::{QuantumGate, QuantumInstruction, Qubit, ResultBit};

    // 1. Barrier opcode parity (0x50, aligned with Python & Rizin C-ABI)
    assert_eq!(
        QuantumGate::Barrier(vec![Qubit(0)]).synthetic_opcode(),
        [0x50, 0, 0, 0]
    );

    // 2. render_ascii_circuit bounds guard on huge num_results prevents OOM
    let oom_inst = vec![QuantumInstruction::new(
        QuantumGate::Mz(Qubit(0), ResultBit(999_999)),
        "mz q[0] -> r[999999]",
    )];
    let ascii = QirEmitter::render_ascii_circuit(&oom_inst);
    assert!(ascii.contains("exceeds ASCII diagram rendering limit"));

    // 3. Opaque ptr Result binding, bitcast alias, global string non-call isolation, mresetz & __adj gates
    let ir = r#"
@.str = private constant [40 x i8] c"call void @__quantum__qis__h__body()\00"
define void @main() #0 {
entry:
  %q0 = inttoptr i64 0 to ptr
  %q0_alias = bitcast ptr %q0 to ptr
  %r2 = inttoptr i64 2 to ptr
  call void @__quantum__qis__h__adj(ptr %q0_alias)
  call void @__quantum__qis__cnot__adj(ptr %q0_alias, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__mresetz__body(ptr %q0_alias, ptr %r2)
  call void @__quantum__qis__mz__body(ptr inttoptr (i64 1 to ptr), ptr %r2)
  ret void
}
attributes #0 = { "entry_point" "required_num_qubits"="2" "required_num_results"="3" }
"#;
    let mut parser = QirParser::new();
    let insts = parser.parse_ir(ir).expect("parse hardening IR");
    assert_eq!(insts.len(), 4); // global string call ignored
    assert_eq!(insts[0].gate, QuantumGate::H(Qubit(0)));
    assert_eq!(insts[1].gate, QuantumGate::CX(Qubit(0), Qubit(1)));
    assert!(matches!(
        &insts[2].gate,
        QuantumGate::Custom { name, results, .. } if name == "mresetz" && results == &vec![ResultBit(2)]
    ));
    assert_eq!(insts[2].gate.synthetic_opcode(), [0x30, 0, 2, 0]);
    assert_eq!(insts[3].gate, QuantumGate::Mz(Qubit(1), ResultBit(2)));
    let mr_qasm = QirEmitter::to_openqasm(&insts[2..3], 1, 0);
    assert!(mr_qasm.contains("bit[3] r;"));
    assert!(mr_qasm.contains("r[2] = measure q[0];"));
    assert!(mr_qasm.contains("reset q[0];"));

    // 4. Custom readout intrinsic (mresetz) must not be pruned as a dead qubit
    let readout_only = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(
            QuantumGate::Custom {
                name: "mresetz".to_string(),
                qubits: vec![Qubit(0)],
                results: vec![ResultBit(0)],
                params: vec![],
            },
            "mresetz q[0] -> r[0]",
        ),
    ];
    let mut deobf = QirDeobfuscator::new();
    let kept = deobf.deobfuscate(&readout_only);
    assert_eq!(kept.len(), 2);

    // 5. Malformed arity and malformed hex floats must return errors, never fabricate q[0]/0.0
    assert!(parser
        .parse_ir("define void @main() {\n  call void @__quantum__qis__cnot__body(ptr null)\n}\n")
        .is_err());
    assert!(parser
        .parse_ir("define void @main() {\n  call void @__quantum__qis__rz__body(double 0x123zzz, ptr null)\n}\n")
        .is_err());

    // 6. Control-flow boundaries emit comments rather than fake barriers in OpenQASM
    let mut cf_inst = QuantumInstruction::new(QuantumGate::Barrier(vec![]), "br label %next");
    cf_inst.is_control_flow = true;
    let qasm = QirEmitter::to_openqasm(&[cf_inst], 1, 0);
    assert!(qasm.contains("// control-flow: br label %next"));
    assert!(!qasm.contains("barrier;"));
}

#[test]
fn test_pure_rust_bitcode_decoder_parity_and_wrapper() {
    use std::path::Path;

    let bell_bc = include_bytes!("../../samples/bell_state.bc");
    let bell_ll = include_str!("../../samples/bell_state.ll");
    let obf_bc = include_bytes!("../../samples/obfuscated_circuit.bc");
    let obf_ll = include_str!("../../samples/obfuscated_circuit.ll");

    assert!(crate::bitcode::is_llvm_bitcode(bell_bc));
    assert!(!crate::bitcode::is_llvm_bitcode(bell_ll.as_bytes()));

    let decoded_bell_ir =
        crate::bitcode::decode_bitcode_to_ir(bell_bc).expect("pure-Rust decode bell_state.bc");
    let decoded_obf_ir = crate::bitcode::decode_bitcode_to_ir(obf_bc)
        .expect("pure-Rust decode obfuscated_circuit.bc");

    assert!(decoded_bell_ir.contains("@__quantum__qis__h__body"));
    assert!(decoded_obf_ir.contains("\"entry_point\""));
    assert!(decoded_obf_ir.contains("\"required_num_qubits\"=\"3\""));

    let mut p_bc = QirParser::new();
    let mut p_ll = QirParser::new();
    let bell_bc_insts = p_bc
        .parse_bytes(bell_bc, Path::new("virtual_bell.bc"))
        .expect("parse bell_state.bc via pure-Rust bitcode path");
    let bell_ll_insts = p_ll.parse_ir(bell_ll).expect("parse bell_state.ll");
    assert_eq!(bell_bc_insts.len(), bell_ll_insts.len());
    for (a, b) in bell_bc_insts.iter().zip(bell_ll_insts.iter()) {
        assert_eq!(a.gate.mnemonic(), b.gate.mnemonic());
    }
    assert_eq!(p_bc.metadata.num_qubits, p_ll.metadata.num_qubits);
    assert_eq!(p_bc.metadata.entry_point, p_ll.metadata.entry_point);

    let obf_bc_insts = p_bc
        .parse_bytes(obf_bc, Path::new("virtual_obf.bc"))
        .expect("parse obfuscated_circuit.bc via pure-Rust bitcode path");
    let obf_ll_insts = p_ll.parse_ir(obf_ll).expect("parse obfuscated_circuit.ll");
    assert_eq!(obf_bc_insts.len(), obf_ll_insts.len());
    for (a, b) in obf_bc_insts.iter().zip(obf_ll_insts.iter()) {
        assert_eq!(a.gate.mnemonic(), b.gate.mnemonic());
    }

    // Wrap bell_bc inside an LLVM Darwin/object wrapper (0x0B17C0DE LE at offset 32)
    let mut wrapped = vec![0u8; 32];
    wrapped[0..4].copy_from_slice(&crate::bitcode::LLVM_WRAPPER_MAGIC);
    wrapped[8..12].copy_from_slice(&32u32.to_le_bytes());
    wrapped[12..16].copy_from_slice(&(bell_bc.len() as u32).to_le_bytes());
    wrapped.extend_from_slice(bell_bc);
    assert!(crate::bitcode::is_llvm_bitcode(&wrapped));
    let wrapped_insts = p_bc
        .parse_bytes(&wrapped, Path::new("wrapped.bc"))
        .expect("parse wrapped LLVM bitcode");
    assert_eq!(wrapped_insts.len(), bell_ll_insts.len());

    // Corrupted/truncated wrapper & truncated stream must error gracefully without panicking
    let mut bad_wrapper = wrapped[..20].to_vec();
    bad_wrapper[12..16].copy_from_slice(&99999u32.to_le_bytes());
    assert!(crate::bitcode::decode_bitcode_to_ir(&bad_wrapper).is_err());
    assert!(crate::bitcode::decode_bitcode_to_ir(&bell_bc[..16]).is_err());
}

#[test]
fn test_qmdd_differential_equivalence_against_dense_matrix() {
    use crate::dd::verify_equivalence_dd;
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    // Cover reversed control/target permutations, rotations, SWAP, CCX, and deobfuscation
    let orig = vec![
        QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]"),
        QuantumInstruction::new(QuantumGate::S(Qubit(1)), "s q[1]"),
        QuantumInstruction::new(QuantumGate::T(Qubit(2)), "t q[2]"),
        QuantumInstruction::new(QuantumGate::Rx(0.7, Qubit(3)), "rx(0.7) q[3]"),
        QuantumInstruction::new(QuantumGate::Ry(-0.4, Qubit(4)), "ry(-0.4) q[4]"),
        QuantumInstruction::new(QuantumGate::Rz(1.2, Qubit(5)), "rz(1.2) q[5]"),
        QuantumInstruction::new(QuantumGate::CX(Qubit(4), Qubit(1)), "cx q[4], q[1]"),
        QuantumInstruction::new(QuantumGate::X(Qubit(2)), "x q[2]"),
        QuantumInstruction::new(QuantumGate::X(Qubit(2)), "x q[2]"),
        QuantumInstruction::new(QuantumGate::CZ(Qubit(5), Qubit(0)), "cz q[5], q[0]"),
        QuantumInstruction::new(QuantumGate::SWAP(Qubit(3), Qubit(1)), "swap q[3], q[1]"),
        QuantumInstruction::new(
            QuantumGate::CCX(Qubit(5), Qubit(0), Qubit(2)),
            "ccx q[5], q[0], q[2]",
        ),
        QuantumInstruction::new(QuantumGate::Sdg(Qubit(1)), "sdg q[1]"),
        QuantumInstruction::new(QuantumGate::Tdg(Qubit(2)), "tdg q[2]"),
    ];

    let mut deobf = QirDeobfuscator::new();
    let clean = deobf.deobfuscate(&orig);
    assert!(clean.len() < orig.len());

    let dense_eq = UnitaryVerifier::verify_equivalence(&orig, &clean, 6);
    let dd_eq = verify_equivalence_dd(&orig, &clean, 6);
    assert!(dense_eq.is_equivalent);
    assert!(dd_eq.is_equivalent);
    assert!((dense_eq.fidelity - dd_eq.fidelity).abs() < 1e-10);
    assert!((dense_eq.frobenius_error - dd_eq.frobenius_error).abs() < 1e-9);
    assert!(dd_eq.unitarity_error < 1e-9);

    // Subtle fault (perturbing Rx by 0.15 rad) must diverge closely with dense fidelity
    let mut faulty = clean.clone();
    faulty[3] = QuantumInstruction::new(QuantumGate::Rx(0.85, Qubit(3)), "rx(0.85) q[3]");
    let dense_bad = UnitaryVerifier::verify_equivalence(&orig, &faulty, 6);
    let dd_bad = verify_equivalence_dd(&orig, &faulty, 6);
    assert!(!dense_bad.is_equivalent);
    assert!(!dd_bad.is_equivalent);
    assert!((dense_bad.fidelity - dd_bad.fidelity).abs() < 1e-9);
    assert!((dense_bad.frobenius_error - dd_bad.frobenius_error).abs() < 1e-8);
}

#[test]
fn test_qmdd_large_scale_32_and_64_qubits() {
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};
    use crate::unitary::UnitaryVerifier;

    for n in [32usize, 64usize] {
        let mut orig = vec![QuantumInstruction::new(QuantumGate::H(Qubit(0)), "h q[0]")];
        for q in 0..(n - 1) {
            orig.push(QuantumInstruction::new(
                QuantumGate::CX(Qubit(q as u32), Qubit((q + 1) as u32)),
                format!("cx q[{}], q[{}]", q, q + 1),
            ));
            if q % 5 == 0 {
                // Inject identity obfuscation pairs that deobfuscator cancels
                orig.push(QuantumInstruction::new(
                    QuantumGate::X(Qubit(q as u32)),
                    format!("x q[{}]", q),
                ));
                orig.push(QuantumInstruction::new(
                    QuantumGate::X(Qubit(q as u32)),
                    format!("x q[{}]", q),
                ));
            }
        }
        orig.push(QuantumInstruction::new(
            QuantumGate::Rz(0.37, Qubit((n - 1) as u32)),
            format!("rz(0.37) q[{}]", n - 1),
        ));

        let mut deobf = QirDeobfuscator::new();
        let clean = deobf.deobfuscate(&orig);
        let res = UnitaryVerifier::verify_equivalence(&orig, &clean, n);
        assert!(
            res.is_equivalent,
            "{}-qubit GHZ+Rz verification failed: {:?}",
            n, res.error_message
        );
        assert!((res.fidelity - 1.0).abs() < 1e-9);

        // Single bit-flip fault on the final qubit must be caught cleanly
        let mut faulty = clean.clone();
        faulty.push(QuantumInstruction::new(
            QuantumGate::X(Qubit((n - 1) as u32)),
            format!("x q[{}]", n - 1),
        ));
        let bad = UnitaryVerifier::verify_equivalence(&orig, &faulty, n);
        assert!(!bad.is_equivalent);
        assert!(bad.fidelity < 1e-6);
    }
}

#[test]
fn test_qmdd_64qubit_hadamard_layer_and_skewed_obfuscation() {
    use crate::dd::verify_equivalence_dd;
    use crate::types::{QuantumGate, QuantumInstruction, Qubit};

    // 1. Full 64-qubit Hadamard layer (H^{\otimes 64}) paired with redundant Z-X-Z-X
    // identity gadgets: tests that dd_mul does NOT prematurely prune outer_w = 2^{-32} / 2^{-64}.
    let n64 = 64usize;
    let mut h_clean = Vec::with_capacity(n64);
    let mut h_obf = Vec::with_capacity(n64 + 12);
    for q in 0..n64 {
        let g = QuantumInstruction::new(QuantumGate::H(Qubit(q as u32)), format!("h q[{q}]"));
        h_clean.push(g.clone());
        h_obf.push(g);
        if q % 16 == 0 {
            h_obf.push(QuantumInstruction::new(
                QuantumGate::X(Qubit(q as u32)),
                format!("x q[{q}]"),
            ));
            h_obf.push(QuantumInstruction::new(
                QuantumGate::X(Qubit(q as u32)),
                format!("x q[{q}]"),
            ));
        }
    }
    let res_h64 = verify_equivalence_dd(&h_obf, &h_clean, n64);
    assert!(
        res_h64.is_equivalent,
        "64-qubit Hadamard layer failed: {:?}",
        res_h64.error_message
    );
    assert!((res_h64.fidelity - 1.0).abs() < 1e-9);
    assert_eq!(
        res_h64.verification_method.as_deref(),
        Some("Canonical QMDD")
    );

    // Faulty version on qubit 63 (missing H) must be caught cleanly.
    let mut h_faulty = h_clean.clone();
    h_faulty.pop();
    let res_h64_bad = verify_equivalence_dd(&h_obf, &h_faulty, n64);
    assert!(!res_h64_bad.is_equivalent);
    assert!(res_h64_bad.fidelity < 1e-6);

    // 2. Heavily skewed obfuscation on 40 qubits: all 20 redundant T/Tdg + CNOT-CNOT + H-H
    // pairs placed at the front before the clean circuit runs, testing identity-seeking lookahead.
    let n40 = 40usize;
    let mut skewed_orig = Vec::new();
    let mut skewed_clean = Vec::new();
    for q in 0..10 {
        let qa = Qubit((q * 2) as u32);
        let qb = Qubit((q * 2 + 1) as u32);
        skewed_orig.push(QuantumInstruction::new(QuantumGate::H(qa), "h"));
        skewed_orig.push(QuantumInstruction::new(QuantumGate::H(qa), "h"));
        skewed_orig.push(QuantumInstruction::new(QuantumGate::CX(qa, qb), "cx"));
        skewed_orig.push(QuantumInstruction::new(QuantumGate::CX(qa, qb), "cx"));
    }
    for q in 0..n40 {
        let qi = QuantumInstruction::new(QuantumGate::H(Qubit(q as u32)), format!("h q[{q}]"));
        skewed_orig.push(qi.clone());
        skewed_clean.push(qi);
    }
    for q in 0..(n40 - 1) {
        let qi = QuantumInstruction::new(
            QuantumGate::CX(Qubit(q as u32), Qubit((q + 1) as u32)),
            format!("cx q[{q}], q[{}]", q + 1),
        );
        skewed_orig.push(qi.clone());
        skewed_clean.push(qi);
    }
    let res_skewed = verify_equivalence_dd(&skewed_orig, &skewed_clean, n40);
    assert!(
        res_skewed.is_equivalent,
        "40-qubit skewed obfuscation failed: {:?}",
        res_skewed.error_message
    );
    assert!((res_skewed.fidelity - 1.0).abs() < 1e-9);
}

#[test]
fn test_wasm_bridge_empty_and_bitcode_validation() {
    use crate::wasm_bridge::{
        qir_wasm_decode_bitcode, qir_wasm_deobfuscate, qir_wasm_disassemble, qir_wasm_render_ascii,
        qir_wasm_render_svg, qir_wasm_verify,
    };

    assert!(qir_wasm_disassemble("").is_err());
    assert!(qir_wasm_disassemble("   \n\t ").is_err());
    assert!(qir_wasm_disassemble("hello world").is_err());
    assert!(qir_wasm_deobfuscate("").is_err());
    assert!(qir_wasm_render_ascii("").is_err());
    assert!(qir_wasm_render_svg("").is_err());
    assert!(qir_wasm_verify("", "", 1).is_err());
    assert!(qir_wasm_verify("hello world", "", 1).is_err());
    assert!(qir_wasm_decode_bitcode(b"").is_err());
    assert!(qir_wasm_decode_bitcode(b"define void @main() { ret void }").is_err());
}

struct TestBitWriter {
    buf: Vec<u8>,
    bit_pos: usize,
}

impl TestBitWriter {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            bit_pos: 0,
        }
    }

    fn write_bits(&mut self, mut val: u64, width: u32) {
        for _ in 0..width {
            let byte_idx = self.bit_pos >> 3;
            let bit_idx = self.bit_pos & 7;
            if byte_idx >= self.buf.len() {
                self.buf.push(0);
            }
            if (val & 1) != 0 {
                self.buf[byte_idx] |= 1 << bit_idx;
            }
            val >>= 1;
            self.bit_pos += 1;
        }
    }

    fn write_vbr(&mut self, mut val: u64, width: u32) {
        let mask = (1u64 << (width - 1)) - 1;
        let cont = 1u64 << (width - 1);
        loop {
            let chunk = val & mask;
            val >>= width - 1;
            if val == 0 {
                self.write_bits(chunk, width);
                break;
            }
            self.write_bits(chunk | cont, width);
        }
    }

    fn align32(&mut self) {
        let rem = self.bit_pos % 32;
        if rem != 0 {
            self.write_bits(0, (32 - rem) as u32);
        }
    }

    fn begin_bitcode() -> Self {
        let mut w = Self::new();
        for &b in b"BC\xC0\xDE" {
            w.write_bits(b as u64, 8);
        }
        w
    }

    fn enter_block(&mut self, parent_aw: u32, block_id: u64, new_aw: u32) -> (usize, usize) {
        self.write_bits(1, parent_aw);
        self.write_vbr(block_id, 8);
        self.write_vbr(new_aw as u64, 4);
        self.align32();
        let wb = self.buf.len();
        self.write_bits(0, 32);
        (wb, self.bit_pos)
    }

    fn end_block(&mut self, aw: u32, wb: usize, start_bit: usize) {
        self.write_bits(0, aw);
        self.align32();
        let words = ((self.bit_pos - start_bit) / 32) as u32;
        self.buf[wb..wb + 4].copy_from_slice(&words.to_le_bytes());
    }

    fn write_unabbrev(&mut self, aw: u32, code: u64, ops: &[u64]) {
        self.write_bits(3, aw);
        self.write_vbr(code, 6);
        self.write_vbr(ops.len() as u64, 6);
        for &op in ops {
            self.write_vbr(op, 6);
        }
    }
}

#[test]
fn test_bitcode_cyclic_type_and_vbr_width_hardening() {
    use crate::bitcode::decode_bitcode_to_ir;

    // 1. Self-referential Pointer type #0 (pointee = 0) referenced by global + function
    //    must terminate cleanly without stack overflow or panic.
    let mut w = TestBitWriter::new();
    for &b in b"BC\xC0\xDE" {
        w.write_bits(b as u64, 8);
    }
    // ENTER_SUBBLOCK MODULE_BLOCK(8), new_aw=4
    w.write_bits(1, 2);
    w.write_vbr(8, 8);
    w.write_vbr(4, 4);
    w.align32();
    let mod_words_byte = w.buf.len();
    w.write_bits(0, 32);
    let mod_start_bit = w.bit_pos;

    // Nested TYPE_BLOCK_ID_NEW(17), new_aw=4
    w.write_bits(1, 4);
    w.write_vbr(17, 8);
    w.write_vbr(4, 4);
    w.align32();
    let ty_words_byte = w.buf.len();
    w.write_bits(0, 32);
    let ty_start_bit = w.bit_pos;

    // TYPE_CODE_POINTER(8): pointee=0 (self-referential!)
    w.write_bits(3, 4);
    w.write_vbr(8, 6);
    w.write_vbr(1, 6);
    w.write_vbr(0, 6);
    // END_BLOCK(0)
    w.write_bits(0, 4);
    w.align32();
    let ty_words = ((w.bit_pos - ty_start_bit) / 32) as u32;
    w.buf[ty_words_byte..ty_words_byte + 4].copy_from_slice(&ty_words.to_le_bytes());

    // MODULE_CODE_VERSION(1) = 1
    w.write_bits(3, 4);
    w.write_vbr(1, 6);
    w.write_vbr(1, 6);
    w.write_vbr(1, 6);
    // MODULE_CODE_GLOBALVAR(7): raw_ty=0, flags=0, init=0, linkage=0 (exercises format_type(0))
    w.write_bits(3, 4);
    w.write_vbr(7, 6);
    w.write_vbr(4, 6);
    w.write_vbr(0, 6);
    w.write_vbr(0, 6);
    w.write_vbr(0, 6);
    w.write_vbr(0, 6);
    // MODULE_CODE_FUNCTION(8): fn_ty=0 (cyclic pointer), cc=0, is_proto=1 (exercises fn_signature_parts(0))
    w.write_bits(3, 4);
    w.write_vbr(8, 6);
    w.write_vbr(3, 6);
    w.write_vbr(0, 6);
    w.write_vbr(0, 6);
    w.write_vbr(1, 6);

    w.write_bits(0, 4);
    w.align32();
    let mod_words = ((w.bit_pos - mod_start_bit) / 32) as u32;
    w.buf[mod_words_byte..mod_words_byte + 4].copy_from_slice(&mod_words.to_le_bytes());

    let ir = decode_bitcode_to_ir(&w.buf).expect("cyclic type bitcode should decode safely");
    assert!(ir.contains("ptr"));

    // 2. Crafted VBR(5) width > u32::MAX (0x1_0000_0008) in DEFINE_ABBREV Fixed op
    //    must fail explicitly rather than truncating to 8 on 32-bit/64-bit targets.
    let mut bad = TestBitWriter::new();
    for &b in b"BC\xC0\xDE" {
        bad.write_bits(b as u64, 8);
    }
    bad.write_bits(1, 2);
    bad.write_vbr(8, 8);
    bad.write_vbr(4, 4);
    bad.align32();
    let bad_words_byte = bad.buf.len();
    bad.write_bits(0, 32);
    let bad_start_bit = bad.bit_pos;
    // DEFINE_ABBREV(2): num_ops=1, is_literal=0, enc=1 (Fixed), width=0x1_0000_0008
    bad.write_bits(2, 4);
    bad.write_vbr(1, 5);
    bad.write_bits(0, 1);
    bad.write_bits(1, 3);
    bad.write_vbr(0x1_0000_0008u64, 5);
    bad.write_bits(0, 4);
    bad.align32();
    let bad_words = ((bad.bit_pos - bad_start_bit) / 32) as u32;
    bad.buf[bad_words_byte..bad_words_byte + 4].copy_from_slice(&bad_words.to_le_bytes());

    let err = decode_bitcode_to_ir(&bad.buf).unwrap_err();
    assert!(
        err.to_string().contains("Fixed abbrev width"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_qmdd_scale_invariant_counterexamples_64_and_48_qubits() {
    use crate::dd::verify_equivalence_dd;
    use crate::{QuantumGate, QuantumInstruction, Qubit};

    fn append_ch(prog: &mut Vec<QuantumInstruction>, c: u32, t: u32) {
        let q = std::f64::consts::FRAC_PI_4;
        prog.push(QuantumInstruction::new(
            QuantumGate::Ry(q, Qubit(t)),
            "ry(pi/4)",
        ));
        prog.push(QuantumInstruction::new(
            QuantumGate::CX(Qubit(c), Qubit(t)),
            "cx",
        ));
        prog.push(QuantumInstruction::new(
            QuantumGate::Ry(-q, Qubit(t)),
            "ry(-pi/4)",
        ));
    }

    // 1. 64-qubit small-amplitude off-diagonal H^{\otimes 63} block (~1.6e-13 weight):
    //    must NOT collapse to Identity (previously false positive!).
    let n64 = 64usize;
    let mut fp_orig = Vec::new();
    for k in 1..n64 as u32 {
        append_ch(&mut fp_orig, 0, k);
    }
    fp_orig.push(QuantumInstruction::new(
        QuantumGate::Ry(0.001, Qubit(0)),
        "ry(0.001)",
    ));
    for k in (1..n64 as u32).rev() {
        append_ch(&mut fp_orig, 0, k);
    }
    let res_fp = verify_equivalence_dd(&fp_orig, &[], n64);
    assert!(
        !res_fp.is_equivalent,
        "64-qubit H^63 small rotation must NOT pass as identity (rel_err ~ 5e-4): {:?}",
        res_fp
    );

    // 2. 64-qubit exact cancellation through H^{\otimes 63} with sub-1e-12 off-diagonal weight:
    //    must NOT falsely diverge (previously false negative!).
    let mut fn_orig = Vec::new();
    fn_orig.push(QuantumInstruction::new(
        QuantumGate::Ry(0.006, Qubit(0)),
        "ry(0.006)",
    ));
    for k in 1..n64 as u32 {
        append_ch(&mut fn_orig, 0, k);
    }
    for k in (1..n64 as u32).rev() {
        append_ch(&mut fn_orig, 0, k);
    }
    fn_orig.push(QuantumInstruction::new(
        QuantumGate::Ry(-0.006, Qubit(0)),
        "ry(-0.006)",
    ));
    let res_fn = verify_equivalence_dd(&fn_orig, &[], n64);
    assert!(
        res_fn.is_equivalent,
        "64-qubit exact cancellation through CH layer must pass: {:?}",
        res_fn.error_message
    );
    assert!((res_fn.fidelity - 1.0).abs() < 1e-9);
    assert!(res_fn.unitarity_error < 1e-6);

    // 3. 64-qubit NodeKey / QuantComplex discrimination at ~4.5e-14 imaginary delta:
    //    U_0 (phi=0) vs U_1 (phi=0.015) controlled by q0 must NOT merge in NodeKey table.
    let theta = 2.0 * (3.0e-12 * 2.0_f64.powi(31)).atan();
    let phi = 0.015;
    let mut crz_diff = Vec::new();
    for k in 2..n64 as u32 {
        append_ch(&mut crz_diff, 1, k);
    }
    // Control on q0 = 1 branch applies Rz(phi) around Ry(theta) via CX(0, 1)
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::Rz(phi / 2.0, Qubit(1)),
        "rz",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(1)),
        "cx",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::Rz(-phi / 2.0, Qubit(1)),
        "rz",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(1)),
        "cx",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::Ry(theta, Qubit(1)),
        "ry",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::Rz(-phi / 2.0, Qubit(1)),
        "rz",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(1)),
        "cx",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::Rz(phi / 2.0, Qubit(1)),
        "rz",
    ));
    crz_diff.push(QuantumInstruction::new(
        QuantumGate::CX(Qubit(0), Qubit(1)),
        "cx",
    ));
    for k in (2..n64 as u32).rev() {
        append_ch(&mut crz_diff, 1, k);
    }
    let mut ref_no_phi = Vec::new();
    for k in 2..n64 as u32 {
        append_ch(&mut ref_no_phi, 1, k);
    }
    ref_no_phi.push(QuantumInstruction::new(
        QuantumGate::Ry(theta, Qubit(1)),
        "ry",
    ));
    for k in (2..n64 as u32).rev() {
        append_ch(&mut ref_no_phi, 1, k);
    }
    let res_coll = verify_equivalence_dd(&crz_diff, &ref_no_phi, n64);
    assert!(
        !res_coll.is_equivalent,
        "64-qubit sub-1e-13 phase rotation delta must NOT merge in NodeKey/cache: {:?}",
        res_coll
    );
}

#[test]
fn test_bitcode_typed_opaque_malformed_call_and_parser_edge_fixes() {
    use crate::bitcode::decode_bitcode_to_ir;
    use crate::unitary::UnitaryMatrix;
    use crate::{QuantumGate, Qubit};

    // 1. Typed-pointer `%Qubit = type opaque` (STRUCT_NAME + TYPE_CODE_OPAQUE) + truncated CALL must not panic,
    //    and Array(Fixed(0)) + DECLAREBLOCKS(0xFFFFFFFF) must fail or bound cleanly.
    let mut w = TestBitWriter::new();
    for &b in b"BC\xC0\xDE" {
        w.write_bits(b as u64, 8);
    }
    w.write_bits(1, 2);
    w.write_vbr(8, 8);
    w.write_vbr(4, 4);
    w.align32();
    let mod_wb = w.buf.len();
    w.write_bits(0, 32);
    let mod_start = w.bit_pos;

    // TYPE_BLOCK(17), new_aw=4
    w.write_bits(1, 4);
    w.write_vbr(17, 8);
    w.write_vbr(4, 4);
    w.align32();
    let ty_wb = w.buf.len();
    w.write_bits(0, 32);
    let ty_start = w.bit_pos;
    // STRUCT_NAME("Qubit") = code 19
    w.write_bits(3, 4);
    w.write_vbr(19, 6);
    w.write_vbr(5, 6);
    for &ch in b"Qubit" {
        w.write_vbr(ch as u64, 6);
    }
    // TYPE_CODE_OPAQUE(6)
    w.write_bits(3, 4);
    w.write_vbr(6, 6);
    w.write_vbr(0, 6);
    w.write_bits(0, 4);
    w.align32();
    let ty_words = ((w.bit_pos - ty_start) / 32) as u32;
    w.buf[ty_wb..ty_wb + 4].copy_from_slice(&ty_words.to_le_bytes());

    w.write_bits(0, 4);
    w.align32();
    let mod_words = ((w.bit_pos - mod_start) / 32) as u32;
    w.buf[mod_wb..mod_wb + 4].copy_from_slice(&mod_words.to_le_bytes());

    let ir = decode_bitcode_to_ir(&w.buf).expect("named opaque struct decodes");
    assert!(ir.contains("%Qubit = type opaque"), "got:\n{}", ir);

    // 2. Malformed truncated CALL (code=34, ops=[]) must return Err, never panic!
    let mut bad_call = TestBitWriter::new();
    for &b in b"BC\xC0\xDE" {
        bad_call.write_bits(b as u64, 8);
    }
    bad_call.write_bits(1, 2);
    bad_call.write_vbr(8, 8);
    bad_call.write_vbr(4, 4);
    bad_call.align32();
    let m_wb = bad_call.buf.len();
    bad_call.write_bits(0, 32);
    let m_start = bad_call.bit_pos;
    // TYPE_BLOCK with void(0), void()(1)
    bad_call.write_bits(1, 4);
    bad_call.write_vbr(17, 8);
    bad_call.write_vbr(4, 4);
    bad_call.align32();
    let t_wb = bad_call.buf.len();
    bad_call.write_bits(0, 32);
    let t_start = bad_call.bit_pos;
    bad_call.write_bits(3, 4);
    bad_call.write_vbr(2, 6);
    bad_call.write_vbr(0, 6);
    bad_call.write_bits(3, 4);
    bad_call.write_vbr(21, 6);
    bad_call.write_vbr(2, 6);
    bad_call.write_vbr(0, 6);
    bad_call.write_vbr(0, 6);
    bad_call.write_bits(0, 4);
    bad_call.align32();
    let t_words = ((bad_call.bit_pos - t_start) / 32) as u32;
    bad_call.buf[t_wb..t_wb + 4].copy_from_slice(&t_words.to_le_bytes());
    // MODULE_CODE_FUNCTION(8): ty=1, cc=0, is_proto=0
    bad_call.write_bits(3, 4);
    bad_call.write_vbr(8, 6);
    bad_call.write_vbr(3, 6);
    bad_call.write_vbr(1, 6);
    bad_call.write_vbr(0, 6);
    bad_call.write_vbr(0, 6);
    // FUNCTION_BLOCK(12), new_aw=4
    bad_call.write_bits(1, 4);
    bad_call.write_vbr(12, 8);
    bad_call.write_vbr(4, 4);
    bad_call.align32();
    let f_wb = bad_call.buf.len();
    bad_call.write_bits(0, 32);
    let f_start = bad_call.bit_pos;
    // CALL(34), num_ops=0 -> truncated
    bad_call.write_bits(3, 4);
    bad_call.write_vbr(34, 6);
    bad_call.write_vbr(0, 6);
    bad_call.write_bits(0, 4);
    bad_call.align32();
    let f_words = ((bad_call.bit_pos - f_start) / 32) as u32;
    bad_call.buf[f_wb..f_wb + 4].copy_from_slice(&f_words.to_le_bytes());
    bad_call.write_bits(0, 4);
    bad_call.align32();
    let m_words = ((bad_call.bit_pos - m_start) / 32) as u32;
    bad_call.buf[m_wb..m_wb + 4].copy_from_slice(&m_words.to_le_bytes());

    let call_err = decode_bitcode_to_ir(&bad_call.buf).unwrap_err();
    assert!(call_err.to_string().contains("Truncated"), "{}", call_err);

    // 3. Parser fixes: inline inttoptr call, read_result under opaque ptr,
    //    omitted entry label / multi-function barrier fences, valueless qir_profiles,
    //    targeted Barrier ASCII without cross symbol, and strict try_apply_gate.
    let edge_ir = r#"
define void @f1() #0 {
  call void @__quantum__qis__h__body(ptr null)
  br label %next
next:
  call void @__quantum__qis__h__body(ptr null)
  %b = call i1 @__quantum__qis__read_result__body(ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__barrier__body(ptr null, ptr inttoptr (i64 2 to ptr))
  ret void
}
define void @f2() {
  call void @__quantum__qis__h__body(ptr null)
  ret void
}
attributes #0 = { "qir_profiles" "required_num_qubits"="3" }
"#;
    let mut p = QirParser::new();
    let parsed = p.parse_ir(edge_ir).expect("parse edge IR");
    assert_eq!(p.metadata.qir_profiles, "");
    let read_res_gates: Vec<_> = parsed
        .iter()
        .filter(|i| matches!(&i.gate, QuantumGate::Custom { name, .. } if name == "read_result"))
        .collect();
    assert_eq!(read_res_gates.len(), 1);
    if let QuantumGate::Custom {
        qubits, results, ..
    } = &read_res_gates[0].gate
    {
        assert!(qubits.is_empty());
        assert_eq!(results, &[crate::ResultBit(2)]);
    }
    let ascii = QirEmitter::render_ascii_circuit(&parsed);
    let q1_line = ascii.lines().find(|l| l.starts_with("q[1]:")).unwrap();
    assert!(
        !q1_line.contains('┼'),
        "Barrier must not draw CNOT crossing on q[1]: {}",
        q1_line
    );

    let mut um = UnitaryMatrix::identity(1);
    assert!(um.try_apply_gate(&QuantumGate::Reset(Qubit(0))).is_err());
}

#[test]
fn test_bitcode_unnamed_fn_call_empty_gep_and_named_metadata() {
    use crate::bitcode::decode_bitcode_to_ir;

    let mut w = TestBitWriter::begin_bitcode();
    let (m_wb, m_start) = w.enter_block(2, 8, 4);
    // MODULE_CODE_VERSION(1) = 2 (relative IDs)
    w.write_unabbrev(4, 1, &[2]);

    // TYPE_BLOCK(17): 0=void, 1=ptr, 2=void(ptr), 3=i8, 4=i8()
    let (t_wb, t_start) = w.enter_block(4, 17, 4);
    w.write_unabbrev(4, 2, &[]);
    w.write_unabbrev(4, 25, &[0]);
    w.write_unabbrev(4, 21, &[0, 0, 1]);
    w.write_unabbrev(4, 7, &[8]);
    w.write_unabbrev(4, 21, &[0, 3]);
    w.end_block(4, t_wb, t_start);

    // Three unnamed functions (version=2 with empty strtab slices):
    // val_id 0 (defined), val_id 1 (decl), val_id 2 (empty non-void def)
    w.write_unabbrev(4, 8, &[0, 0, 2, 0, 0]);
    w.write_unabbrev(4, 8, &[0, 0, 2, 0, 1]);
    w.write_unabbrev(4, 8, &[0, 0, 4, 0, 0]);

    // FUNCTION_BLOCK(12) for val_id 0 (%0 is val_id 3, next_id starts at 4)
    let (f_wb, f_start) = w.enter_block(4, 12, 4);
    w.write_unabbrev(4, 1, &[1]); // DECLAREBLOCKS(1)
    w.write_unabbrev(4, 43, &[0, 3, 1]); // GEP i8, ptr %0 (0 indices) -> val_id 4
    w.write_unabbrev(4, 34, &[0, 0, 4, 1]); // CALL @fn.1(ptr %1)
    w.write_unabbrev(4, 10, &[]); // RET void
    w.end_block(4, f_wb, f_start);

    // METADATA_BLOCK(15): empty node !0 + METADATA_NAME(4) + METADATA_NAMED_NODE(10)
    let (md_wb, md_start) = w.enter_block(4, 15, 4);
    w.write_unabbrev(4, 3, &[]);
    let name_ops: Vec<u64> = b"custom metadata".iter().map(|&b| b as u64).collect();
    w.write_unabbrev(4, 4, &name_ops);
    w.write_unabbrev(4, 10, &[0]);
    w.end_block(4, md_wb, md_start);

    w.end_block(4, m_wb, m_start);

    let ir = decode_bitcode_to_ir(&w.buf).expect("synthetic module decodes");
    assert!(
        ir.contains("define void @fn.0(ptr %0)"),
        "expected @fn.0 def:\n{}",
        ir
    );
    assert!(
        ir.contains("declare void @fn.1(ptr)"),
        "expected @fn.1 decl:\n{}",
        ir
    );
    assert!(
        ir.contains("= getelementptr i8, ptr %0\n"),
        "expected GEP with no trailing comma:\n{}",
        ir
    );
    assert!(
        ir.contains("call void @fn.1(ptr %1)"),
        "expected @fn.1 call matching decl:\n{}",
        ir
    );
    assert!(
        ir.contains("define i8 @fn.2() {\nentry:\n  unreachable\n}"),
        "expected unreachable fallback in empty non-void body:\n{}",
        ir
    );
    assert!(
        ir.contains("!\"custom metadata\" = !{!0}"),
        "expected quoted named metadata:\n{}",
        ir
    );
}

#[test]
fn test_llvm20_type_and_cast_codes_and_multi_define_alloc() {
    use crate::bitcode::decode_bitcode_to_ir;

    let mut w = TestBitWriter::begin_bitcode();
    let (m_wb, m_start) = w.enter_block(2, 8, 4);
    w.write_unabbrev(4, 1, &[1]); // MODULE_CODE_VERSION = 1

    // TYPE_BLOCK(17):
    // 0=void, 1=i32, 2=half(10), 3=float(3), 4=double(4), 5=fp128(14), 6=i8(7,[8]),
    // 7=i8*(8,[6,0]), 8=void(half)(9,[0,0,0,2]), 9=void(half, double, fp128, i32, i8*)(21)
    let (t_wb, t_start) = w.enter_block(4, 17, 4);
    w.write_unabbrev(4, 2, &[]);
    w.write_unabbrev(4, 7, &[32]);
    w.write_unabbrev(4, 10, &[]);
    w.write_unabbrev(4, 3, &[]);
    w.write_unabbrev(4, 4, &[]);
    w.write_unabbrev(4, 14, &[]);
    w.write_unabbrev(4, 7, &[8]);
    w.write_unabbrev(4, 8, &[6, 0]);
    w.write_unabbrev(4, 9, &[0, 0, 0, 2]);
    w.write_unabbrev(4, 21, &[0, 0, 2, 4, 5, 1, 7]);
    w.end_block(4, t_wb, t_start);

    // Function 0: legacy FUNCTION_OLD proto (ty=8), Function 1: body (ty=9)
    w.write_unabbrev(4, 8, &[8, 0, 1]);
    w.write_unabbrev(4, 8, &[9, 0, 0]);

    // FUNCTION_BLOCK(12) for Function 1: params at values[2..7], next_id starts at 7
    let (f_wb, f_start) = w.enter_block(4, 12, 4);
    w.write_unabbrev(4, 1, &[1]); // DECLAREBLOCKS(1)
                                  // Test CastOpcodes 3..=8: fptoui(3), fptosi(4), uitofp(5), sitofp(6), fptrunc(7), fpext(8)
    w.write_unabbrev(4, 3, &[4, 1, 3]); // %5 = fptoui double %1 to i32  (val_id 7, delta 7-3=4)
    w.write_unabbrev(4, 3, &[5, 1, 4]); // %6 = fptosi double %1 to i32  (val_id 8, delta 8-3=5)
    w.write_unabbrev(4, 3, &[4, 4, 5]); // %7 = uitofp i32 %3 to double  (val_id 9, delta 9-5=4)
    w.write_unabbrev(4, 3, &[5, 4, 6]); // %8 = sitofp i32 %3 to double  (val_id 10, delta 10-5=5)
    w.write_unabbrev(4, 3, &[8, 3, 7]); // %9 = fptrunc double %1 to float (val_id 11, delta 11-3=8)
    w.write_unabbrev(4, 3, &[10, 4, 8]); // %10 = fpext half %0 to double (val_id 12, delta 12-2=10)
    w.write_unabbrev(4, 10, &[]);
    w.end_block(4, f_wb, f_start);

    w.end_block(4, m_wb, m_start);

    let ir = decode_bitcode_to_ir(&w.buf).expect("decodes LLVM types & casts");
    assert!(ir.contains("declare void @fn.0(half)"), "{}", ir);
    assert!(
        ir.contains("define void @fn.1(half %0, double %1, fp128 %2, i32 %3, i8* %4)"),
        "{}",
        ir
    );
    assert!(ir.contains("= fptoui double %1 to i32"), "{}", ir);
    assert!(ir.contains("= fptosi double %1 to i32"), "{}", ir);
    assert!(ir.contains("= uitofp i32 %3 to double"), "{}", ir);
    assert!(ir.contains("= sitofp i32 %3 to double"), "{}", ir);
    assert!(ir.contains("= fptrunc double %1 to float"), "{}", ir);
    assert!(ir.contains("= fpext half %0 to double"), "{}", ir);

    // Multi-function dynamic qubit_allocate must preserve max_alloc_q even after an empty later define.
    let multi_alloc_ir = r#"
define void @quantum_kernel() {
  %q0 = call ptr @__quantum__rt__qubit_allocate()
  %q1 = call ptr @__quantum__rt__qubit_allocate()
  %q2 = call ptr @__quantum__rt__qubit_allocate()
  ret void
}
define void @cleanup_helper() {
  ret void
}
"#;
    let mut parser = QirParser::new();
    let _ = parser.parse_ir(multi_alloc_ir).expect("parse multi define");
    assert_eq!(parser.metadata.num_qubits, 3);
}

#[test]
fn test_bitcode_wide_ints_aggregates_vectors_atomics_and_eh() {
    use crate::bitcode::decode_bitcode_to_ir;

    let mut w = TestBitWriter::begin_bitcode();
    let (m_wb, m_start) = w.enter_block(2, 8, 4);
    w.write_unabbrev(4, 1, &[1]); // MODULE_CODE_VERSION = 1

    // TYPE_BLOCK(17):
    // 0=void, 1=i128, 2=i32, 3=[2 x i32], 4=fp128, 5={ i32, i32 }, 6=<2 x i32>,
    // 7=ptr(opaque), 8=i1, 9=i32(), 10=void(ptr, <2 x i32>, { i32, i32 })
    let (t_wb, t_start) = w.enter_block(4, 17, 4);
    w.write_unabbrev(4, 2, &[]);
    w.write_unabbrev(4, 7, &[128]);
    w.write_unabbrev(4, 7, &[32]);
    w.write_unabbrev(4, 11, &[2, 2]);
    w.write_unabbrev(4, 14, &[]);
    w.write_unabbrev(4, 18, &[0, 2, 2]);
    w.write_unabbrev(4, 12, &[2, 2]);
    w.write_unabbrev(4, 25, &[0]);
    w.write_unabbrev(4, 7, &[1]);
    w.write_unabbrev(4, 21, &[0, 2]);
    w.write_unabbrev(4, 21, &[0, 0, 7, 6, 5]);
    w.end_block(4, t_wb, t_start);

    // MODULE_CODE_CONSTANTS before globals via globals referencing constants:
    // Decl 0: @personality (val_id 0), Func 1: @main (val_id 1, personality val_id 0 -> op14 = 1)
    w.write_unabbrev(4, 8, &[9, 0, 1]);
    w.write_unabbrev(4, 8, &[10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    // Module CONSTANTS_BLOCK(11): val_ids 2..=7
    let (c_wb, c_start) = w.enter_block(4, 11, 4);
    // val_id 2: i128::MAX = [3, 0xFFFFFFFFFFFFFFFE]
    w.write_unabbrev(4, 1, &[1]);
    w.write_unabbrev(4, 5, &[3, 18446744073709551614]);
    // val_id 3: i128::MIN = [0, 1]
    w.write_unabbrev(4, 5, &[0, 1]);
    // val_id 4: [2 x i32] data = [10, -30_u32]
    w.write_unabbrev(4, 1, &[3]);
    w.write_unabbrev(4, 22, &[10, 4294967266]);
    // val_id 5: fp128 1.0 = [0, 0x3FFF000000000000]
    w.write_unabbrev(4, 1, &[4]);
    w.write_unabbrev(4, 6, &[0, 0x3FFF_0000_0000_0000]);
    // val_id 6: i32 7 -> [14] (7 << 1)
    w.write_unabbrev(4, 1, &[2]);
    w.write_unabbrev(4, 4, &[14]);
    // val_id 7: { i32, i32 } aggregate [6, 6]
    w.write_unabbrev(4, 1, &[5]);
    w.write_unabbrev(4, 7, &[6, 6]);
    w.end_block(4, c_wb, c_start);

    // Globals val_ids 8..=12 (init_id is 1-based into module_values, e.g. val_id 2 -> init_id 3)
    w.write_unabbrev(4, 7, &[1, 2, 3, 0]);
    w.write_unabbrev(4, 7, &[1, 2, 4, 0]);
    w.write_unabbrev(4, 7, &[3, 3, 5, 0]);
    w.write_unabbrev(4, 7, &[4, 3, 6, 0]);
    w.write_unabbrev(4, 7, &[5, 3, 8, 0]);

    // FUNCTION_BLOCK(12) for Func 1:
    // values: [0..13) module_values, params %0=13(ptr), %1=14(<2 x i32>), %2=15({i32,i32}) -> next_id=16
    let (f_wb, f_start) = w.enter_block(4, 12, 4);
    w.write_unabbrev(4, 1, &[3]); // DECLAREBLOCKS(3)
    w.write_unabbrev(4, 26, &[1, 1]); // val 16 (%3): extractvalue %2 (delta=16-15=1), 1 -> i32
    w.write_unabbrev(4, 6, &[3, 11]); // val 17 (%4): extractelement %1 (delta=17-14=3), i32 7 (val 6 -> delta=17-6=11) -> i32
    w.write_unabbrev(4, 36, &[6]); // fence seq_cst (no value)
    w.write_unabbrev(4, 46, &[5, 12, 2, 0, 6, 0, 3, 0]); // val 18 (%5): cmpxchg ptr %0 (18-13=5), cmp i32 7 (18-6=12), new %3 (18-16=2) seq_cst acquire -> {i32, i1}
    w.write_unabbrev(4, 26, &[1, 1]); // val 19 (%6): extractvalue %5 (19-18=1), 1 -> i1
    w.write_unabbrev(4, 13, &[0, 0, 0, 1, 20]); // val 20 (%7): invoke i32 @fn.0() to bb0 unwind bb1
    w.write_unabbrev(4, 47, &[5, 1]); // val 21 (%8): landingpad { i32, i32 } cleanup
    w.write_unabbrev(4, 39, &[1]); // resume { i32, i32 } %8 (22-21=1)
    w.write_unabbrev(4, 31, &[7, 9, 1, 2]); // indirectbr ptr %0 (22-13=9), [label %bb1, label %bb2]
    w.end_block(4, f_wb, f_start);

    w.end_block(4, m_wb, m_start);

    let ir = decode_bitcode_to_ir(&w.buf).expect("advanced bitcode decodes");
    assert!(
        ir.contains("170141183460469231731687303715884105727"),
        "{}",
        ir
    );
    assert!(
        ir.contains("-170141183460469231731687303715884105728"),
        "{}",
        ir
    );
    assert!(ir.contains("[i32 10, i32 -30]"), "{}", ir);
    assert!(ir.contains("0xL00000000000000003FFF000000000000"), "{}", ir);
    assert!(ir.contains("{ i32 7, i32 7 }"), "{}", ir);
    assert!(ir.contains("personality ptr @fn.0"), "{}", ir);
    assert!(ir.contains("= extractvalue { i32, i32 } %2, 1"), "{}", ir);
    assert!(
        ir.contains("= extractelement <2 x i32> %1, i32 7"),
        "{}",
        ir
    );
    assert!(ir.contains("fence seq_cst"), "{}", ir);
    assert!(
        ir.contains("= cmpxchg ptr %0, i32 7, i32 %3 seq_cst acquire"),
        "{}",
        ir
    );
    assert!(ir.contains("= extractvalue { i32, i1 } %5, 1"), "{}", ir);
    assert!(ir.contains("= landingpad { i32, i32 } cleanup"), "{}", ir);
    assert!(ir.contains("resume { i32, i32 } %8"), "{}", ir);
    assert!(
        ir.contains("indirectbr ptr %0, [label %bb1, label %bb2]"),
        "{}",
        ir
    );
}
