//! Canonical QMDD unique table, edge normalization, and memoized compute caches.

use std::collections::HashMap;
use std::hash::Hash;

use crate::unitary::Complex64;

/// Terminal node ID representing scalar `[1]` (all non-zero paths terminate here at depth `num_qubits`).
pub(crate) const TERMINAL_NODE: u32 = 0;

/// Underflow guard for scaled weights (never drops legitimate $2^{-n/2}$ QMDD edges).
const EDGE_UNDERFLOW_SQ: f64 = 1e-300;

/// Relative component snap threshold for orthogonal floating-point dust (`re` vs `im`).
const REL_COMP_SNAP: f64 = 1e-13;

/// Relative child pruning threshold inside `make_node` based on true child Frobenius norm.
const REL_NODE_CHILD_ZERO: f64 = 1e-13;

/// Maximum entries in `add`/`mul` compute caches before clearing to bound memory.
const MAX_CACHE_ENTRIES: usize = 512_000;

/// Weighted directed edge in the Quantum Multiple-Valued Decision Diagram.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub weight: Complex64,
    pub node: u32,
}

impl Edge {
    pub const ZERO: Self = Self {
        weight: Complex64::ZERO,
        node: TERMINAL_NODE,
    };

    pub const ONE: Self = Self {
        weight: Complex64::ONE,
        node: TERMINAL_NODE,
    };

    #[inline]
    pub fn is_zero(self) -> bool {
        (self.weight.re == 0.0 && self.weight.im == 0.0) || self.weight.norm_sq() == 0.0
    }
}

/// Internal QMDD node splitting a $2^{n-q} \times 2^{n-q}$ matrix into 4 quadrants:
/// `[00, 01, 10, 11]`.
#[derive(Clone, Debug)]
pub struct DdNode {
    pub qubit: usize,
    pub edges: [Edge; 4],
    pub frob_norm: f64,
}

#[inline]
pub(crate) fn snap_complex(c: Complex64) -> Complex64 {
    let n_sq = c.norm_sq();
    if !n_sq.is_finite() || n_sq <= EDGE_UNDERFLOW_SQ {
        return Complex64::ZERO;
    }
    let comp_eps = n_sq.sqrt() * REL_COMP_SNAP;
    let re = if c.re.abs() < comp_eps { 0.0 } else { c.re };
    let im = if c.im.abs() < comp_eps { 0.0 } else { c.im };
    Complex64::new(re, im)
}

