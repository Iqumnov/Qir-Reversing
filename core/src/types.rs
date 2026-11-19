//! Core Quantum Intermediate Representation (QIR) Data Types
//! Author: Matvei Igumnov <iqumnov@proton.me>

use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly-typed hardware or virtual qubit index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Qubit(pub u32);

impl fmt::Display for Qubit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "q[{}]", self.0)
    }
}

/// Strongly-typed classical measurement result register index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResultBit(pub u32);

impl fmt::Display for ResultBit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r[{}]", self.0)
    }
}

/// Quantum gate operations supported by QIR Base Profile and Adaptive Profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuantumGate {
    H(Qubit),
    X(Qubit),
    Y(Qubit),
    Z(Qubit),
    S(Qubit),
    Sdg(Qubit),
    T(Qubit),
    Tdg(Qubit),
    Rx(f64, Qubit),
    Ry(f64, Qubit),
    Rz(f64, Qubit),
    CX(Qubit, Qubit),         // ctrl, tgt
    CZ(Qubit, Qubit),         // ctrl, tgt
    SWAP(Qubit, Qubit),       // q1, q2
    CCX(Qubit, Qubit, Qubit), // c1, c2, tgt
    Mz(Qubit, ResultBit),     // q -> r
    Reset(Qubit),
    Barrier(Vec<Qubit>),
    Custom {
        name: String,
        qubits: Vec<Qubit>,
        results: Vec<ResultBit>,
        params: Vec<f64>,
    },
}

impl QuantumGate {
    /// Returns the primary qubits manipulated by this gate.
    pub fn active_qubits(&self) -> Vec<Qubit> {
        match self {
            QuantumGate::H(q)
            | QuantumGate::X(q)
            | QuantumGate::Y(q)
            | QuantumGate::Z(q)
            | QuantumGate::S(q)
            | QuantumGate::Sdg(q)
            | QuantumGate::T(q)
            | QuantumGate::Tdg(q)
            | QuantumGate::Rx(_, q)
            | QuantumGate::Ry(_, q)
            | QuantumGate::Rz(_, q)
            | QuantumGate::Mz(q, _)
            | QuantumGate::Reset(q) => vec![*q],
            QuantumGate::CX(q1, q2) | QuantumGate::CZ(q1, q2) | QuantumGate::SWAP(q1, q2) => {
                vec![*q1, *q2]
            }
            QuantumGate::CCX(q1, q2, q3) => vec![*q1, *q2, *q3],
            QuantumGate::Barrier(qs) => qs.clone(),
            QuantumGate::Custom { qubits, .. } => qubits.clone(),
        }
    }

    /// Check if this gate is self-inverse (involutory: G * G = I).
    pub fn is_self_inverse(&self) -> bool {
        matches!(
            self,
            QuantumGate::H(_)
                | QuantumGate::X(_)
                | QuantumGate::Y(_)
                | QuantumGate::Z(_)
                | QuantumGate::CX(_, _)
                | QuantumGate::CZ(_, _)
                | QuantumGate::SWAP(_, _)
                | QuantumGate::CCX(_, _, _)
        )
    }

