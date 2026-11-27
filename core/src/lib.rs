//! QIR Reversing Core
//! Memory-safe LLVM QIR parsing, quantum disassembly, and deobfuscation platform.
//! Author: Matvei Igumnov <matvei@iqumnov.com>

#![deny(unsafe_code)]

pub mod bitcode;
pub mod dd;
pub mod deobfuscator;
pub mod emitter;
pub mod parser;
pub mod types;
pub mod unitary;
pub mod wasm_bridge;

#[cfg(test)]
pub mod tests;

pub use deobfuscator::{ObfuscationFinding, ObfuscationSeverity, QirDeobfuscator};
pub use emitter::QirEmitter;
pub use parser::{ParserError, QirModuleMetadata, QirParser};
pub use types::{QuantumGate, QuantumInstruction, Qubit, ResultBit};
pub use unitary::{Complex64, UnitaryMatrix, UnitaryVerifier, VerificationResult};
