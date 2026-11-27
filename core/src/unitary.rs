//! Formal Quantum State & Unitary Matrix Verification Engine
//! Author: Matvei Igumnov <matvei@iqumnov.com>

use crate::types::{QuantumGate, QuantumInstruction};
use serde::{Deserialize, Serialize};

/// Complex number representation for unitary gate simulation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Complex64 {
    pub re: f64,
    pub im: f64,
}

impl Complex64 {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };
    pub const I: Self = Self { re: 0.0, im: 1.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn from_polar(r: f64, theta: f64) -> Self {
        Self {
            re: r * theta.cos(),
            im: r * theta.sin(),
        }
    }

    pub fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

impl std::ops::Add for Complex64 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl std::ops::Sub for Complex64 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl std::ops::Mul for Complex64 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Div for Complex64 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        // Smith's numerically stable complex division avoiding intermediate squaring overflow.
        if rhs.re.abs() >= rhs.im.abs() {
            let r = rhs.im / rhs.re;
            let denom = rhs.re + rhs.im * r;
            Self {
                re: (self.re + self.im * r) / denom,
                im: (self.im - self.re * r) / denom,
            }
        } else {
            let r = rhs.re / rhs.im;
            let denom = rhs.im + rhs.re * r;
            Self {
                re: (self.re * r + self.im) / denom,
                im: (self.im * r - self.re) / denom,
            }
        }
    }
}

/// Maximum qubit count supported by dense $2^N \times 2^N$ unitary simulation.
pub const MAX_VERIFIABLE_QUBITS: usize = 12;

/// N-qubit Unitary Matrix representation in 2^N dimensional Hilbert space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitaryMatrix {
    pub num_qubits: usize,
    pub dim: usize,
    pub data: Vec<Complex64>, // row-major dim x dim
}

