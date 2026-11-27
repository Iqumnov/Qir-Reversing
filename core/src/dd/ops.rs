//! Recursive QMDD arithmetic (`add`, `mul`, `adjoint`), gate-matrix builders, and
//! cancellation-free normalized Frobenius / trace metrics.

use std::collections::HashMap;

use super::table::{snap_complex, DdTables, Edge, EdgeKey, PairEdgeKey, TERMINAL_NODE};
use crate::types::QuantumGate;
use crate::unitary::Complex64;

#[inline]
pub(crate) fn scale_edge(e: Edge, factor: Complex64) -> Edge {
    if e.is_zero() {
        return Edge::ZERO;
    }
    let w = snap_complex(e.weight * factor);
    if w == Complex64::ZERO {
        Edge::ZERO
    } else {
        Edge {
            weight: w,
            node: e.node,
        }
    }
}

/// Builds the canonical identity DD $I_{2^n}$ using exact $O(n)$ shared nodes.
pub(crate) fn identity_dd(tables: &mut DdTables, num_qubits: usize) -> Result<Edge, String> {
    let mut sub = Edge::ONE;
    for q in (0..num_qubits).rev() {
        sub = tables.make_node(q, [sub, Edge::ZERO, Edge::ZERO, sub])?;
    }
    Ok(sub)
}

/// Builds an $n$-qubit product DD $\bigotimes_{q=0}^{n-1} M_q$ where unlisted qubits act as $I_2$,
/// using at most $O(n)$ nodes.
pub(crate) fn product_dd(
    tables: &mut DdTables,
    num_qubits: usize,
    factors: &[(usize, [[Complex64; 2]; 2])],
) -> Result<Edge, String> {
    let mut sub = Edge::ONE;
    for q in (0..num_qubits).rev() {
        if let Some((_, mat)) = factors.iter().find(|(fq, _)| *fq == q) {
            sub = tables.make_node(
                q,
                [
                    scale_edge(sub, mat[0][0]),
                    scale_edge(sub, mat[0][1]),
                    scale_edge(sub, mat[1][0]),
                    scale_edge(sub, mat[1][1]),
                ],
            )?;
        } else {
            sub = tables.make_node(q, [sub, Edge::ZERO, Edge::ZERO, sub])?;
        }
    }
    Ok(sub)
}

fn conj_transpose_2x2(m: [[Complex64; 2]; 2]) -> [[Complex64; 2]; 2] {
    [
        [m[0][0].conj(), m[1][0].conj()],
        [m[0][1].conj(), m[1][1].conj()],
    ]
}

fn single_qubit_matrix(gate: &QuantumGate) -> Option<(usize, [[Complex64; 2]; 2])> {
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    match gate {
        QuantumGate::H(q) => Some((
            q.0 as usize,
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
        )),
        QuantumGate::X(q) => Some((
            q.0 as usize,
            [
                [Complex64::ZERO, Complex64::ONE],
                [Complex64::ONE, Complex64::ZERO],
            ],
        )),
        QuantumGate::Y(q) => Some((
            q.0 as usize,
            [
                [Complex64::ZERO, Complex64::new(0.0, -1.0)],
                [Complex64::new(0.0, 1.0), Complex64::ZERO],
            ],
        )),
        QuantumGate::Z(q) => Some((
            q.0 as usize,
            [
                [Complex64::ONE, Complex64::ZERO],
                [Complex64::ZERO, Complex64::new(-1.0, 0.0)],
            ],
        )),
        QuantumGate::S(q) => Some((
            q.0 as usize,
            [
                [Complex64::ONE, Complex64::ZERO],
                [Complex64::ZERO, Complex64::I],
            ],
        )),
        QuantumGate::Sdg(q) => Some((
            q.0 as usize,
            [
                [Complex64::ONE, Complex64::ZERO],
                [Complex64::ZERO, Complex64::new(0.0, -1.0)],
            ],
        )),
        QuantumGate::T(q) => {
            let theta = std::f64::consts::PI / 4.0;
            Some((
                q.0 as usize,
                [
                    [Complex64::ONE, Complex64::ZERO],
                    [Complex64::ZERO, Complex64::from_polar(1.0, theta)],
                ],
            ))
        }
        QuantumGate::Tdg(q) => {
            let theta = -std::f64::consts::PI / 4.0;
            Some((
                q.0 as usize,
                [
                    [Complex64::ONE, Complex64::ZERO],
                    [Complex64::ZERO, Complex64::from_polar(1.0, theta)],
                ],
            ))
        }
        QuantumGate::Rx(theta, q) => {
            let half = theta / 2.0;
            Some((
                q.0 as usize,
                [
                    [
                        Complex64::new(half.cos(), 0.0),
                        Complex64::new(0.0, -half.sin()),
                    ],
                    [
                        Complex64::new(0.0, -half.sin()),
                        Complex64::new(half.cos(), 0.0),
                    ],
                ],
            ))
        }
        QuantumGate::Ry(theta, q) => {
            let half = theta / 2.0;
            Some((
                q.0 as usize,
                [
                    [
                        Complex64::new(half.cos(), 0.0),
                        Complex64::new(-half.sin(), 0.0),
                    ],
                    [
                        Complex64::new(half.sin(), 0.0),
                        Complex64::new(half.cos(), 0.0),
                    ],
                ],
            ))
        }
        QuantumGate::Rz(theta, q) => {
            let half = theta / 2.0;
            Some((
                q.0 as usize,
                [
                    [Complex64::from_polar(1.0, -half), Complex64::ZERO],
                    [Complex64::ZERO, Complex64::from_polar(1.0, half)],
                ],
            ))
        }
        _ => None,
    }
}

