//! Alternating Decision-Diagram Equivalence Checker ($G \to \mathbb{I} \leftarrow H^\dagger$)
//! modeled after MQT QCEC for scalable formal verification up to 64 qubits.

use super::ops::{
    dd_add, dd_dagger, dd_mul, gate_dd, identity_dd, normalized_frobenius_sq, normalized_trace,
    scale_edge,
};
use super::table::DdTables;
use crate::types::QuantumInstruction;
use crate::unitary::{
    non_unitary_eq, split_unitary_segments, unitarity_tolerance, validate_circuit_gates, Complex64,
    VerificationResult,
};

/// Maximum qubits supported by canonical QMDD equivalence checking.
pub const MAX_DD_QUBITS: usize = 64;

/// Computes $\sqrt{2^n}$ safely in `f64` even when $n \ge 64$.
#[inline]
fn sqrt_hilbert_dim(num_qubits: usize) -> f64 {
    2.0_f64.powf(0.5 * (num_qubits as f64))
}

/// Checks full-matrix unitarity $\|M M^\dagger - I\|_F$ of a constructed QMDD edge $M$
/// when within the node budget, otherwise falling back to the Cauchy-Schwarz normalized Frobenius lower bound.
fn verify_edge_unitarity_dd(
    tables: &mut DdTables,
    edge: super::table::Edge,
    id: super::table::Edge,
    num_qubits: usize,
) -> Result<f64, String> {
    const UNITARITY_DD_NODE_BUDGET: usize = 40_000;
    let sqrt_d = sqrt_hilbert_dim(num_qubits);
    if edge.node == id.node {
        return Ok((edge.weight.norm_sq() - 1.0).abs() * sqrt_d);
    }
    if tables.node_count() <= UNITARITY_DD_NODE_BUDGET {
        let dag = dd_dagger(tables, edge)?;
        let prod = dd_mul(tables, edge, dag)?;
        let diff = dd_add(tables, prod, scale_edge(id, Complex64::new(-1.0, 0.0)))?;
        let err_sq = normalized_frobenius_sq(tables, diff);
        Ok(err_sq.sqrt() * sqrt_d)
    } else {
        let norm_sq = normalized_frobenius_sq(tables, edge);
        Ok((1.0 - norm_sq).abs() * sqrt_d)
    }
}

/// Validates unitarity of standalone gate segments in forward order (used on structural mismatch).
fn verify_segment_gates_unitarity_dd(
    tables: &mut DdTables,
    seg: &[QuantumInstruction],
    num_qubits: usize,
) -> Result<f64, String> {
    let id = identity_dd(tables, num_qubits)?;
    let mut u = id;
    for inst in seg.iter() {
        let g = gate_dd(tables, &inst.gate, num_qubits, false)?;
        u = dd_mul(tables, g, u)?;
    }
    verify_edge_unitarity_dd(tables, u, id, num_qubits)
}