#[inline]
fn quantize_f64(v: f64) -> u64 {
    if !v.is_finite() {
        return u64::MAX;
    }
    if v == 0.0 || v.abs() < 1e-150 {
        return 0;
    }
    // Scale-invariant IEEE-754 mantissa bucketing (~3.6e-12 relative bin width)
    // so both O(1) and O(2^{-32}) amplitudes retain full angular discrimination.
    const DROP_BITS: u64 = 14;
    const ROUND_HALF: u64 = 1u64 << (DROP_BITS - 1);
    let bits = v.to_bits();
    let exp = (bits >> 52) & 0x7FF;
    if exp >= 0x7FE {
        return bits;
    }
    bits.saturating_add(ROUND_HALF) & !((1u64 << DROP_BITS) - 1)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct QuantComplex {
    re: u64,
    im: u64,
}

impl QuantComplex {
    #[inline]
    pub(crate) fn from_complex(c: Complex64) -> Self {
        let s = snap_complex(c);
        Self {
            re: quantize_f64(s.re),
            im: quantize_f64(s.im),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EdgeKey {
    weight: QuantComplex,
    node: u32,
}

impl EdgeKey {
    #[inline]
    pub(crate) fn from_edge(e: Edge) -> Self {
        if e.is_zero() {
            Self {
                weight: QuantComplex { re: 0, im: 0 },
                node: TERMINAL_NODE,
            }
        } else {
            Self {
                weight: QuantComplex::from_complex(e.weight),
                node: e.node,
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NodeKey {
    qubit: usize,
    edges: [EdgeKey; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PairEdgeKey {
    pub(crate) a: EdgeKey,
    pub(crate) b: EdgeKey,
}

/// Manages canonical QMDD node storage, unique lookup, and operation caches.
pub(crate) struct DdTables {
    nodes: Vec<DdNode>,
    unique: HashMap<NodeKey, u32>,
    pub(crate) add_cache: HashMap<PairEdgeKey, Edge>,
    pub(crate) mul_cache: HashMap<PairEdgeKey, Edge>,
    pub(crate) max_nodes: usize,
}

impl DdTables {
    pub(crate) fn new(max_nodes: usize) -> Self {
        let mut nodes = Vec::with_capacity(1024);
        // Slot 0 reserved for terminal node scalar [1].
        nodes.push(DdNode {
            qubit: usize::MAX,
            edges: [Edge::ZERO; 4],
            frob_norm: 1.0,
        });
        Self {
            nodes,
            unique: HashMap::new(),
            add_cache: HashMap::new(),
            mul_cache: HashMap::new(),
            max_nodes,
        }
    }

    #[inline]
    pub(crate) fn edge_frob(&self, e: Edge) -> f64 {
        if e.is_zero() {
            0.0
        } else {
            e.weight.abs() * self.nodes[e.node as usize].frob_norm
        }
    }

    #[inline]
    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub(crate) fn get(&self, id: u32) -> &DdNode {
        &self.nodes[id as usize]
    }

    /// Canonicalizes `[Edge; 4]` using scale-invariant Frobenius weighting and interns node.
    pub(crate) fn make_node(&mut self, qubit: usize, raw: [Edge; 4]) -> Result<Edge, String> {
        let mut edges = [Edge::ZERO; 4];
        let mut child_frobs = [0.0f64; 4];
        let mut max_frob = 0.0f64;

        for (i, e) in raw.iter().enumerate() {
            if !e.weight.re.is_finite() || !e.weight.im.is_finite() {
                return Err(
                    "Non-finite floating-point weight encountered during QMDD construction"
                        .to_string(),
                );
            }
            let snapped = snap_complex(e.weight);
            if snapped != Complex64::ZERO {
                let cf = snapped.abs() * self.nodes[e.node as usize].frob_norm;
                if cf.is_finite() && cf > 0.0 {
                    edges[i] = Edge {
                        weight: snapped,
                        node: e.node,
                    };
                    child_frobs[i] = cf;
                    max_frob = max_frob.max(cf);
                }
            }
        }

        if max_frob == 0.0 {
            return Ok(Edge::ZERO);
        }

        let cutoff = max_frob * REL_NODE_CHILD_ZERO;
        for i in 0..4 {
            if child_frobs[i] < cutoff {
                edges[i] = Edge::ZERO;
                child_frobs[i] = 0.0;
            }
        }

        // Hybrid canonical pivot: pick the first child within 1e-7 relative Frobenius norm of max_frob.
        let threshold = max_frob * (1.0 - 1e-7);
        let pivot_idx = (0..4)
            .find(|&i| !edges[i].is_zero() && child_frobs[i] >= threshold)
            .or_else(|| (0..4).find(|&i| !edges[i].is_zero()))
            .ok_or_else(|| "QMDD normalization found no non-zero pivot edge".to_string())?;
        let norm = edges[pivot_idx].weight;

        for (i, e) in edges.iter_mut().enumerate() {
            if !e.is_zero() {
                if i == pivot_idx {
                    e.weight = Complex64::ONE;
                } else {
                    let w = snap_complex(e.weight / norm);
                    if w == Complex64::ZERO {
                        *e = Edge::ZERO;
                    } else {
                        e.weight = w;
                    }
                }
            }
        }

        let mut norm_sq_sum = 0.0f64;
        for e in &edges {
            if !e.is_zero() {
                let child_fn = self.nodes[e.node as usize].frob_norm;
                norm_sq_sum += e.weight.norm_sq() * (child_fn * child_fn);
            }
        }
        let frob_norm = norm_sq_sum.sqrt();
        if !frob_norm.is_finite() {
            return Err("QMDD node Frobenius norm overflow".to_string());
        }

        let key = NodeKey {
            qubit,
            edges: [
                EdgeKey::from_edge(edges[0]),
                EdgeKey::from_edge(edges[1]),
                EdgeKey::from_edge(edges[2]),
                EdgeKey::from_edge(edges[3]),
            ],
        };

        let node_id = if let Some(&existing) = self.unique.get(&key) {
            existing
        } else {
            if self.nodes.len() >= self.max_nodes {
                return Err(format!(
                    "QMDD node table reached memory budget ({} nodes) for circuit verification",
                    self.max_nodes
                ));
            }
            let id = u32::try_from(self.nodes.len())
                .map_err(|_| "QMDD node table exceeded u32 index space".to_string())?;
            self.nodes.push(DdNode {
                qubit,
                edges,
                frob_norm,
            });
            self.unique.insert(key, id);
            id
        };

        Ok(Edge {
            weight: norm,
            node: node_id,
        })
    }

    pub(crate) fn maybe_prune_caches(&mut self) {
        if self.add_cache.len() > MAX_CACHE_ENTRIES {
            self.add_cache.clear();
        }
        if self.mul_cache.len() > MAX_CACHE_ENTRIES {
            self.mul_cache.clear();
        }
    }
}
