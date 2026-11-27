//! QIR Reversing CLI
//! Author: Matvei Igumnov <matvei@iqumnov.com>

use clap::{Parser, Subcommand};
use colored::Colorize;
use qir_core::{QirDeobfuscator, QirEmitter, QirParser, UnitaryVerifier};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "qir-rev")]
#[command(author = "Matvei Igumnov <matvei@iqumnov.com>")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Quantum Intermediate Representation (QIR) Reverse Engineering & Verification", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Disassemble QIR LLVM IR / Bitcode file with quantum mnemonics and semantic annotations
    Disasm {
        /// Path to QIR .ll or .bc file
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// Output decoded textual LLVM IR instead of tabular disassembly (useful for .bc files)
        #[arg(long)]
        emit_ir: bool,
    },
    /// Render 2D ASCII quantum circuit timeline diagram
    Circuit {
        /// Path to QIR .ll or .bc file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Execute multi-pass quantum deobfuscation and verification
    Deobf {
        /// Path to QIR .ll or .bc file
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// Export clean circuit directly to OpenQASM 3.0
        #[arg(short, long)]
        qasm: bool,
    },
    /// Formally verify unitary matrix equivalence (Dense 2^N <=12q, QMDD <=64q)
    Verify {
        /// Path to QIR .ll or .bc file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Export canonical quantum circuit (OpenQASM 3.0 or Vector SVG)
    Export {
        /// Path to QIR .ll or .bc file
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// Target export format: qasm (default) or svg
        #[arg(short, long, default_value = "qasm")]
        format: String,
    },
}

fn parse_qir_input(file: &std::path::Path) -> (QirParser, Vec<qir_core::QuantumInstruction>) {
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(err) => {
            eprintln!(
                "{} Failed to read {}: {}",
                "[ERROR]".red().bold(),
                file.display(),
                err
            );
            process::exit(1);
        }
    };
    let ir = if qir_core::bitcode::is_llvm_bitcode(&bytes) {
        match qir_core::bitcode::decode_bitcode_to_ir(&bytes) {
            Ok(s) => s,
            Err(err) => {
                eprintln!(
                    "{} Failed to decode bitcode {}: {}",
                    "[ERROR]".red().bold(),
                    file.display(),
                    err
                );
                process::exit(1);
            }
        }
    } else {
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(err) => {
                eprintln!(
                    "{} Failed to decode UTF-8 IR {}: {}",
                    "[ERROR]".red().bold(),
                    file.display(),
                    err
                );
                process::exit(1);
            }
        };
        if text.trim().is_empty() {
            eprintln!(
                "{} Input file {} is empty.",
                "[ERROR]".red().bold(),
                file.display()
            );
            process::exit(1);
        }
        if !qir_core::parser::looks_like_llvm_ir(&text) {
            eprintln!(
                "{} {} does not contain recognizable LLVM IR or QIR instructions.",
                "[ERROR]".red().bold(),
                file.display()
            );
            process::exit(1);
        }
        text
    };
    let mut parser = QirParser::new();
    match parser.parse_ir(&ir) {
        Ok(insts) => (parser, insts),
        Err(err) => {
            eprintln!(
                "{} Failed to parse {}: {}",
                "[ERROR]".red().bold(),
                file.display(),
                err
            );
            process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Disasm { file, emit_ir } => {
            if emit_ir {
                let bytes = match std::fs::read(&file) {
                    Ok(b) => b,
                    Err(err) => {
                        eprintln!(
                            "{} Failed to read {}: {}",
                            "[ERROR]".red().bold(),
                            file.display(),
                            err
                        );
                        process::exit(1);
                    }
                };
                let ir = if qir_core::bitcode::is_llvm_bitcode(&bytes) {
                    match qir_core::bitcode::decode_bitcode_to_ir(&bytes) {
                        Ok(s) => s,
                        Err(err) => {
                            eprintln!(
                                "{} Failed to decode bitcode {}: {}",
                                "[ERROR]".red().bold(),
                                file.display(),
                                err
                            );
                            process::exit(1);
                        }
                    }
                } else {
                    match String::from_utf8(bytes) {
                        Ok(s) => {
                            if s.trim().is_empty() || !qir_core::parser::looks_like_llvm_ir(&s) {
                                eprintln!(
                                    "{} {} does not contain recognizable LLVM IR or QIR instructions.",
                                    "[ERROR]".red().bold(),
                                    file.display()
                                );
                                process::exit(1);
                            }
                            s
                        }
                        Err(err) => {
                            eprintln!(
                                "{} Failed to decode UTF-8 IR {}: {}",
                                "[ERROR]".red().bold(),
                                file.display(),
                                err
                            );
                            process::exit(1);
                        }
                    }
                };
                print!("{}", ir);
                return;
            }
            let (_parser, insts) = parse_qir_input(&file);

            let filename = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("source.ll");
            println!("{}", QirEmitter::format_disassembly(&insts, filename));
        }

        Commands::Circuit { file } => {
            let (_parser, insts) = parse_qir_input(&file);

            println!("{}", QirEmitter::render_ascii_circuit(&insts));
        }

        Commands::Deobf { file, qasm } => {
            let (_parser, insts) = parse_qir_input(&file);

            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);

            println!("{}", deobf.generate_report(&insts, &cleaned));

            if qasm {
                let live_qubits = cleaned
                    .iter()
                    .flat_map(|i| i.gate.active_qubits())
                    .map(|q| q.0.saturating_add(1))
                    .max()
                    .unwrap_or(1);
                println!(
                    "\n; ── RECONSTRUCTED CANONICAL OPENQASM 3.0 ─────────────────────────────────"
                );
                println!("{}", QirEmitter::to_openqasm(&cleaned, live_qubits, 0));
            }
        }

        Commands::Verify { file } => {
            let (_parser, insts) = parse_qir_input(&file);

            let mut deobf = QirDeobfuscator::new();
            let (live_orig, dead_qubits) = deobf.prune_dead_qubits(&insts);
            let cleaned = deobf.deobfuscate(&insts);

            let max_live_q = live_orig
                .iter()
                .chain(cleaned.iter())
                .flat_map(|i| i.gate.active_qubits())
                .map(|q| q.0)
                .max()
                .map(|m| m.saturating_add(1))
                .unwrap_or(0) as usize;

            let n_qubits = max_live_q.max(1);

            let dim_label = match 1usize.checked_shl(n_qubits as u32) {
                Some(dim) => format!("{} states", dim),
                None => format!("2^{} states", n_qubits),
            };
            let engine_label = if n_qubits > 12 {
                "QMDD Alternating Verification"
            } else {
                "Unitary Equivalence Verification"
            };
            let orig_gate_count = insts.iter().filter(|i| !i.is_control_flow).count();
            let clean_gate_count = cleaned.iter().filter(|i| !i.is_control_flow).count();
            println!("═══════════════════════════════════════════════════════════════════════════");
            println!(" {} (2^{} States)", engine_label, n_qubits);
            println!(" Target File              : {}", file.display());
            println!(
                " Observable Qubits        : {} (dimension = {})",
                n_qubits, dim_label
            );
            if !dead_qubits.is_empty() {
                let mut dq_sorted: Vec<_> = dead_qubits.iter().copied().collect();
                dq_sorted.sort_by_key(|q| q.0);
                let dq_labels: Vec<_> = dq_sorted.iter().map(|q| format!("{}", q)).collect();
                println!(" Pruned Unmeasured Qubits : {}", dq_labels.join(", "));
            }
            println!(" Original Instructions    : {}", orig_gate_count);
            println!(" Deobfuscated Instructions: {}", clean_gate_count);
            println!(
                "═══════════════════════════════════════════════════════════════════════════\n"
            );

            let res = UnitaryVerifier::verify_equivalence(&live_orig, &cleaned, n_qubits);

            if let Some(err) = &res.error_message {
                eprintln!("  {} {}\n", "[✗] VERIFICATION BLOCKED:".red().bold(), err);
                process::exit(1);
            }

            if let Some(method) = &res.verification_method {
                println!("  Verification Backend      : {}", method);
            }
            println!("  Frobenius Matrix Error    : {:.10}", res.frobenius_error);
            println!(
                "  Quantum Process Fidelity  : {:.8} (1.0 = perfect invariance)",
                res.fidelity
            );
            println!("  Unitarity Drift ||U·U†-I||: {:.10}", res.unitarity_error);

            if res.is_equivalent {
                println!("\n  {} Formal Unitary Equivalence Mathematically Proven (U_orig ≡ U_clean on Observable Subspace)", "[✓] PASSED:".green().bold());
                let unitarity_bound = qir_core::unitary::unitarity_tolerance(res.num_qubits);
                if res.unitarity_error < unitarity_bound {
                    println!(
                        "  {} Circuit Reversibility Theorem Satisfied: U_clean · U_clean† = I\n",
                        "[✓] SOUND:".green().bold()
                    );
                } else {
                    eprintln!(
                        "  {} Unitarity drift ({:.3e}) exceeds {:.3e} bound!\n",
                        "[✗] NON-UNITARY:".red().bold(),
                        res.unitarity_error,
                        unitarity_bound
                    );
                    process::exit(1);
                }
            } else {
                println!(
                    "\n  {} Unitary matrices diverge! Circuit semantics altered.\n",
                    "[✗] FAILED:".red().bold()
                );
                process::exit(1);
            }
        }

        Commands::Export { file, format } => {
            let (_parser, insts) = parse_qir_input(&file);

            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);
            let live_qubits = cleaned
                .iter()
                .flat_map(|i| i.gate.active_qubits())
                .map(|q| q.0.saturating_add(1))
                .max()
                .unwrap_or(1);

            match format.to_lowercase().as_str() {
                "svg" => {
                    print!("{}", QirEmitter::render_svg_circuit(&cleaned, live_qubits));
                }
                "qasm" | "openqasm" => {
                    print!("{}", QirEmitter::to_openqasm(&cleaned, live_qubits, 0));
                }
                other => {
                    eprintln!(
                        "{} Unsupported export format '{}'. Expected 'qasm' or 'svg'.",
                        "[ERROR]".red().bold(),
                        other
                    );
                    process::exit(1);
                }
            }
        }
    }
}