const P0: [[Complex64; 2]; 2] = [
    [Complex64::ONE, Complex64::ZERO],
    [Complex64::ZERO, Complex64::ZERO],
];
const P1: [[Complex64; 2]; 2] = [
    [Complex64::ZERO, Complex64::ZERO],
    [Complex64::ZERO, Complex64::ONE],
];
const E01: [[Complex64; 2]; 2] = [
    [Complex64::ZERO, Complex64::ONE],
    [Complex64::ZERO, Complex64::ZERO],
];
const E10: [[Complex64; 2]; 2] = [
    [Complex64::ZERO, Complex64::ZERO],
    [Complex64::ONE, Complex64::ZERO],
];
const X_MAT: [[Complex64; 2]; 2] = [
    [Complex64::ZERO, Complex64::ONE],
    [Complex64::ONE, Complex64::ZERO],
];
const Z_MAT: [[Complex64; 2]; 2] = [
    [Complex64::ONE, Complex64::ZERO],
    [Complex64::ZERO, Complex64::new(-1.0, 0.0)],
];

/// Constructs the $2^n \times 2^n$ QMDD for a single unitary gate (or its Hermitian adjoint).
pub(crate) fn gate_dd(
    tables: &mut DdTables,
    gate: &QuantumGate,
    num_qubits: usize,
    adjoint: bool,
) -> Result<Edge, String> {
    if let Some((q, m)) = single_qubit_matrix(gate) {
        let mat = if adjoint { conj_transpose_2x2(m) } else { m };
        return product_dd(tables, num_qubits, &[(q, mat)]);
    }

    match gate {
        QuantumGate::CX(c, t) => {
            let t0 = product_dd(tables, num_qubits, &[(c.0 as usize, P0)])?;
            let t1 = product_dd(
                tables,
                num_qubits,
                &[(c.0 as usize, P1), (t.0 as usize, X_MAT)],
            )?;
            dd_add(tables, t0, t1)
        }
        QuantumGate::CZ(c, t) => {
            let t0 = product_dd(tables, num_qubits, &[(c.0 as usize, P0)])?;
            let t1 = product_dd(
                tables,
                num_qubits,
                &[(c.0 as usize, P1), (t.0 as usize, Z_MAT)],
            )?;
            dd_add(tables, t0, t1)
        }
        QuantumGate::CCX(c1, c2, t) => {
            let p0 = product_dd(tables, num_qubits, &[(c1.0 as usize, P0)])?;
            let p10 = product_dd(
                tables,
                num_qubits,
                &[(c1.0 as usize, P1), (c2.0 as usize, P0)],
            )?;
            let p11 = product_dd(
                tables,
                num_qubits,
                &[
                    (c1.0 as usize, P1),
                    (c2.0 as usize, P1),
                    (t.0 as usize, X_MAT),
                ],
            )?;
            let sum_01 = dd_add(tables, p0, p10)?;
            dd_add(tables, sum_01, p11)
        }
        QuantumGate::SWAP(a, b) => {
            let t00 = product_dd(
                tables,
                num_qubits,
                &[(a.0 as usize, P0), (b.0 as usize, P0)],
            )?;
            let t01 = product_dd(
                tables,
                num_qubits,
                &[(a.0 as usize, E01), (b.0 as usize, E10)],
            )?;
            let t10 = product_dd(
                tables,
                num_qubits,
                &[(a.0 as usize, E10), (b.0 as usize, E01)],
            )?;
            let t11 = product_dd(
                tables,
                num_qubits,
                &[(a.0 as usize, P1), (b.0 as usize, P1)],
            )?;
            let s0 = dd_add(tables, t00, t01)?;
            let s1 = dd_add(tables, t10, t11)?;
            dd_add(tables, s0, s1)
        }
        QuantumGate::Barrier(_) => {
            Err("Barriers must be excluded prior to gate DD construction".to_string())
        }
        _ => Err(format!(
            "Unsupported gate '{}' in unitary DD builder",
            gate.mnemonic()
        )),
    }
}

