//! Pure-Rust Quantum Multiple-Valued Decision Diagram (QMDD) equivalence engine.
//!
//! Enables scalable unitary equivalence verification (`U_orig ≡ e^{iθ} U_clean`) beyond dense
//! statevector/matrix limits using canonical node sharing and alternating $G \to \mathbb{I} \leftarrow H^\dagger$
//! simulation up to [`MAX_DD_QUBITS`] (64 qubits).

mod ops;
mod table;
mod verifier;

pub use verifier::{verify_equivalence_dd, MAX_DD_QUBITS};