/// Verifies unitary equivalence of a single segment using alternating simulation:
/// $\mathcal{D} = (G_{k-1} \cdots G_0) (H_{m-1} \cdots H_0)^\dagger$, with identity-seeking
/// lookahead when off-identity so front/rear-loaded obfuscations cancel immediately.
fn verify_segment_alternating(
    orig: &[QuantumInstruction],
    clean: &[QuantumInstruction],
    num_qubits: usize,
    tables: &mut DdTables,
) -> Result<(f64, f64, f64), String> {
    let id = identity_dd(tables, num_qubits)?;
    let mut d = id;
    let l1 = orig.len();
    let l2 = clean.len();
    let mut i = 0usize;
    let mut j = 0usize;

    while i < l1 || j < l2 {
        if i < l1 && j < l2 {
            if d.node != id.node {
                let base_nodes = tables.node_count();
                let g = gate_dd(tables, &orig[i].gate, num_qubits, false)?;
                let d_orig = dd_mul(tables, g, d)?;
                if d_orig.node == id.node {
                    d = d_orig;
                    i += 1;
                    continue;
                }
                let orig_growth = tables.node_count() - base_nodes;

                let mid_nodes = tables.node_count();
                let h_dag = gate_dd(tables, &clean[j].gate, num_qubits, true)?;
                let d_clean = dd_mul(tables, d, h_dag)?;
                if d_clean.node == id.node {
                    d = d_clean;
                    j += 1;
                    continue;
                }
                let clean_growth = tables.node_count() - mid_nodes;

                if orig_growth < clean_growth
                    || (orig_growth == clean_growth
                        && (i as u128) * (l2 as u128) <= (j as u128) * (l1 as u128))
                {
                    d = d_orig;
                    i += 1;
                } else {
                    d = d_clean;
                    j += 1;
                }
            } else if (i as u128) * (l2 as u128) <= (j as u128) * (l1 as u128) {
                let g = gate_dd(tables, &orig[i].gate, num_qubits, false)?;
                d = dd_mul(tables, g, d)?;
                i += 1;
            } else {
                let h_dag = gate_dd(tables, &clean[j].gate, num_qubits, true)?;
                d = dd_mul(tables, d, h_dag)?;
                j += 1;
            }
        } else if i < l1 {
            let g = gate_dd(tables, &orig[i].gate, num_qubits, false)?;
            d = dd_mul(tables, g, d)?;
            i += 1;
        } else {
            let h_dag = gate_dd(tables, &clean[j].gate, num_qubits, true)?;
            d = dd_mul(tables, d, h_dag)?;
            j += 1;
        }
    }

    let norm_tr = normalized_trace(tables, d);
    let tr_abs = norm_tr.abs();
    let fidelity = tr_abs * tr_abs;

    let phase = if tr_abs > 1e-12 {
        Complex64::new(norm_tr.re / tr_abs, norm_tr.im / tr_abs)
    } else {
        Complex64::ONE
    };

    let phase_neg_id = scale_edge(id, Complex64::new(-phase.re, -phase.im));
    let diff = dd_add(tables, d, phase_neg_id)?;
    let norm_err_sq = normalized_frobenius_sq(tables, diff);
    let fro_error = norm_err_sq.sqrt() * sqrt_hilbert_dim(num_qubits);
    let unit_error = verify_edge_unitarity_dd(tables, d, id, num_qubits)?;

    Ok((fro_error, fidelity, unit_error))
}