/// Matrix addition $C = A + B$ in QMDD form with relative-weight normalization caching.
pub(crate) fn dd_add(tables: &mut DdTables, a: Edge, b: Edge) -> Result<Edge, String> {
    if a.is_zero() {
        return Ok(b);
    }
    if b.is_zero() {
        return Ok(a);
    }
    if a.node == b.node {
        let sum_raw = a.weight + b.weight;
        let mag_sum = a.weight.abs() + b.weight.abs();
        if sum_raw.abs() <= mag_sum * 1e-13 {
            return Ok(Edge::ZERO);
        }
        let sum_w = snap_complex(sum_raw);
        return Ok(if sum_w == Complex64::ZERO {
            Edge::ZERO
        } else {
            Edge {
                weight: sum_w,
                node: a.node,
            }
        });
    }

    // Factor out the larger true Frobenius magnitude (tie-breaking by node ID for commutativity).
    let na = tables.edge_frob(a);
    let nb = tables.edge_frob(b);
    let (primary, secondary, p_norm, s_norm) = if (na, a.node) >= (nb, b.node) {
        (a, b, na, nb)
    } else {
        (b, a, nb, na)
    };
    if p_norm > 0.0 && s_norm < p_norm * 1e-13 {
        return Ok(primary);
    }
    let factor = primary.weight;
    let rel_b_weight = snap_complex(secondary.weight / factor);
    if rel_b_weight == Complex64::ZERO {
        return Ok(primary);
    }
    let rel_a = Edge {
        weight: Complex64::ONE,
        node: primary.node,
    };
    let rel_b = Edge {
        weight: rel_b_weight,
        node: secondary.node,
    };
    let key = PairEdgeKey {
        a: EdgeKey::from_edge(rel_a),
        b: EdgeKey::from_edge(rel_b),
    };
    if let Some(&cached) = tables.add_cache.get(&key) {
        return Ok(scale_edge(cached, factor));
    }

    let (qa, ea) = {
        let n = tables.get(primary.node);
        (n.qubit, n.edges)
    };
    let (qb, eb) = {
        let n = tables.get(secondary.node);
        (n.qubit, n.edges)
    };
    if qa != qb || primary.node == TERMINAL_NODE || secondary.node == TERMINAL_NODE {
        return Err(format!("QMDD addition level mismatch: q{} vs q{}", qa, qb));
    }

    let mut children = [Edge::ZERO; 4];
    for k in 0..4 {
        children[k] = dd_add(tables, ea[k], scale_edge(eb[k], rel_b_weight))?;
    }

    let res = tables.make_node(qa, children)?;
    tables.maybe_prune_caches();
    tables.add_cache.insert(key, res);
    Ok(scale_edge(res, factor))
}

/// Matrix multiplication $C = A \cdot B$ in QMDD form.
///
/// Crucially, `outer_w` is NOT pruned before recursion on internal nodes: a normalized
/// $n$-qubit superposition DD such as $H^{\otimes n}$ has root weight $2^{-n/2}$ whose product
/// falls below $10^{-12}$ for $n \ge 40$, while `a.node * b.node` yields $2^n I$.
pub(crate) fn dd_mul(tables: &mut DdTables, a: Edge, b: Edge) -> Result<Edge, String> {
    if a.is_zero() || b.is_zero() {
        return Ok(Edge::ZERO);
    }
    let outer_w = a.weight * b.weight;
    if !outer_w.re.is_finite() || !outer_w.im.is_finite() {
        return Err("Non-finite scalar weight in QMDD multiplication".to_string());
    }
    if a.node == TERMINAL_NODE && b.node == TERMINAL_NODE {
        let w = snap_complex(outer_w);
        return Ok(if w == Complex64::ZERO {
            Edge::ZERO
        } else {
            Edge {
                weight: w,
                node: TERMINAL_NODE,
            }
        });
    }
    if a.node == TERMINAL_NODE || b.node == TERMINAL_NODE {
        return Err(
            "QMDD multiplication encountered terminal/non-terminal level mismatch".to_string(),
        );
    }

    let unit_a = Edge {
        weight: Complex64::ONE,
        node: a.node,
    };
    let unit_b = Edge {
        weight: Complex64::ONE,
        node: b.node,
    };
    let key = PairEdgeKey {
        a: EdgeKey::from_edge(unit_a),
        b: EdgeKey::from_edge(unit_b),
    };
    if let Some(&cached) = tables.mul_cache.get(&key) {
        return Ok(scale_edge(cached, outer_w));
    }

    let (qa, ea) = {
        let n = tables.get(a.node);
        (n.qubit, n.edges)
    };
    let (qb, eb) = {
        let n = tables.get(b.node);
        (n.qubit, n.edges)
    };
    if qa != qb {
        return Err(format!(
            "QMDD multiplication level mismatch: q{} vs q{}",
            qa, qb
        ));
    }

    // 2x2 block matrix product: C_{ij} = \sum_{k=0}^1 A_{ik} * B_{kj}
    let mut children = [Edge::ZERO; 4];
    for r in 0..2 {
        for c in 0..2 {
            let p0 = dd_mul(tables, ea[r * 2], eb[c])?;
            let p1 = dd_mul(tables, ea[r * 2 + 1], eb[2 + c])?;
            children[r * 2 + c] = dd_add(tables, p0, p1)?;
        }
    }

    let res = tables.make_node(qa, children)?;
    tables.maybe_prune_caches();
    tables.mul_cache.insert(key, res);
    Ok(scale_edge(res, outer_w))
}

