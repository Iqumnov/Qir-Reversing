//! Quantum Circuit Emitter (OpenQASM 3.0, ASCII Timeline Matrix, Vector SVG)
//! Author: Matvei Igumnov <iqumnov@proton.me>

use crate::types::{QuantumGate, QuantumInstruction, Qubit};
use std::collections::HashSet;

/// Formats disassembly, ASCII circuits, and OpenQASM 3.0.
pub struct QirEmitter;

impl QirEmitter {
    /// Format reverse engineering table with hex opcodes and semantic comments.
    pub fn format_disassembly(instructions: &[QuantumInstruction], _filename: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "  {:<10} {:<14} {:<26} ANNOTATION\n",
            "OFFSET", "OPCODE", "INSTRUCTION"
        ));
        out.push_str(
            "  ─────────────────────────────────────────────────────────────────────────\n",
        );

        for inst in instructions {
            let bytes = inst.gate.synthetic_opcode();
            let hex_str = format!(
                "{:02x} {:02x} {:02x} {:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            );
            let mnem = inst.gate.mnemonic();
            let comment = Self::semantic_comment(&inst.gate);

            out.push_str(&format!(
                "  0x{:06x}  {:<14} {:<26} ; {}\n",
                inst.address, hex_str, mnem, comment
            ));
        }
        out.push_str(
            "  ─────────────────────────────────────────────────────────────────────────\n",
        );
        out
    }

    fn semantic_comment(gate: &QuantumGate) -> String {
        match gate {
            QuantumGate::H(q) => format!("Superposition: |0> -> (|0>+|1>)/√2 on {}", q),
            QuantumGate::X(q) => format!("Pauli-X (Bit-Flip / NOT) on {}", q),
            QuantumGate::Y(q) => format!("Pauli-Y (Bit+Phase Flip) on {}", q),
            QuantumGate::Z(q) => format!("Pauli-Z (Phase-Flip) on {}", q),
            QuantumGate::S(q) => format!("Phase S gate (+π/2 phase) on {}", q),
            QuantumGate::Sdg(q) => format!("Adjoint Phase S† gate (-π/2 phase) on {}", q),
            QuantumGate::T(q) => format!("Phase T gate (+π/4 phase) on {}", q),
            QuantumGate::Tdg(q) => format!("Adjoint Phase T† gate (-π/4 phase) on {}", q),
            QuantumGate::Rx(theta, q) => format!("Bit-Rotation Rx({:.4} rad) on {}", theta, q),
            QuantumGate::Ry(theta, q) => format!("Rotation Ry({:.4} rad) on {}", theta, q),
            QuantumGate::Rz(theta, q) => format!("Phase Rotation Rz({:.4} rad) on {}", theta, q),
            QuantumGate::CX(c, t) => format!("Entanglement: CNOT (ctrl={}, tgt={})", c, t),
            QuantumGate::CZ(c, t) => format!("Controlled-Z phase flip (ctrl={}, tgt={})", c, t),
            QuantumGate::SWAP(q1, q2) => format!("SWAP quantum states between {} and {}", q1, q2),
            QuantumGate::CCX(c1, c2, t) => {
                format!("Toffoli (CCX) gate on ({}, {}) -> tgt={}", c1, c2, t)
            }
            QuantumGate::Mz(q, r) => format!(
                "Projective Z-measurement: collapse {} -> classical {}",
                q, r
            ),
            QuantumGate::Reset(q) => format!("State Reset: project {} back to ground state |0>", q),
            QuantumGate::Barrier(qs) => {
                format!("Circuit scheduling barrier on {} qubits", qs.len())
            }
            QuantumGate::Custom { name, .. } => format!("Custom quantum intrinsic: {}", name),
        }
    }

    /// Renders 2D ASCII timeline diagram.
    pub fn render_ascii_circuit(instructions: &[QuantumInstruction]) -> String {
        let mut max_qubit = 0;
        let mut max_result = 0;

        for inst in instructions {
            for q in inst.gate.active_qubits() {
                if q.0 > max_qubit {
                    max_qubit = q.0;
                }
            }
            if let QuantumGate::Mz(_, r) = inst.gate {
                if r.0 > max_result {
                    max_result = r.0;
                }
            }
        }

        let num_qubits = (max_qubit + 1) as usize;
        let num_results = (max_result + 1) as usize;

        let mut wires: Vec<String> = (0..num_qubits).map(|i| format!("q[{}]: ──", i)).collect();

        for inst in instructions {
            let active = inst.gate.active_qubits();
            let active_set: HashSet<Qubit> = active.iter().copied().collect();

            match &inst.gate {
                QuantumGate::H(q) => wires[q.0 as usize].push_str("[H]──"),
                QuantumGate::X(q) => wires[q.0 as usize].push_str("[X]──"),
                QuantumGate::Y(q) => wires[q.0 as usize].push_str("[Y]──"),
                QuantumGate::Z(q) => wires[q.0 as usize].push_str("[Z]──"),
                QuantumGate::S(q) => wires[q.0 as usize].push_str("[S]──"),
                QuantumGate::Sdg(q) => wires[q.0 as usize].push_str("[S†]─"),
                QuantumGate::T(q) => wires[q.0 as usize].push_str("[T]──"),
                QuantumGate::Tdg(q) => wires[q.0 as usize].push_str("[T†]─"),
                QuantumGate::Rx(_, q) => wires[q.0 as usize].push_str("[RX]─"),
                QuantumGate::Ry(_, q) => wires[q.0 as usize].push_str("[RY]─"),
                QuantumGate::Rz(_, q) => wires[q.0 as usize].push_str("[RZ]─"),
                QuantumGate::CX(c, t) => {
                    wires[c.0 as usize].push_str("─●───");
                    wires[t.0 as usize].push_str("─X───");
                }
                QuantumGate::CZ(c, t) => {
                    wires[c.0 as usize].push_str("─●───");
                    wires[t.0 as usize].push_str("─Z───");
                }
                QuantumGate::SWAP(q1, q2) => {
                    wires[q1.0 as usize].push_str("─×───");
                    wires[q2.0 as usize].push_str("─×───");
                }
                QuantumGate::CCX(c1, c2, t) => {
                    wires[c1.0 as usize].push_str("─●───");
                    wires[c2.0 as usize].push_str("─●───");
                    wires[t.0 as usize].push_str("─X───");
                }
                QuantumGate::Mz(q, _) => wires[q.0 as usize].push_str("[M]──"),
                QuantumGate::Reset(q) => wires[q.0 as usize].push_str("[|0>]"),
                _ => {}
            }

            let (min_q, max_q) = if active.len() >= 2 {
                let min = active.iter().map(|q| q.0).min().unwrap_or(0);
                let max = active.iter().map(|q| q.0).max().unwrap_or(0);
                (Some(min), Some(max))
            } else {
                (None, None)
            };

            // Pad inactive wires, routing multi-qubit lines across intervening wires
            let max_len = wires.iter().map(|w| w.chars().count()).max().unwrap_or(0);
            for (idx, wire) in wires.iter_mut().enumerate() {
                let q_idx = idx as u32;
                if !active_set.contains(&Qubit(q_idx)) {
                    let is_intervening = match (min_q, max_q) {
                        (Some(min), Some(max)) => q_idx > min && q_idx < max,
                        _ => false,
                    };
                    while wire.chars().count() < max_len {
                        if is_intervening && max_len - wire.chars().count() >= 5 {
                            wire.push_str("─┼───");
                        } else {
                            wire.push('─');
                        }
                    }
                }
            }
        }

        let has_measurements = instructions
            .iter()
            .any(|i| matches!(i.gate, QuantumGate::Mz(_, _)));
        let mut out = String::new();
        out.push_str(
            "; ── QUANTUM CIRCUIT TIMELINE DIAGRAM ──────────────────────────────────────\n",
        );
        for wire in wires {
            out.push_str(&wire);
            out.push('\n');
        }
        if has_measurements {
            out.push_str(
                "; ── CLASSICAL MEASUREMENT REGISTERS ───────────────────────────────────────\n",
            );
            for r in 0..num_results {
                out.push_str(&format!(
                    "r[{}]: ═══════════════════════════════════════════════════════════════════\n",
                    r
                ));
            }
        }
        out.push_str(
            "; ──────────────────────────────────────────────────────────────────────────\n",
        );
        out
    }

    /// Decompiles instructions to OpenQASM 3.0 source code.
    pub fn to_openqasm(
        instructions: &[QuantumInstruction],
        num_qubits: u32,
        num_results: u32,
    ) -> String {
        let max_q = instructions
            .iter()
            .flat_map(|inst| inst.gate.active_qubits())
            .map(|q| q.0 + 1)
            .max()
            .unwrap_or(0);
        let n_q = num_qubits.max(max_q).max(1);

        let max_r = instructions
            .iter()
            .filter_map(|inst| {
                if let QuantumGate::Mz(_, r) = inst.gate {
                    Some(r.0 + 1)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);
        let n_r = num_results.max(max_r);

        let mut qasm = String::new();
        qasm.push_str("OPENQASM 3.0;\n");
        qasm.push_str("include \"stdgates.inc\";\n\n");
        qasm.push_str(&format!("qubit[{}] q;\n", n_q));
        if n_r > 0 {
            qasm.push_str(&format!("bit[{}] r;\n", n_r));
        }
        qasm.push('\n');

        for inst in instructions {
            match &inst.gate {
                QuantumGate::H(q) => qasm.push_str(&format!("h q[{}];\n", q.0)),
                QuantumGate::X(q) => qasm.push_str(&format!("x q[{}];\n", q.0)),
                QuantumGate::Y(q) => qasm.push_str(&format!("y q[{}];\n", q.0)),
                QuantumGate::Z(q) => qasm.push_str(&format!("z q[{}];\n", q.0)),
                QuantumGate::S(q) => qasm.push_str(&format!("s q[{}];\n", q.0)),
                QuantumGate::Sdg(q) => qasm.push_str(&format!("sdg q[{}];\n", q.0)),
                QuantumGate::T(q) => qasm.push_str(&format!("t q[{}];\n", q.0)),
                QuantumGate::Tdg(q) => qasm.push_str(&format!("tdg q[{}];\n", q.0)),
                QuantumGate::Rx(theta, q) => qasm.push_str(&format!("rx({}) q[{}];\n", theta, q.0)),
                QuantumGate::Ry(theta, q) => qasm.push_str(&format!("ry({}) q[{}];\n", theta, q.0)),
                QuantumGate::Rz(theta, q) => qasm.push_str(&format!("rz({}) q[{}];\n", theta, q.0)),
                QuantumGate::CX(c, t) => qasm.push_str(&format!("cx q[{}], q[{}];\n", c.0, t.0)),
                QuantumGate::CZ(c, t) => qasm.push_str(&format!("cz q[{}], q[{}];\n", c.0, t.0)),
                QuantumGate::SWAP(q1, q2) => {
                    qasm.push_str(&format!("swap q[{}], q[{}];\n", q1.0, q2.0))
                }
                QuantumGate::CCX(c1, c2, t) => {
                    qasm.push_str(&format!("ccx q[{}], q[{}], q[{}];\n", c1.0, c2.0, t.0))
                }
                QuantumGate::Mz(q, r) => {
                    qasm.push_str(&format!("r[{}] = measure q[{}];\n", r.0, q.0))
                }
                QuantumGate::Reset(q) => qasm.push_str(&format!("reset q[{}];\n", q.0)),
                QuantumGate::Barrier(qs) => {
                    let qs_str = qs
                        .iter()
                        .map(|q| format!("q[{}]", q.0))
                        .collect::<Vec<_>>()
                        .join(", ");
                    qasm.push_str(&format!("barrier {};\n", qs_str));
                }
                QuantumGate::Custom { name, qubits, .. } => {
                    let qs_str = qubits
                        .iter()
                        .map(|q| format!("q[{}]", q.0))
                        .collect::<Vec<_>>()
                        .join(", ");
                    qasm.push_str(&format!("// {} {}\n", name, qs_str));
                }
            }
        }
        qasm
    }

    /// Renders vector SVG quantum circuit diagram with dark tactical aesthetic.
    pub fn render_svg_circuit(instructions: &[QuantumInstruction], num_qubits: u32) -> String {
        let n_q = num_qubits.max(1) as usize;
        let n_cols = instructions.len().max(1);

        let col_width = 60.0;
        let row_height = 50.0;
        let start_x = 80.0;
        let start_y = 50.0;

        let width = start_x + (n_cols as f64) * col_width + 40.0;
        let height = start_y + (n_q as f64) * row_height + 30.0;

        let mut svg = String::new();
        svg.push_str(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.1} {:.1}" width="{:.1}" height="{:.1}" style="background-color: #000000; font-family: 'JetBrains Mono', 'SF Mono', monospace;">"##,
            width, height, width, height
        ));
        svg.push('\n');

        // Draw quantum wire lines
        for i in 0..n_q {
            let y = start_y + (i as f64) * row_height;
            svg.push_str(&format!(
                r##"  <text x="20" y="{:.1}" fill="#808080" font-size="12" font-weight="bold" dominant-baseline="middle">q[{}]</text>"##,
                y, i
            ));
            svg.push('\n');
            svg.push_str(&format!(
                r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#222222" stroke-width="1.5" />"##,
                start_x - 10.0, y, width - 20.0, y
            ));
            svg.push('\n');
        }

        // Draw gates
        for (col, inst) in instructions.iter().enumerate() {
            let cx = start_x + (col as f64) * col_width + 20.0;

            match &inst.gate {
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
                | QuantumGate::Reset(q) => {
                    let cy = start_y + (q.0 as f64) * row_height;
                    let label = match &inst.gate {
                        QuantumGate::H(_) => "H",
                        QuantumGate::X(_) => "X",
                        QuantumGate::Y(_) => "Y",
                        QuantumGate::Z(_) => "Z",
                        QuantumGate::S(_) => "S",
                        QuantumGate::Sdg(_) => "S†",
                        QuantumGate::T(_) => "T",
                        QuantumGate::Tdg(_) => "T†",
                        QuantumGate::Rx(_, _) => "Rx",
                        QuantumGate::Ry(_, _) => "Ry",
                        QuantumGate::Rz(_, _) => "Rz",
                        QuantumGate::Mz(_, _) => "M",
                        QuantumGate::Reset(_) => "|0⟩",
                        _ => "?",
                    };
                    let border = match &inst.gate {
                        QuantumGate::Mz(_, _) => "#888888",
                        QuantumGate::Reset(_) => "#555555",
                        _ => "#ffffff",
                    };
                    svg.push_str(&format!(
                        r##"  <rect x="{:.1}" y="{:.1}" width="32" height="32" rx="0" fill="#0d0d0d" stroke="{}" stroke-width="1" />"##,
                        cx - 16.0, cy - 16.0, border
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <text x="{:.1}" y="{:.1}" fill="#ffffff" font-size="12" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        cx, cy, label
                    ));
                    svg.push('\n');
                }
                QuantumGate::CX(c, t) => {
                    let cy_c = start_y + (c.0 as f64) * row_height;
                    let cy_t = start_y + (t.0 as f64) * row_height;
                    svg.push_str(&format!(
                        r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, cy_c, cx, cy_t
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="5" fill="#ffffff" />"##,
                        cx, cy_c
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="11" fill="#0d0d0d" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, cy_t
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <text x="{:.1}" y="{:.1}" fill="#ffffff" font-size="13" font-weight="bold" text-anchor="middle" dominant-baseline="middle">⊕</text>"##,
                        cx, cy_t
                    ));
                    svg.push('\n');
                }
                QuantumGate::CZ(c, t) => {
                    let cy_c = start_y + (c.0 as f64) * row_height;
                    let cy_t = start_y + (t.0 as f64) * row_height;
                    svg.push_str(&format!(
                        r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, cy_c, cx, cy_t
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="5" fill="#ffffff" />"##,
                        cx, cy_c
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="5" fill="#ffffff" />"##,
                        cx, cy_t
                    ));
                    svg.push('\n');
                }
                QuantumGate::SWAP(q1, q2) => {
                    let cy_1 = start_y + (q1.0 as f64) * row_height;
                    let cy_2 = start_y + (q2.0 as f64) * row_height;
                    svg.push_str(&format!(
                        r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, cy_1, cx, cy_2
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <text x="{:.1}" y="{:.1}" fill="#ffffff" font-size="14" font-weight="bold" text-anchor="middle" dominant-baseline="middle">✕</text>"##,
                        cx, cy_1
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <text x="{:.1}" y="{:.1}" fill="#ffffff" font-size="14" font-weight="bold" text-anchor="middle" dominant-baseline="middle">✕</text>"##,
                        cx, cy_2
                    ));
                    svg.push('\n');
                }
                QuantumGate::CCX(c1, c2, t) => {
                    let cy_c1 = start_y + (c1.0 as f64) * row_height;
                    let cy_c2 = start_y + (c2.0 as f64) * row_height;
                    let cy_t = start_y + (t.0 as f64) * row_height;
                    let min_y = cy_c1.min(cy_c2).min(cy_t);
                    let max_y = cy_c1.max(cy_c2).max(cy_t);
                    svg.push_str(&format!(
                        r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, min_y, cx, max_y
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="5" fill="#ffffff" />"##,
                        cx, cy_c1
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="5" fill="#ffffff" />"##,
                        cx, cy_c2
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <circle cx="{:.1}" cy="{:.1}" r="11" fill="#0d0d0d" stroke="#ffffff" stroke-width="1.5" />"##,
                        cx, cy_t
                    ));
                    svg.push('\n');
                    svg.push_str(&format!(
                        r##"  <text x="{:.1}" y="{:.1}" fill="#ffffff" font-size="13" font-weight="bold" text-anchor="middle" dominant-baseline="middle">⊕</text>"##,
                        cx, cy_t
                    ));
                    svg.push('\n');
                }
                _ => {}
            }
        }

        svg.push_str("</svg>\n");
        svg
    }
}
