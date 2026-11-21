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

    pub fn new(re: f64, im: f64) -> Self {
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
        self.norm_sq().sqrt()
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
        let dim = 1 << num_qubits;
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

/// Maximum number of qubits for full 2^N x 2^N classical unitary matrix representation.
pub const MAX_VERIFIABLE_QUBITS: usize = 14;

/// Computes circuit unitary and mathematically verifies equivalence.
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
    pub fn verify_equivalence(
        orig: &[QuantumInstruction],
        clean: &[QuantumInstruction],
        num_qubits: usize,
    ) -> VerificationResult {
        if num_qubits > MAX_VERIFIABLE_QUBITS {
            return VerificationResult {
                is_equivalent: false,
                frobenius_error: f64::NAN,
                fidelity: 0.0,
                unitarity_error: f64::NAN,
                num_qubits,
                error_message: Some(format!(
                    "Observable Hilbert space dimension 2^{} exceeds classical matrix memory limits (max supported: {} qubits / {}x{} states)",
                    num_qubits,
                    MAX_VERIFIABLE_QUBITS,
                    1 << MAX_VERIFIABLE_QUBITS,
                    1 << MAX_VERIFIABLE_QUBITS
                )),
            };
        }

        let u_orig = Self::compute_circuit_unitary(orig, num_qubits);
        let u_clean = Self::compute_circuit_unitary(clean, num_qubits);

        let error = u_orig.frobenius_distance(&u_clean);
        let fidelity = u_orig.process_fidelity(&u_clean);
        let is_equivalent = error < 1e-6 || (1.0 - fidelity).abs() < 1e-6;

        // Check reversibility: U * U† = I
        let u_adj = u_clean.adjoint();
        let u_id = u_clean.multiply(&u_adj);
        let id = UnitaryMatrix::identity(num_qubits);
        let unitarity_error = u_id.frobenius_distance(&id);

        VerificationResult {
            is_equivalent,
            frobenius_error: error,
            fidelity,
            unitarity_error,
            num_qubits,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub is_equivalent: bool,
    pub frobenius_error: f64,
    pub fidelity: f64,
    pub unitarity_error: f64,
    pub num_qubits: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
