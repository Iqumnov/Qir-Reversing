//! Multi-Pass Quantum Deobfuscation & Anti-Obfuscation Audit Engine
//! Author: Matvei Igumnov <matvei@iqumnov.com>

use crate::types::{QuantumGate, QuantumInstruction, Qubit};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Quantum obfuscation pattern category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObfuscationSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for ObfuscationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
        }
    }
}

/// Recorded anomaly site found during bytecode deobfuscation audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObfuscationFinding {
    pub id: usize,
    pub pattern_type: String,
    pub severity: ObfuscationSeverity,
    pub description: String,
    pub qubits: Vec<u32>,
    pub instruction_indices: Vec<usize>,
}

/// Quantum deobfuscation pipeline and report generator.
pub struct QirDeobfuscator {
    pub findings: Vec<ObfuscationFinding>,
    pub obfuscation_ratio: f64,
}

impl Default for QirDeobfuscator {
    fn default() -> Self {
        Self::new()
    }
}

impl QirDeobfuscator {
    pub fn new() -> Self {
        Self {
            findings: Vec::new(),
            obfuscation_ratio: 0.0,
        }
    }

    /// Execute multi-pass deobfuscation, returning the canonical cleaned instruction stream.
    pub fn deobfuscate(&mut self, instructions: &[QuantumInstruction]) -> Vec<QuantumInstruction> {
        self.findings.clear();

        let orig_count = instructions.iter().filter(|i| !i.is_control_flow).count();
        if orig_count == 0 {
            self.obfuscation_ratio = 0.0;
            return instructions.to_vec();
        }

        // Pass 1: Prune Dead Qubits
        let (pass1_indexed, dead_qubits) = self.prune_dead_qubits_indexed(instructions);
        let mut sorted_dead: Vec<Qubit> = dead_qubits.iter().copied().collect();
        sorted_dead.sort();
        for dq in sorted_dead {
            let id = self.findings.len() + 1;
            let mut dead_indices: Vec<usize> = instructions
                .iter()
                .enumerate()
                .filter_map(|(idx, inst)| {
                    if !matches!(inst.gate, QuantumGate::Barrier(_))
                        && inst.gate.active_qubits().contains(&dq)
                    {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if dead_indices.is_empty() {
                dead_indices = instructions
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, inst)| {
                        if matches!(inst.gate, QuantumGate::Barrier(_))
                            && inst.gate.active_qubits().contains(&dq)
                        {
                            Some(idx)
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            self.findings.push(ObfuscationFinding {
                id,
                pattern_type: "DEAD_QUBIT_INJECTION".to_string(),
                severity: ObfuscationSeverity::High,
                description: format!(
                    "Dead unmeasured qubit telemetry injection detected: q[{}] has no causal link to measurements",
                    dq.0
                ),
                qubits: vec![dq.0],
                instruction_indices: dead_indices,
            });
        }

        // Pass 2: Lookahead Peephole Optimization (Involutions & Continuous Rotations)
        let pass2_insts = self.pass_peephole_lookahead(pass1_indexed);

        let clean_count = pass2_insts.iter().filter(|i| !i.is_control_flow).count();
        self.obfuscation_ratio = if orig_count > 0 {
            (orig_count.saturating_sub(clean_count) as f64 / orig_count as f64) * 100.0
        } else {
            0.0
        };

        pass2_insts
    }

    /// Identify qubits that are never measured and do not entangle with measured qubits.
    pub fn prune_dead_qubits(
        &self,
        instructions: &[QuantumInstruction],
    ) -> (Vec<QuantumInstruction>, HashSet<Qubit>) {
        let (indexed, dead) = self.prune_dead_qubits_indexed(instructions);
        (indexed.into_iter().map(|(_, inst)| inst).collect(), dead)
    }

    fn prune_dead_qubits_indexed(
        &self,
        instructions: &[QuantumInstruction],
    ) -> (Vec<(usize, QuantumInstruction)>, HashSet<Qubit>) {
        let mut measured_qubits = HashSet::new();
        let mut all_qubits = HashSet::new();

        for inst in instructions {
            for q in inst.gate.active_qubits() {
                all_qubits.insert(q);
            }
            match &inst.gate {
                QuantumGate::Mz(q, _) => {
                    measured_qubits.insert(*q);
                }
                QuantumGate::Custom {
                    qubits, results, ..
                } if !results.is_empty() => {
                    for &q in qubits {
                        measured_qubits.insert(q);
                    }
                }
                _ => {}
            }
        }

        // If the circuit contains no explicit measurements (e.g. pure unitary transformation or oracle),
        // all qubits are treated as live observable wires.
        if measured_qubits.is_empty() {
            return (
                instructions.iter().cloned().enumerate().collect(),
                HashSet::new(),
            );
        }

        // Propagate causal entanglement backwards from measurements
        let mut live_qubits = measured_qubits.clone();
        let mut changed = true;

        while changed {
            changed = false;
            for inst in instructions.iter().rev() {
                match inst.gate {
                    QuantumGate::CX(c, t) | QuantumGate::CZ(c, t) | QuantumGate::SWAP(c, t) => {
                        if live_qubits.contains(&c) && !live_qubits.contains(&t) {
                            live_qubits.insert(t);
                            changed = true;
                        } else if live_qubits.contains(&t) && !live_qubits.contains(&c) {
                            live_qubits.insert(c);
                            changed = true;
                        }
                    }
                    QuantumGate::CCX(c1, c2, t) => {
                        if live_qubits.contains(&c1)
                            || live_qubits.contains(&c2)
                            || live_qubits.contains(&t)
                        {
                            for q in [c1, c2, t] {
                                if live_qubits.insert(q) {
                                    changed = true;
                                }
                            }
                        }
                    }
                    QuantumGate::Custom { ref qubits, .. }
                        if qubits.len() >= 2 && qubits.iter().any(|q| live_qubits.contains(q)) =>
                    {
                        for &q in qubits {
                            if live_qubits.insert(q) {
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let dead_qubits: HashSet<Qubit> = all_qubits.difference(&live_qubits).copied().collect();

        if dead_qubits.is_empty() {
            return (
                instructions.iter().cloned().enumerate().collect(),
                dead_qubits,
            );
        }

        let filtered: Vec<(usize, QuantumInstruction)> = instructions
            .iter()
            .enumerate()
            .filter_map(|(idx, inst)| match &inst.gate {
                QuantumGate::Barrier(qs) => {
                    if qs.is_empty() {
                        Some((idx, inst.clone()))
                    } else {
                        let live_qs: Vec<Qubit> = qs
                            .iter()
                            .copied()
                            .filter(|q| !dead_qubits.contains(q))
                            .collect();
                        if live_qs.is_empty() {
                            None
                        } else {
                            let mut cloned = inst.clone();
                            cloned.gate = QuantumGate::Barrier(live_qs);
                            Some((idx, cloned))
                        }
                    }
                }
                _ => {
                    let qs = inst.gate.active_qubits();
                    if qs.iter().any(|q| dead_qubits.contains(q)) {
                        None
                    } else {
                        Some((idx, inst.clone()))
                    }
                }
            })
            .collect();

        (filtered, dead_qubits)
    }

    /// Lookahead peephole optimizer for self-inverse pairs and rotation cancellations.
    fn pass_peephole_lookahead(
        &mut self,
        mut stream: Vec<(usize, QuantumInstruction)>,
    ) -> Vec<QuantumInstruction> {
        let mut modified = true;

        while modified {
            modified = false;
            let mut i = 0;

            while i < stream.len() {
                if stream[i].1.is_control_flow
                    || matches!(
                        stream[i].1.gate,
                        QuantumGate::Mz(_, _) | QuantumGate::Reset(_) | QuantumGate::Barrier(_)
                    )
                {
                    i += 1;
                    continue;
                }

                // Eliminate standalone trivial identity rotations: R*(0) or R*(2πk) ≡ I
                if let Some(theta) = rotation_angle(&stream[i].1.gate) {
                    if is_identity_rotation_angle(theta) {
                        let id = self.findings.len() + 1;
                        self.findings.push(ObfuscationFinding {
                            id,
                            pattern_type: "TRIVIAL_IDENTITY_ROTATION".to_string(),
                            severity: ObfuscationSeverity::High,
                            description: format!(
                                "Trivial identity rotation (angle ≡ 0 mod 2π) removed: {} = I",
                                stream[i].1.gate.mnemonic()
                            ),
                            qubits: stream[i]
                                .1
                                .gate
                                .active_qubits()
                                .iter()
                                .map(|q| q.0)
                                .collect(),
                            instruction_indices: vec![stream[i].0],
                        });
                        stream.remove(i);
                        modified = true;
                        break;
                    }
                }

                let mut j = i + 1;

                while j < stream.len() {
                    if stream[j].1.is_control_flow {
                        break;
                    }

                    let orig_i = stream[i].0;
                    let orig_j = stream[j].0;

                    // Check for self-inverse cancellation (H*H = I, X*X = I, CX*CX = I)
                    if self.can_cancel_involutions(&stream[i].1.gate, &stream[j].1.gate) {
                        let id = self.findings.len() + 1;
                        self.findings.push(ObfuscationFinding {
                            id,
                            pattern_type: "IDENTITY_INVOLUTION_PAIR".to_string(),
                            severity: ObfuscationSeverity::High,
                            description: format!(
                                "Redundant self-inverse involutory pair: {} · {} = I",
                                stream[i].1.gate.mnemonic(),
                                stream[j].1.gate.mnemonic()
                            ),
                            qubits: stream[i]
                                .1
                                .gate
                                .active_qubits()
                                .iter()
                                .map(|q| q.0)
                                .collect(),
                            instruction_indices: vec![orig_i, orig_j],
                        });

                        stream.remove(j);
                        stream.remove(i);
                        modified = true;
                        break;
                    }

                    // Check for discrete phase group consolidation (Z_8: T, S, Z, S†, T†)
                    if let Some(replacement) =
                        self.try_consolidate_discrete_phase(&stream[i].1.gate, &stream[j].1.gate)
                    {
                        let id = self.findings.len() + 1;
                        if let Some(clean_gate) = replacement {
                            self.findings.push(ObfuscationFinding {
                                id,
                                pattern_type: "DISCRETE_PHASE_CONSOLIDATION".to_string(),
                                severity: ObfuscationSeverity::Medium,
                                description: format!(
                                    "Consolidated discrete Clifford+T phase gates: {} · {} -> {}",
                                    stream[i].1.gate.mnemonic(),
                                    stream[j].1.gate.mnemonic(),
                                    clean_gate.mnemonic()
                                ),
                                qubits: stream[i]
                                    .1
                                    .gate
                                    .active_qubits()
                                    .iter()
                                    .map(|q| q.0)
                                    .collect(),
                                instruction_indices: vec![orig_i, orig_j],
                            });
                            stream[i].1.raw_ir = clean_gate.mnemonic();
                            stream[i].1.gate = clean_gate;
                            stream.remove(j);
                        } else {
                            self.findings.push(ObfuscationFinding {
                                id,
                                pattern_type: "DISCRETE_PHASE_CANCELLATION".to_string(),
                                severity: ObfuscationSeverity::High,
                                description: format!(
                                    "Inverse discrete phase gates cancelled out: {} · {} = I",
                                    stream[i].1.gate.mnemonic(),
                                    stream[j].1.gate.mnemonic()
                                ),
                                qubits: stream[i]
                                    .1
                                    .gate
                                    .active_qubits()
                                    .iter()
                                    .map(|q| q.0)
                                    .collect(),
                                instruction_indices: vec![stream[i].0, stream[j].0],
                            });
                            stream.remove(j);
                            stream.remove(i);
                        }
                        modified = true;
                        break;
                    }

                    // Check for continuous rotation cancellation: Rz(theta) + Rz(-theta) = I
                    if let Some(replacement) =
                        self.try_consolidate_rotations(&stream[i].1.gate, &stream[j].1.gate)
                    {
                        let id = self.findings.len() + 1;
                        if let Some(clean_gate) = replacement {
                            self.findings.push(ObfuscationFinding {
                                id,
                                pattern_type: "CONTINUOUS_ROTATION_CONSOLIDATION".to_string(),
                                severity: ObfuscationSeverity::Medium,
                                description: format!(
                                    "Consolidated continuous rotations: {} + {} -> {}",
                                    stream[i].1.gate.mnemonic(),
                                    stream[j].1.gate.mnemonic(),
                                    clean_gate.mnemonic()
                                ),
                                qubits: stream[i]
                                    .1
                                    .gate
                                    .active_qubits()
                                    .iter()
                                    .map(|q| q.0)
                                    .collect(),
                                instruction_indices: vec![stream[i].0, stream[j].0],
                            });
                            stream[i].1.raw_ir = clean_gate.mnemonic();
                            stream[i].1.gate = clean_gate;
                            stream.remove(j);
                        } else {
                            self.findings.push(ObfuscationFinding {
                                id,
                                pattern_type: "INVERSE_ROTATION_CANCELLATION".to_string(),
                                severity: ObfuscationSeverity::High,
                                description: format!(
                                    "Opposite continuous rotations cancelled out: {} · {} = I",
                                    stream[i].1.gate.mnemonic(),
                                    stream[j].1.gate.mnemonic()
                                ),
                                qubits: stream[i]
                                    .1
                                    .gate
                                    .active_qubits()
                                    .iter()
                                    .map(|q| q.0)
                                    .collect(),
                                instruction_indices: vec![orig_i, orig_j],
                            });
                            stream.remove(j);
                            stream.remove(i);
                        }
                        modified = true;
                        break;
                    }

                    // Check if gate at j interferes with gate at i
                    if self.gates_interfere(&stream[i].1.gate, &stream[j].1.gate) {
                        break;
                    }

                    j += 1;
                }

                if modified {
                    break;
                }
                i += 1;
            }
        }

        stream.into_iter().map(|(_, inst)| inst).collect()
    }

    fn can_cancel_involutions(&self, g1: &QuantumGate, g2: &QuantumGate) -> bool {
        match (g1, g2) {
            (QuantumGate::H(q1), QuantumGate::H(q2)) if q1 == q2 => true,
            (QuantumGate::X(q1), QuantumGate::X(q2)) if q1 == q2 => true,
            (QuantumGate::Y(q1), QuantumGate::Y(q2)) if q1 == q2 => true,
            (QuantumGate::Z(q1), QuantumGate::Z(q2)) if q1 == q2 => true,
            (QuantumGate::CX(c1, t1), QuantumGate::CX(c2, t2)) if c1 == c2 && t1 == t2 => true,
            (QuantumGate::CZ(c1, t1), QuantumGate::CZ(c2, t2))
                if (c1 == c2 && t1 == t2) || (c1 == t2 && t1 == c2) =>
            {
                true
            }
            (QuantumGate::SWAP(a1, b1), QuantumGate::SWAP(a2, b2))
                if (a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2) =>
            {
                true
            }
            (QuantumGate::CCX(c1_a, c2_a, t_a), QuantumGate::CCX(c1_b, c2_b, t_b))
                if t_a == t_b
                    && ((c1_a == c1_b && c2_a == c2_b) || (c1_a == c2_b && c2_a == c1_b)) =>
            {
                true
            }
            _ => false,
        }
    }

    fn try_consolidate_discrete_phase(
        &self,
        g1: &QuantumGate,
        g2: &QuantumGate,
    ) -> Option<Option<QuantumGate>> {
        let to_z8 = |g: &QuantumGate| -> Option<(u8, Qubit)> {
            match g {
                QuantumGate::T(q) => Some((1, *q)),
                QuantumGate::S(q) => Some((2, *q)),
                QuantumGate::Z(q) => Some((4, *q)),
                QuantumGate::Sdg(q) => Some((6, *q)),
                QuantumGate::Tdg(q) => Some((7, *q)),
                _ => None,
            }
        };

        if let (Some((p1, q1)), Some((p2, q2))) = (to_z8(g1), to_z8(g2)) {
            if q1 == q2 {
                let sum = (p1 + p2) % 8;
                return match sum {
                    0 => Some(None),                       // Cancels to Identity
                    1 => Some(Some(QuantumGate::T(q1))),   // e.g. T† · S or S · T†
                    2 => Some(Some(QuantumGate::S(q1))),   // e.g. T · T or S† · Z
                    4 => Some(Some(QuantumGate::Z(q1))),   // e.g. S · S or S† · S†
                    6 => Some(Some(QuantumGate::Sdg(q1))), // e.g. T† · T† or S · Z
                    7 => Some(Some(QuantumGate::Tdg(q1))), // e.g. T · S† or S† · T
                    _ => None,                             // Preserve exact Clifford basis
                };
            }
        }
        None
    }

    fn try_consolidate_rotations(
        &self,
        g1: &QuantumGate,
        g2: &QuantumGate,
    ) -> Option<Option<QuantumGate>> {
        match (g1, g2) {
            (QuantumGate::Rz(t1, q1), QuantumGate::Rz(t2, q2)) if q1 == q2 => {
                consolidate_angle_pair(*t1, *t2).map(|opt| opt.map(|a| QuantumGate::Rz(a, *q1)))
            }
            (QuantumGate::Rx(t1, q1), QuantumGate::Rx(t2, q2)) if q1 == q2 => {
                consolidate_angle_pair(*t1, *t2).map(|opt| opt.map(|a| QuantumGate::Rx(a, *q1)))
            }
            (QuantumGate::Ry(t1, q1), QuantumGate::Ry(t2, q2)) if q1 == q2 => {
                consolidate_angle_pair(*t1, *t2).map(|opt| opt.map(|a| QuantumGate::Ry(a, *q1)))
            }
            _ => None,
        }
    }

    /// Returns true if two quantum gates fail to commute (i.e. [G1, G2] != 0).
    /// If two gates commute ([G1, G2] == 0), one can slide past the other without changing circuit semantics.
    fn gates_interfere(&self, g1: &QuantumGate, g2: &QuantumGate) -> bool {
        // Non-unitary operations (measurement, reset, custom intrinsics) and barriers partition unitary
        // epochs and must not be reordered past unitary gates (keeps causal epochs & segment verifier sound).
        if matches!(
            g1,
            QuantumGate::Mz(_, _) | QuantumGate::Reset(_) | QuantumGate::Custom { .. }
        ) || matches!(
            g2,
            QuantumGate::Mz(_, _) | QuantumGate::Reset(_) | QuantumGate::Custom { .. }
        ) {
            return true;
        }
        if matches!(g1, QuantumGate::Barrier(b) if b.is_empty())
            || matches!(g2, QuantumGate::Barrier(b) if b.is_empty())
        {
            return true;
        }

        let qs1: HashSet<Qubit> = g1.active_qubits().into_iter().collect();
        let qs2: HashSet<Qubit> = g2.active_qubits().into_iter().collect();

        // 1. Spatially disjoint qubits commute unconditionally: [A ⊗ I, I ⊗ B] = 0
        if qs1.is_disjoint(&qs2) {
            return false;
        }

        let is_z_family = |g: &QuantumGate, q: Qubit| -> bool {
            match g {
                QuantumGate::Z(t)
                | QuantumGate::S(t)
                | QuantumGate::Sdg(t)
                | QuantumGate::T(t)
                | QuantumGate::Tdg(t)
                | QuantumGate::Rz(_, t) => *t == q,
                _ => false,
            }
        };

        let is_x_family = |g: &QuantumGate, q: Qubit| -> bool {
            match g {
                QuantumGate::X(t) | QuantumGate::Rx(_, t) => *t == q,
                _ => false,
            }
        };

        let is_y_family = |g: &QuantumGate, q: Qubit| -> bool {
            match g {
                QuantumGate::Y(t) | QuantumGate::Ry(_, t) => *t == q,
                _ => false,
            }
        };

        for &q in &qs1 {
            if qs2.contains(&q) {
                if is_z_family(g1, q) && is_z_family(g2, q) {
                    continue;
                }
                if is_x_family(g1, q) && is_x_family(g2, q) {
                    continue;
                }
                if is_y_family(g1, q) && is_y_family(g2, q) {
                    continue;
                }

                match (g1, g2) {
                    (g_z, QuantumGate::CX(c, t)) | (QuantumGate::CX(c, t), g_z)
                        if c != t && q == *c && is_z_family(g_z, q) =>
                    {
                        continue
                    }
                    (g_x, QuantumGate::CX(c, t)) | (QuantumGate::CX(c, t), g_x)
                        if c != t && q == *t && is_x_family(g_x, q) =>
                    {
                        continue
                    }
                    (g_z, QuantumGate::CZ(c, t)) | (QuantumGate::CZ(c, t), g_z)
                        if (q == *c || q == *t) && is_z_family(g_z, q) =>
                    {
                        continue
                    }
                    (QuantumGate::CZ(_, _), QuantumGate::CZ(_, _)) => continue,
                    (QuantumGate::CX(c1, t1), QuantumGate::CX(c2, t2))
                        if c1 != t1 && c2 != t2 && c1 == c2 =>
                    {
                        continue
                    }
                    (QuantumGate::CX(c1, t1), QuantumGate::CX(c2, t2))
                        if c1 != t1 && c2 != t2 && t1 == t2 =>
                    {
                        continue
                    }
                    (g_z, QuantumGate::CCX(c1, c2, t)) | (QuantumGate::CCX(c1, c2, t), g_z)
                        if q != *t && (q == *c1 || q == *c2) && is_z_family(g_z, q) =>
                    {
                        continue
                    }
                    (g_x, QuantumGate::CCX(c1, c2, t)) | (QuantumGate::CCX(c1, c2, t), g_x)
                        if q != *c1 && q != *c2 && q == *t && is_x_family(g_x, q) =>
                    {
                        continue
                    }
                    (QuantumGate::CCX(c1_a, c2_a, t_a), QuantumGate::CCX(c1_b, c2_b, t_b))
                        if c1_a != c2_a
                            && c1_a != t_a
                            && c2_a != t_a
                            && c1_b != c2_b
                            && c1_b != t_b
                            && c2_b != t_b
                            && ((c1_a == c1_b && c2_a == c2_b)
                                || (c1_a == c2_b && c2_a == c1_b)) =>
                    {
                        continue
                    }
                    _ => return true,
                }
            }
        }

        false
    }

    /// Format executive anti-obfuscation audit report.
    pub fn generate_report(
        &self,
        _orig: &[QuantumInstruction],
        _cleaned: &[QuantumInstruction],
    ) -> String {
        let mut report = String::new();
        report.push_str("  ID   SEVERITY   PATTERN TYPE                    DESCRIPTION\n");
        report.push_str(
            "  ─────────────────────────────────────────────────────────────────────────\n",
        );

        if self.findings.is_empty() {
            report.push_str("  --   NORMAL     CANONICAL_CIRCUIT               No quantum obfuscation detected.\n");
        } else {
            for f in &self.findings {
                report.push_str(&format!(
                    "  {:02}   {:<10} {:<31} {}\n",
                    f.id,
                    f.severity.to_string(),
                    f.pattern_type,
                    f.description
                ));
            }
        }
        report.push_str(
            "  ─────────────────────────────────────────────────────────────────────────\n",
        );

        report
    }
}

const ROTATION_EPSILON: f64 = 1e-6;

fn rotation_angle(gate: &QuantumGate) -> Option<f64> {
    match gate {
        QuantumGate::Rx(t, _) | QuantumGate::Ry(t, _) | QuantumGate::Rz(t, _) => Some(*t),
        _ => None,
    }
}

fn is_identity_rotation_angle(theta: f64) -> bool {
    if !theta.is_finite() {
        return false;
    }
    let normalized = theta.rem_euclid(std::f64::consts::TAU);
    normalized < ROTATION_EPSILON || (std::f64::consts::TAU - normalized) < ROTATION_EPSILON
}

fn wrap_angle_signed_pi(theta: f64) -> f64 {
    let wrapped =
        (theta + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if (wrapped + std::f64::consts::PI).abs() < 1e-12 {
        std::f64::consts::PI
    } else if wrapped.abs() == 0.0 {
        0.0
    } else {
        wrapped
    }
}

fn consolidate_angle_pair(t1: f64, t2: f64) -> Option<Option<f64>> {
    if !t1.is_finite() || !t2.is_finite() {
        return None;
    }
    let sum = t1 + t2;
    if !sum.is_finite() {
        return None;
    }
    if is_identity_rotation_angle(sum) {
        Some(None)
    } else {
        Some(Some(wrap_angle_signed_pi(sum)))
    }
}