impl UnitaryMatrix {
    /// Initialize 2^N x 2^N Identity Matrix
    pub fn identity(num_qubits: usize) -> Self {
        let dim = 1usize
            .checked_shl(num_qubits as u32)
            .filter(|_| num_qubits <= MAX_VERIFIABLE_QUBITS)
            .expect("UnitaryMatrix::identity qubit count exceeds MAX_VERIFIABLE_QUBITS");
        let mut data = vec![Complex64::ZERO; dim * dim];
        for i in 0..dim {
            data[i * dim + i] = Complex64::ONE;
        }
        Self {
            num_qubits,
            dim,
            data,
        }
    }

    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        self.data[row * self.dim + col]
    }

    pub fn set(&mut self, row: usize, col: usize, val: Complex64) {
        self.data[row * self.dim + col] = val;
    }

    /// Multiply two unitary matrices: C = A * B
    pub fn multiply(&self, rhs: &Self) -> Self {
        assert_eq!(self.dim, rhs.dim);
        let dim = self.dim;
        let mut res = vec![Complex64::ZERO; dim * dim];

        for i in 0..dim {
            for k in 0..dim {
                let aik = self.data[i * dim + k];
                if aik.abs() < 1e-12 {
                    continue;
                }
                for j in 0..dim {
                    res[i * dim + j] = res[i * dim + j] + aik * rhs.data[k * dim + j];
                }
            }
        }

        Self {
            num_qubits: self.num_qubits,
            dim,
            data: res,
        }
    }

    /// Conjugate transpose (Hermitian adjoint: U†)
    pub fn adjoint(&self) -> Self {
        let dim = self.dim;
        let mut res = vec![Complex64::ZERO; dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                res[j * dim + i] = self.data[i * dim + j].conj();
            }
        }
        Self {
            num_qubits: self.num_qubits,
            dim,
            data: res,
        }
    }

    /// Frobenius distance between two unitaries: ||U - V||_F
    pub fn frobenius_distance(&self, other: &Self) -> f64 {
        assert_eq!(self.dim, other.dim);
        let mut sum_sq = 0.0;
        for i in 0..self.data.len() {
            let diff = self.data[i] - other.data[i];
            sum_sq += diff.norm_sq();
        }
        sum_sq.sqrt()
    }

    /// Matrix trace: sum of diagonal elements Tr(U)
    pub fn trace(&self) -> Complex64 {
        let mut tr = Complex64::ZERO;
        for i in 0..self.dim {
            tr = tr + self.get(i, i);
        }
        tr
    }

    /// Quantum Process Fidelity between two unitaries:
    /// F(U, V) = |Tr(U† · V)|^2 / d^2, invariant to global phase shifts e^(iθ)
    pub fn process_fidelity(&self, other: &Self) -> f64 {
        assert_eq!(self.dim, other.dim);
        let mut inner_prod = Complex64::ZERO;
        for i in 0..self.data.len() {
            inner_prod = inner_prod + self.data[i].conj() * other.data[i];
        }
        let d = self.dim as f64;
        inner_prod.norm_sq() / (d * d)
    }

    /// Apply a single quantum gate to the unitary representation.
    pub fn apply_gate(&mut self, gate: &QuantumGate) {
        let n = self.num_qubits;
        if n == 0 {
            return;
        }
        for q in gate.active_qubits() {
            if q.0 as usize >= n {
                return;
            }
        }
        let dim = self.dim;

        match gate {
            QuantumGate::H(q) => {
                let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
                self.apply_single_qubit_2x2(
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
                );
            }
            QuantumGate::X(q) => {
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ZERO, Complex64::ONE],
                        [Complex64::ONE, Complex64::ZERO],
                    ],
                );
            }
            QuantumGate::Y(q) => {
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ZERO, Complex64::new(0.0, -1.0)],
                        [Complex64::new(0.0, 1.0), Complex64::ZERO],
                    ],
                );
            }
            QuantumGate::Z(q) => {
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ONE, Complex64::ZERO],
                        [Complex64::ZERO, Complex64::new(-1.0, 0.0)],
                    ],
                );
            }
            QuantumGate::S(q) => {
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ONE, Complex64::ZERO],
                        [Complex64::ZERO, Complex64::I],
                    ],
                );
            }
            QuantumGate::Sdg(q) => {
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ONE, Complex64::ZERO],
                        [Complex64::ZERO, Complex64::new(0.0, -1.0)],
                    ],
                );
            }
            QuantumGate::T(q) => {
                let theta = std::f64::consts::PI / 4.0;
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ONE, Complex64::ZERO],
                        [Complex64::ZERO, Complex64::from_polar(1.0, theta)],
                    ],
                );
            }
            QuantumGate::Tdg(q) => {
                let theta = -std::f64::consts::PI / 4.0;
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::ONE, Complex64::ZERO],
                        [Complex64::ZERO, Complex64::from_polar(1.0, theta)],
                    ],
                );
            }
            QuantumGate::Rx(theta, q) => {
                let half_theta = theta / 2.0;
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [
                            Complex64::new(half_theta.cos(), 0.0),
                            Complex64::new(0.0, -half_theta.sin()),
                        ],
                        [
                            Complex64::new(0.0, -half_theta.sin()),
                            Complex64::new(half_theta.cos(), 0.0),
                        ],
                    ],
                );
            }
            QuantumGate::Ry(theta, q) => {
                let half_theta = theta / 2.0;
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [
                            Complex64::new(half_theta.cos(), 0.0),
                            Complex64::new(-half_theta.sin(), 0.0),
                        ],
                        [
                            Complex64::new(half_theta.sin(), 0.0),
                            Complex64::new(half_theta.cos(), 0.0),
                        ],
                    ],
                );
            }
            QuantumGate::Rz(theta, q) => {
                let half_theta = theta / 2.0;
                self.apply_single_qubit_2x2(
                    q.0 as usize,
                    [
                        [Complex64::from_polar(1.0, -half_theta), Complex64::ZERO],
                        [Complex64::ZERO, Complex64::from_polar(1.0, half_theta)],
                    ],
                );
            }
            QuantumGate::CX(c, t) => {
                let c_bit = 1 << (n - 1 - c.0 as usize);
                let t_bit = 1 << (n - 1 - t.0 as usize);

                for i in 0..dim {
                    // When control bit is 1, swap basis states where target is 0 vs 1
                    if (i & c_bit) != 0 && (i & t_bit) == 0 {
                        let j = i | t_bit;
                        for col in 0..dim {
                            let tmp = self.get(i, col);
                            self.set(i, col, self.get(j, col));
                            self.set(j, col, tmp);
                        }
                    }
                }
            }
            QuantumGate::CZ(c, t) => {
                let c_bit = 1 << (n - 1 - c.0 as usize);
                let t_bit = 1 << (n - 1 - t.0 as usize);

                for i in 0..dim {
                    if (i & c_bit) != 0 && (i & t_bit) != 0 {
                        for col in 0..dim {
                            let val = self.get(i, col);
                            self.set(i, col, Complex64::new(-val.re, -val.im));
                        }
                    }
                }
            }
            QuantumGate::SWAP(q1, q2) => {
                let b1 = 1 << (n - 1 - q1.0 as usize);
                let b2 = 1 << (n - 1 - q2.0 as usize);

                for i in 0..dim {
                    if (i & b1) != 0 && (i & b2) == 0 {
                        let j = (i & !b1) | b2;
                        for col in 0..dim {
                            let tmp = self.get(i, col);
                            self.set(i, col, self.get(j, col));
                            self.set(j, col, tmp);
                        }
                    }
                }
            }
            QuantumGate::CCX(c1, c2, t) => {
                let c1_bit = 1 << (n - 1 - c1.0 as usize);
                let c2_bit = 1 << (n - 1 - c2.0 as usize);
                let t_bit = 1 << (n - 1 - t.0 as usize);

                for i in 0..dim {
                    if (i & c1_bit) != 0 && (i & c2_bit) != 0 && (i & t_bit) == 0 {
                        let j = i | t_bit;
                        for col in 0..dim {
                            let tmp = self.get(i, col);
                            self.set(i, col, self.get(j, col));
                            self.set(j, col, tmp);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Strict version of [`apply_gate`](Self::apply_gate) that rejects non-unitary or
    /// custom/unsupported operations instead of silently treating them as Identity $I$.
    pub fn try_apply_gate(&mut self, gate: &QuantumGate) -> Result<(), String> {
        match gate {
            QuantumGate::H(_)
            | QuantumGate::X(_)
            | QuantumGate::Y(_)
            | QuantumGate::Z(_)
            | QuantumGate::S(_)
            | QuantumGate::Sdg(_)
            | QuantumGate::T(_)
            | QuantumGate::Tdg(_)
            | QuantumGate::Rx(_, _)
            | QuantumGate::Ry(_, _)
            | QuantumGate::Rz(_, _)
            | QuantumGate::CX(_, _)
            | QuantumGate::CZ(_, _)
            | QuantumGate::SWAP(_, _)
            | QuantumGate::CCX(_, _, _) => {
                self.apply_gate(gate);
                Ok(())
            }
            QuantumGate::Barrier(_) => Ok(()),
            other => Err(format!(
                "Operation '{}' cannot be represented as a single unitary matrix without segment isolation",
                other.mnemonic()
            )),
        }
    }

    fn apply_single_qubit_2x2(&mut self, target_qubit: usize, m: [[Complex64; 2]; 2]) {
        let n = self.num_qubits;
        let dim = self.dim;
        let bit = 1 << (n - 1 - target_qubit);

        for i in 0..dim {
            if (i & bit) == 0 {
                let j = i | bit;
                for col in 0..dim {
                    let v0 = self.get(i, col);
                    let v1 = self.get(j, col);

                    let new_v0 = m[0][0] * v0 + m[0][1] * v1;
                    let new_v1 = m[1][0] * v0 + m[1][1] * v1;

                    self.set(i, col, new_v0);
                    self.set(j, col, new_v1);
                }
            }
        }
    }
}

/// Canonical scale-aware unitarity drift bound (`1e-9` for `n <= 12`, smooth $\sqrt{2}$ scaling above 12).
pub fn unitarity_tolerance(num_qubits: usize) -> f64 {
    1e-9 * 2.0_f64.powf(0.5 * num_qubits.saturating_sub(12) as f64)
}

/// Shared non-unitary segment mismatch result across dense and QMDD verifiers.
pub(crate) fn segment_mismatch_result(
    num_qubits: usize,
    max_unitarity_error: f64,
) -> VerificationResult {
    let sqrt_d = 2.0_f64.powf(0.5 * num_qubits as f64);
    let method = if num_qubits > dense_qubit_limit() {
        "Canonical QMDD"
    } else {
        "Dense 2^N"
    };
    VerificationResult {
        is_equivalent: false,
        frobenius_error: 2.0 * sqrt_d,
        fidelity: 0.0,
        unitarity_error: max_unitarity_error,
        num_qubits,
        verification_method: Some(method.to_string()),
        error_message: None,
    }
}

#[inline]
pub(crate) fn dense_qubit_limit() -> usize {
    let safe_max_qubits = if cfg!(target_arch = "wasm32") { 10 } else { 12 };
    MAX_VERIFIABLE_QUBITS.min(safe_max_qubits)
}

#[derive(Debug, Clone)]
pub(crate) enum SegmentBoundary {
    Mz(u32, u32),
    Reset(u32),
    ControlFlow(String),
    Custom {
        name: String,
        qubits: Vec<u32>,
        results: Vec<u32>,
        params: Vec<f64>,
    },
}

pub(crate) fn non_unitary_eq(a: &SegmentBoundary, b: &SegmentBoundary) -> bool {
    match (a, b) {
        (SegmentBoundary::Mz(qa, ra), SegmentBoundary::Mz(qb, rb)) => qa == qb && ra == rb,
        (SegmentBoundary::Reset(qa), SegmentBoundary::Reset(qb)) => qa == qb,
        (SegmentBoundary::ControlFlow(ra), SegmentBoundary::ControlFlow(rb)) => {
            ra.trim() == rb.trim()
        }
        (
            SegmentBoundary::Custom {
                name: na,
                qubits: qa,
                results: ra,
                params: pa,
            },
            SegmentBoundary::Custom {
                name: nb,
                qubits: qb,
                results: rb,
                params: pb,
            },
        ) => {
            na.trim() == nb.trim()
                && qa == qb
                && ra == rb
                && pa.len() == pb.len()
                && pa
                    .iter()
                    .zip(pb.iter())
                    .all(|(x, y)| x.is_finite() && y.is_finite() && (x - y).abs() < 1e-9)
        }
        _ => false,
    }
}

pub(crate) fn split_unitary_segments(
    instructions: &[QuantumInstruction],
) -> (Vec<Vec<QuantumInstruction>>, Vec<SegmentBoundary>) {
    let mut segments = Vec::new();
    let mut non_unitaries = Vec::new();
    let mut current = Vec::new();

    for inst in instructions {
        if inst.is_control_flow {
            segments.push(std::mem::take(&mut current));
            non_unitaries.push(SegmentBoundary::ControlFlow(inst.raw_ir.clone()));
            continue;
        }
        match &inst.gate {
            QuantumGate::Mz(q, r) => {
                segments.push(std::mem::take(&mut current));
                non_unitaries.push(SegmentBoundary::Mz(q.0, r.0));
            }
            QuantumGate::Reset(q) => {
                segments.push(std::mem::take(&mut current));
                non_unitaries.push(SegmentBoundary::Reset(q.0));
            }
            QuantumGate::Custom {
                name,
                qubits,
                results,
                params,
            } => {
                segments.push(std::mem::take(&mut current));
                non_unitaries.push(SegmentBoundary::Custom {
                    name: name.clone(),
                    qubits: qubits.iter().map(|q| q.0).collect(),
                    results: results.iter().map(|r| r.0).collect(),
                    params: params.clone(),
                });
            }
            QuantumGate::Barrier(_) => {
                // Compiler scheduling barrier does not alter unitary semantics.
            }
            _ => {
                current.push(inst.clone());
            }
        }
    }
    segments.push(current);
    (segments, non_unitaries)
}

fn compute_unitarity_error(u: &UnitaryMatrix) -> f64 {
    let dim = u.dim;
    let mut sq_err = 0.0;
    let mut row_dot = vec![Complex64::ZERO; dim];
    for i in 0..dim {
        row_dot.fill(Complex64::ZERO);
        let row_i = &u.data[i * dim..(i + 1) * dim];
        for k in 0..dim {
            let uik = row_i[k];
            if uik.norm_sq() < 1e-24 {
                continue;
            }
            for j in 0..dim {
                row_dot[j] = row_dot[j] + (uik * u.data[j * dim + k].conj());
            }
        }
        for (j, &sum) in row_dot.iter().enumerate() {
            let diff = if i == j { sum - Complex64::ONE } else { sum };
            sq_err += diff.norm_sq();
        }
    }
    sq_err.sqrt()
}

fn validate_gate_finite(gate: &QuantumGate) -> bool {
    match gate {
        QuantumGate::Rx(t, _) | QuantumGate::Ry(t, _) | QuantumGate::Rz(t, _) => t.is_finite(),
        QuantumGate::Custom { params, .. } => params.iter().all(|p| p.is_finite()),
        _ => true,
    }
}

fn has_duplicate_qubits(gate: &QuantumGate) -> bool {
    match gate {
        QuantumGate::CX(a, b) | QuantumGate::CZ(a, b) | QuantumGate::SWAP(a, b) => a == b,
        QuantumGate::CCX(a, b, c) => a == b || a == c || b == c,
        _ => false,
    }
}

pub(crate) fn validate_circuit_gates(
    orig: &[QuantumInstruction],
    clean: &[QuantumInstruction],
    num_qubits: usize,
) -> Option<VerificationResult> {
    for inst in orig.iter().chain(clean.iter()) {
        if inst.is_control_flow {
            continue;
        }
        if !validate_gate_finite(&inst.gate) {
            return Some(VerificationResult {
                is_equivalent: false,
                frobenius_error: f64::NAN,
                fidelity: 0.0,
                unitarity_error: f64::NAN,
                num_qubits,
                verification_method: None,
                error_message: Some(format!(
                    "Instruction '{}' contains non-finite (NaN/Inf) rotation parameter",
                    inst.gate.mnemonic()
                )),
            });
        }
        if has_duplicate_qubits(&inst.gate) {
            return Some(VerificationResult {
                is_equivalent: false,
                frobenius_error: f64::NAN,
                fidelity: 0.0,
                unitarity_error: f64::NAN,
                num_qubits,
                verification_method: None,
                error_message: Some(format!(
                    "Instruction '{}' requires distinct operand qubits",
                    inst.gate.mnemonic()
                )),
            });
        }
        for q in inst.gate.active_qubits() {
            if (q.0 as usize) >= num_qubits {
                return Some(VerificationResult {
                    is_equivalent: false,
                    frobenius_error: f64::NAN,
                    fidelity: 0.0,
                    unitarity_error: f64::NAN,
                    num_qubits,
                    verification_method: None,
                    error_message: Some(format!(
                        "Instruction '{}' references out-of-bounds qubit q[{}] for {}-qubit system",
                        inst.gate.mnemonic(),
                        q.0,
                        num_qubits
                    )),
                });
            }
        }
    }
    None
}

/// Formal quantum circuit equivalence verifier (dense $2^N \times 2^N$ and canonical QMDD).
pub struct UnitaryVerifier;

impl UnitaryVerifier {
    pub fn compute_circuit_unitary(
        instructions: &[QuantumInstruction],
        num_qubits: usize,
    ) -> UnitaryMatrix {
        let mut u = UnitaryMatrix::identity(num_qubits);
        for inst in instructions {
            if !inst.is_control_flow {
                u.apply_gate(&inst.gate);
            }
        }
        u
    }

    /// Mathematically proves unitary equivalence between original and clean circuits.
    ///
    /// Uses exact dense $2^N \times 2^N$ unitary simulation up to 12 qubits (10 in WASM),
    /// and automatically transitions to canonical QMDD alternating verification (`G -> I <- H†`)
    /// up to 64 qubits.
    pub fn verify_equivalence(
        orig: &[QuantumInstruction],
        clean: &[QuantumInstruction],
        num_qubits: usize,
    ) -> VerificationResult {
        if let Some(err_res) = validate_circuit_gates(orig, clean, num_qubits) {
            return err_res;
        }

        if num_qubits > dense_qubit_limit() {
            return crate::dd::verify_equivalence_dd(orig, clean, num_qubits);
        }

        let (segs_orig, non_unit_orig) = split_unitary_segments(orig);
        let (segs_clean, non_unit_clean) = split_unitary_segments(clean);

        let non_unit_match = non_unit_orig.len() == non_unit_clean.len()
            && non_unit_orig
                .iter()
                .zip(non_unit_clean.iter())
                .all(|(a, b)| non_unitary_eq(a, b));

        let dim = 1usize << num_qubits;
        let mut max_unitarity_error: f64 = 0.0;

        if !non_unit_match || segs_orig.len() != segs_clean.len() {
            for seg_c in &segs_clean {
                if seg_c.is_empty() {
                    continue;
                }
                let u_clean = Self::compute_circuit_unitary(seg_c, num_qubits);
                let seg_u_err = compute_unitarity_error(&u_clean);
                if !seg_u_err.is_finite() {
                    max_unitarity_error = f64::NAN;
                    break;
                }
                max_unitarity_error = max_unitarity_error.max(seg_u_err);
            }
            return segment_mismatch_result(num_qubits, max_unitarity_error);
        }

        let mut max_error: f64 = 0.0;
        let mut min_fidelity: f64 = 1.0;

        for (seg_o, seg_c) in segs_orig.iter().zip(segs_clean.iter()) {
            if seg_o.is_empty() && seg_c.is_empty() {
                continue;
            }
            let u_orig = Self::compute_circuit_unitary(seg_o, num_qubits);
            let u_clean = Self::compute_circuit_unitary(seg_c, num_qubits);

            let seg_unitarity_err =
                compute_unitarity_error(&u_orig).max(compute_unitarity_error(&u_clean));

            // Global phase alignment: minimize ||U_orig - e^{i theta} U_clean||_F
            let mut tr = Complex64::ZERO;
            for (a, b) in u_orig.data.iter().zip(u_clean.data.iter()) {
                tr = tr + (a.conj() * *b);
            }
            let tr_abs = tr.abs();
            let phase = if tr_abs > 1e-12 {
                Complex64::new(tr.re / tr_abs, -tr.im / tr_abs)
            } else {
                Complex64::ONE
            };

            let mut sq_err = 0.0;
            for (a, b) in u_orig.data.iter().zip(u_clean.data.iter()) {
                sq_err += (*a - (phase * *b)).norm_sq();
            }
            let error = sq_err.sqrt();
            let fidelity = u_orig.process_fidelity(&u_clean);

            if !error.is_finite() || !fidelity.is_finite() || !seg_unitarity_err.is_finite() {
                return VerificationResult {
                    is_equivalent: false,
                    frobenius_error: f64::NAN,
                    fidelity: 0.0,
                    unitarity_error: f64::NAN,
                    num_qubits,
                    verification_method: Some("Dense 2^N".to_string()),
                    error_message: Some(
                        "Non-finite floating-point matrix metric encountered during verification"
                            .to_string(),
                    ),
                };
            }

            max_unitarity_error = max_unitarity_error.max(seg_unitarity_err);
            max_error = max_error.max(error);
            min_fidelity = min_fidelity.min(fidelity);
        }

        let is_equivalent = max_error < 1e-6 * (dim as f64).sqrt()
            && (1.0 - min_fidelity).abs() < 1e-6
            && max_unitarity_error < unitarity_tolerance(num_qubits);

        VerificationResult {
            is_equivalent,
            frobenius_error: max_error,
            fidelity: min_fidelity,
            unitarity_error: max_unitarity_error,
            num_qubits,
            verification_method: Some("Dense 2^N".to_string()),
            error_message: None,
        }
    }
}

fn deserialize_nan_tolerant_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer).map(|opt| opt.unwrap_or(f64::NAN))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub is_equivalent: bool,
    #[serde(deserialize_with = "deserialize_nan_tolerant_f64")]
    pub frobenius_error: f64,
    #[serde(deserialize_with = "deserialize_nan_tolerant_f64")]
    pub fidelity: f64,
    #[serde(deserialize_with = "deserialize_nan_tolerant_f64")]
    pub unitarity_error: f64,
    pub num_qubits: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