/// Validates gates and verifies circuit equivalence via canonical QMDD alternating checking.
pub fn verify_equivalence_dd(
    orig: &[QuantumInstruction],
    clean: &[QuantumInstruction],
    num_qubits: usize,
) -> VerificationResult {
    if num_qubits > MAX_DD_QUBITS {
        return VerificationResult {
            is_equivalent: false,
            frobenius_error: f64::NAN,
            fidelity: 0.0,
            unitarity_error: f64::NAN,
            num_qubits,
            verification_method: Some("Canonical QMDD".to_string()),
            error_message: Some(format!(
                "Circuit qubit count ({}) exceeds QMDD verification limit ({} qubits)",
                num_qubits, MAX_DD_QUBITS
            )),
        };
    }

    if let Some(err_res) = validate_circuit_gates(orig, clean, num_qubits) {
        return err_res;
    }

    let max_nodes = if cfg!(target_arch = "wasm32") {
        120_000
    } else {
        300_000
    };

    let (segs_orig, non_unit_orig) = split_unitary_segments(orig);
    let (segs_clean, non_unit_clean) = split_unitary_segments(clean);
    let non_unit_match = non_unit_orig.len() == non_unit_clean.len()
        && non_unit_orig
            .iter()
            .zip(non_unit_clean.iter())
            .all(|(a, b)| non_unitary_eq(a, b));

    let sqrt_d = sqrt_hilbert_dim(num_qubits);

    if !non_unit_match || segs_orig.len() != segs_clean.len() {
        let mut max_u_err = 0.0f64;
        for seg in segs_orig.iter().chain(segs_clean.iter()) {
            if seg.is_empty() {
                continue;
            }
            let mut u_tables = DdTables::new(max_nodes);
            if let Ok(e) = verify_segment_gates_unitarity_dd(&mut u_tables, seg, num_qubits) {
                if !e.is_finite() {
                    max_u_err = f64::NAN;
                    break;
                }
                max_u_err = max_u_err.max(e);
            }
        }
        return VerificationResult {
            is_equivalent: false,
            frobenius_error: 2.0 * sqrt_d,
            fidelity: 0.0,
            unitarity_error: max_u_err,
            num_qubits,
            verification_method: Some("Canonical QMDD".to_string()),
            error_message: None,
        };
    }

    let mut max_error = 0.0f64;
    let mut min_fidelity = 1.0f64;
    let mut max_unitarity_error = 0.0f64;

    for (seg_o, seg_c) in segs_orig.iter().zip(segs_clean.iter()) {
        if seg_o.is_empty() && seg_c.is_empty() {
            continue;
        }
        let mut seg_tables = DdTables::new(max_nodes);
        match verify_segment_alternating(seg_o, seg_c, num_qubits, &mut seg_tables) {
            Ok((err, fid, u_err)) if err.is_finite() && fid.is_finite() && u_err.is_finite() => {
                max_error = max_error.max(err);
                min_fidelity = min_fidelity.min(fid);
                max_unitarity_error = max_unitarity_error.max(u_err);
            }
            Ok(_) => {
                return VerificationResult {
                    is_equivalent: false,
                    frobenius_error: f64::NAN,
                    fidelity: 0.0,
                    unitarity_error: f64::NAN,
                    num_qubits,
                    verification_method: Some("Canonical QMDD".to_string()),
                    error_message: Some(
                        "Non-finite floating-point QMDD metric encountered during verification"
                            .to_string(),
                    ),
                };
            }
            Err(msg) => {
                return VerificationResult {
                    is_equivalent: false,
                    frobenius_error: f64::NAN,
                    fidelity: 0.0,
                    unitarity_error: f64::NAN,
                    num_qubits,
                    verification_method: Some("Canonical QMDD".to_string()),
                    error_message: Some(msg),
                };
            }
        }
    }

    let rel_err = max_error / sqrt_d;
    let is_equivalent = rel_err < 1e-6
        && (1.0 - min_fidelity).abs() < 1e-6
        && max_unitarity_error < unitarity_tolerance(num_qubits);

    VerificationResult {
        is_equivalent,
        frobenius_error: max_error,
        fidelity: min_fidelity,
        unitarity_error: max_unitarity_error,
        num_qubits,
        verification_method: Some("Canonical QMDD".to_string()),
        error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::ops::product_dd;
    use super::*;

    #[test]
    fn test_qmdd_unitarity_budget_fallback_64q_no_ulp_blowup() {
        let mut tables = DdTables::new(60_000);
        let id = identity_dd(&mut tables, 64).expect("id");
        // Populate unique table past UNITARITY_DD_NODE_BUDGET (40_000) with distinct 64q Rz(theta) edges
        let sub_tail = tables.get(id.node).edges[0];
        let mut last_rz = id;
        for k in 1..=40_050u32 {
            let theta = (k as f64) * 1e-4;
            let p0 = Complex64::from_polar(1.0, -0.5 * theta);
            let p1 = Complex64::from_polar(1.0, 0.5 * theta);
            last_rz = tables
                .make_node(
                    0,
                    [
                        scale_edge(sub_tail, p0),
                        super::super::table::Edge::ZERO,
                        super::super::table::Edge::ZERO,
                        scale_edge(sub_tail, p1),
                    ],
                )
                .expect("rz node");
        }
        assert!(tables.node_count() > 40_000);

        // 1-ULP perturbed identity at 64 qubits must stay tiny (~9.5e-7), never 64.0
        let perturbed_id = scale_edge(id, Complex64::new(1.0 - 2.220446049250313e-16, 0.0));
        let u_id_err = verify_edge_unitarity_dd(&mut tables, perturbed_id, id, 64).expect("id err");
        assert!(
            u_id_err < 1e-5,
            "64q 1-ULP identity error unexpectedly large: {}",
            u_id_err
        );

        // Over-budget non-identity 64q unitary edge must pass tolerance cleanly without sqrt(ulp) blowup
        let u_rz_err = verify_edge_unitarity_dd(&mut tables, last_rz, id, 64).expect("rz err");
        assert!(
            u_rz_err < unitarity_tolerance(64),
            "64q over-budget unitary error {} exceeds tolerance {}",
            u_rz_err,
            unitarity_tolerance(64)
        );

        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let h_edge = product_dd(
            &mut tables,
            64,
            &[(
                0,
                [
                    [
                        Complex64::new(inv_sqrt2, 0.0),
                        Complex64::new(inv_sqrt2, 0.0),
                    ],
                    [
                        Complex64::new(inv_sqrt2, 0.0),
                        Complex64::new(-inv_sqrt2, 0.0),
                    ],
                ],
            )],
        )
        .expect("h64");
        let u_h_err = verify_edge_unitarity_dd(&mut tables, h_edge, id, 64).expect("h err");
        assert!(u_h_err < unitarity_tolerance(64), "{}", u_h_err);
    }
}
