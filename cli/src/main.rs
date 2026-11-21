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
#[command(version = "0.1.0")]
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
    /// Formally verify unitary matrix equivalence (U_clean == U_orig) in 2^N Hilbert space
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Disasm { file } => {
            let mut parser = QirParser::new();
            let insts = match parser.parse_file(&file) {
                Ok(i) => i,
                Err(err) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        "[ERROR]".red().bold(),
                        file.display(),
                        err
                    );
                    process::exit(1);
                }
            };

            let filename = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("source.ll");
            println!("{}", QirEmitter::format_disassembly(&insts, filename));
        }

        Commands::Circuit { file } => {
            let mut parser = QirParser::new();
            let insts = match parser.parse_file(&file) {
                Ok(i) => i,
                Err(err) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        "[ERROR]".red().bold(),
                        file.display(),
                        err
                    );
                    process::exit(1);
                }
            };

            println!("{}", QirEmitter::render_ascii_circuit(&insts));
        }

        Commands::Deobf { file, qasm } => {
            let mut parser = QirParser::new();
            let insts = match parser.parse_file(&file) {
                Ok(i) => i,
                Err(err) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        "[ERROR]".red().bold(),
                        file.display(),
                        err
                    );
                    process::exit(1);
                }
            };

            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);

            println!("{}", deobf.generate_report(&insts, &cleaned));

            if qasm {
                println!(
                    "\n; ── RECONSTRUCTED CANONICAL OPENQASM 3.0 ─────────────────────────────────"
                );
                println!(
                    "{}",
                    QirEmitter::to_openqasm(
                        &cleaned,
                        parser.metadata.num_qubits,
                        parser.metadata.num_results
                    )
                );
            }
        }

        Commands::Verify { file } => {
            let mut parser = QirParser::new();
            let insts = match parser.parse_file(&file) {
                Ok(i) => i,
                Err(err) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        "[ERROR]".red().bold(),
                        file.display(),
                        err
                    );
                    process::exit(1);
                }
            };

            let mut deobf = QirDeobfuscator::new();
            let (live_orig, dead_qubits) = deobf.prune_dead_qubits(&insts);
            let cleaned = deobf.deobfuscate(&insts);

            let max_live_q = live_orig
                .iter()
                .chain(cleaned.iter())
                .flat_map(|i| i.gate.active_qubits())
                .map(|q| q.0)
                .max()
                .map(|m| m + 1)
                .unwrap_or(0) as usize;

            let n_qubits = max_live_q.max(1);

            println!("═══════════════════════════════════════════════════════════════════════════");
            println!(" Unitary Equivalence Verification (2^{} States)", n_qubits);
            println!(" Target File              : {}", file.display());
            println!(
                " Observable Qubits        : {} (dimension = {} states)",
                n_qubits,
                1 << n_qubits
            );
            if !dead_qubits.is_empty() {
                let mut dq_sorted: Vec<_> = dead_qubits.iter().map(|q| format!("{}", q)).collect();
                dq_sorted.sort();
                println!(" Pruned Unmeasured Qubits : {}", dq_sorted.join(", "));
            }
            println!(" Original Instructions    : {}", insts.len());
            println!(" Deobfuscated Instructions: {}", cleaned.len());
            println!(
                "═══════════════════════════════════════════════════════════════════════════\n"
            );

            let res = UnitaryVerifier::verify_equivalence(&live_orig, &cleaned, n_qubits);

            if let Some(err) = &res.error_message {
                eprintln!("  {} {}\n", "[✗] VERIFICATION BLOCKED:".red().bold(), err);
                process::exit(1);
            }

            println!("  Frobenius Matrix Error    : {:.10}", res.frobenius_error);
            println!(
                "  Quantum Process Fidelity  : {:.8} (1.0 = perfect invariance)",
                res.fidelity
            );
            println!("  Unitarity Drift ||U·U†-I||: {:.10}", res.unitarity_error);

            if res.is_equivalent {
                println!("\n  {} Formal Unitary Equivalence Mathematically Proven (U_orig ≡ U_clean on Observable Subspace)", "[✓] PASSED:".green().bold());
                println!(
                    "  {} Circuit Reversibility Theorem Satisfied: U_clean · U_clean† = I\n",
                    "[✓] SOUND:".green().bold()
                );
            } else {
                println!(
                    "\n  {} Unitary matrices diverge! Circuit semantics altered.\n",
                    "[✗] FAILED:".red().bold()
                );
                process::exit(1);
            }
        }

        Commands::Export { file, format } => {
            let mut parser = QirParser::new();
            let insts = match parser.parse_file(&file) {
                Ok(i) => i,
                Err(err) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        "[ERROR]".red().bold(),
                        file.display(),
                        err
                    );
                    process::exit(1);
                }
            };

            let mut deobf = QirDeobfuscator::new();
            let cleaned = deobf.deobfuscate(&insts);

            match format.to_lowercase().as_str() {
                "svg" => {
                    print!(
                        "{}",
                        QirEmitter::render_svg_circuit(&cleaned, parser.metadata.num_qubits)
                    );
                }
                _ => {
                    print!(
                        "{}",
                        QirEmitter::to_openqasm(
                            &cleaned,
                            parser.metadata.num_qubits,
                            parser.metadata.num_results
                        )
                    );
                }
            }
        }
    }
}