/// Computes Hermitian conjugate $M^\dagger$ of a QMDD edge in $O(|\text{DD}|)$ memoized time.
pub(crate) fn dd_dagger(tables: &mut DdTables, edge: Edge) -> Result<Edge, String> {
    fn rec(
        tables: &mut DdTables,
        node: u32,
        memo: &mut HashMap<u32, Edge>,
    ) -> Result<Edge, String> {
        if node == TERMINAL_NODE {
            return Ok(Edge::ONE);
        }
        if let Some(&cached) = memo.get(&node) {
            return Ok(cached);
        }
        let (qubit, edges) = {
            let n = tables.get(node);
            (n.qubit, n.edges)
        };
        let mut t_edges = [Edge::ZERO; 4];
        let remap = [0usize, 2, 1, 3];
        for (dst, &src) in remap.iter().enumerate() {
            let child = edges[src];
            if !child.is_zero() {
                let sub = rec(tables, child.node, memo)?;
                t_edges[dst] = scale_edge(sub, child.weight.conj());
            }
        }
        let built = tables.make_node(qubit, t_edges)?;
        memo.insert(node, built);
        Ok(built)
    }

    if edge.is_zero() {
        return Ok(Edge::ZERO);
    }
    let mut memo = HashMap::new();
    let sub = rec(tables, edge.node, &mut memo)?;
    Ok(scale_edge(sub, edge.weight.conj()))
}

/// Computes normalized matrix trace $\widehat{\tau}(D) = \operatorname{Tr}(D) / 2^n \in \mathbb{C}$,
/// bounded in $|\widehat{\tau}| \le 1$ so high-qubit traces never overflow `f64`.
pub(crate) fn normalized_trace(tables: &DdTables, edge: Edge) -> Complex64 {
    fn rec(tables: &DdTables, node: u32, memo: &mut HashMap<u32, Complex64>) -> Complex64 {
        if node == TERMINAL_NODE {
            return Complex64::ONE;
        }
        if let Some(&v) = memo.get(&node) {
            return v;
        }
        let n = tables.get(node);
        let t00 = if n.edges[0].is_zero() {
            Complex64::ZERO
        } else {
            n.edges[0].weight * rec(tables, n.edges[0].node, memo)
        };
        let t11 = if n.edges[3].is_zero() {
            Complex64::ZERO
        } else {
            n.edges[3].weight * rec(tables, n.edges[3].node, memo)
        };
        // Factor 0.5 per qubit keeps magnitude per node normalized by 2^{-(n-q)}.
        let half = Complex64::new(0.5 * (t00.re + t11.re), 0.5 * (t00.im + t11.im));
        memo.insert(node, half);
        half
    }

    if edge.is_zero() {
        Complex64::ZERO
    } else {
        let mut memo = HashMap::new();
        edge.weight * rec(tables, edge.node, &mut memo)
    }
}

/// Computes normalized squared Frobenius norm $\frac{1}{2^n} \|D\|_F^2$ without $2^n$ cancellation.
pub(crate) fn normalized_frobenius_sq(tables: &DdTables, edge: Edge) -> f64 {
    fn rec(tables: &DdTables, node: u32, memo: &mut HashMap<u32, f64>) -> f64 {
        if node == TERMINAL_NODE {
            return 1.0;
        }
        if let Some(&v) = memo.get(&node) {
            return v;
        }
        let n = tables.get(node);
        let mut sum = 0.0f64;
        for e in &n.edges {
            if !e.is_zero() {
                sum += e.weight.norm_sq() * rec(tables, e.node, memo);
            }
        }
        let avg = 0.5 * sum;
        memo.insert(node, avg);
        avg
    }

    if edge.is_zero() {
        0.0
    } else {
        let mut memo = HashMap::new();
        edge.weight.norm_sq() * rec(tables, edge.node, &mut memo)
    }
}