    /// Returns standard quantum assembly mnemonic.
    pub fn mnemonic(&self) -> String {
        match self {
            QuantumGate::H(q) => format!("h {}", q),
            QuantumGate::X(q) => format!("x {}", q),
            QuantumGate::Y(q) => format!("y {}", q),
            QuantumGate::Z(q) => format!("z {}", q),
            QuantumGate::S(q) => format!("s {}", q),
            QuantumGate::Sdg(q) => format!("sdg {}", q),
            QuantumGate::T(q) => format!("t {}", q),
            QuantumGate::Tdg(q) => format!("tdg {}", q),
            QuantumGate::Rx(theta, q) => format!("rx({:.6}) {}", theta, q),
            QuantumGate::Ry(theta, q) => format!("ry({:.6}) {}", theta, q),
            QuantumGate::Rz(theta, q) => format!("rz({:.6}) {}", theta, q),
            QuantumGate::CX(c, t) => format!("cx {}, {}", c, t),
            QuantumGate::CZ(c, t) => format!("cz {}, {}", c, t),
            QuantumGate::SWAP(q1, q2) => format!("swap {}, {}", q1, q2),
            QuantumGate::CCX(c1, c2, t) => format!("ccx {}, {}, {}", c1, c2, t),
            QuantumGate::Mz(q, r) => format!("mz {} -> {}", q, r),
            QuantumGate::Reset(q) => format!("reset {}", q),
            QuantumGate::Barrier(qs) => {
                let qs_str = qs
                    .iter()
                    .map(|q| q.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("barrier {}", qs_str)
            }
            QuantumGate::Custom { name, qubits, .. } => {
                let qs_str = qubits
                    .iter()
                    .map(|q| q.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {}", name, qs_str)
            }
        }
    }

    /// Synthetic instruction opcode bytes for binary emulation.
    pub fn synthetic_opcode(&self) -> [u8; 4] {
        let (b0, b1, b2, b3) = match self {
            QuantumGate::H(q) => (0x01, q.0 as u8, 0, 0),
            QuantumGate::X(q) => (0x02, q.0 as u8, 0, 0),
            QuantumGate::Y(q) => (0x03, q.0 as u8, 0, 0),
            QuantumGate::Z(q) => (0x04, q.0 as u8, 0, 0),
            QuantumGate::S(q) => (0x05, q.0 as u8, 0, 0),
            QuantumGate::Sdg(q) => (0x06, q.0 as u8, 0, 0),
            QuantumGate::T(q) => (0x07, q.0 as u8, 0, 0),
            QuantumGate::Tdg(q) => (0x08, q.0 as u8, 0, 0),
            QuantumGate::CX(c, t) => (0x10, c.0 as u8, t.0 as u8, 0),
            QuantumGate::CZ(c, t) => (0x11, c.0 as u8, t.0 as u8, 0),
            QuantumGate::SWAP(q1, q2) => (0x12, q1.0 as u8, q2.0 as u8, 0),
            QuantumGate::CCX(c1, c2, t) => (0x13, c1.0 as u8, c2.0 as u8, t.0 as u8),
            QuantumGate::Rx(theta, q) => {
                let deg = ((theta.abs() * 180.0 / std::f64::consts::PI).round() as u32 % 360) as u8;
                (0x20, q.0 as u8, deg, 0)
            }
            QuantumGate::Ry(theta, q) => {
                let deg = ((theta.abs() * 180.0 / std::f64::consts::PI).round() as u32 % 360) as u8;
                (0x21, q.0 as u8, deg, 0)
            }
            QuantumGate::Rz(theta, q) => {
                let deg = ((theta.abs() * 180.0 / std::f64::consts::PI).round() as u32 % 360) as u8;
                (0x22, q.0 as u8, deg, 0)
            }
            QuantumGate::Mz(q, r) => (0x30, q.0 as u8, r.0 as u8, 0),
            QuantumGate::Reset(q) => (0x3f, q.0 as u8, 0, 0),
            _ => (0xff, 0, 0, 0),
        };
        [b0, b1, b2, b3]
    }
}

/// An annotated quantum instruction within an execution stream or basic block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantumInstruction {
    pub gate: QuantumGate,
    pub address: u64,
    pub raw_ir: String,
    pub is_control_flow: bool,
    pub label: Option<String>,
}

impl QuantumInstruction {
    pub fn new(gate: QuantumGate, raw_ir: impl Into<String>) -> Self {
        Self {
            gate,
            address: 0,
            raw_ir: raw_ir.into(),
            is_control_flow: false,
            label: None,
        }
    }

    pub fn with_address(mut self, address: u64) -> Self {
        self.address = address;
        self
    }
}
