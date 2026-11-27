#!/usr/bin/env python3
"""
Rizin QIR Extension Plugin & CLI Interface
Author: Matvei Igumnov <matvei@iqumnov.com>

Integrates the QIR Parser, Disassembler, and Deobfuscator into Rizin (via rzpipe).
Provides interactive shell commands:
  qir           - Show QIR plugin status and target metadata
  qir.disasm    - Disassemble QIR bytecode into Quantum Assembly
  qir.circuit   - Render 2D ASCII Quantum Circuit diagram
  qir.deobf     - Run Anti-Reversing Analysis & Circuit Deobfuscator
  qir.export    - Export clean OpenQASM 3.0
"""

import sys
import os
import re
try:
    import rzpipe
except ImportError:
    rzpipe = None

# Resolve internal paths
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DYNAMIC_RIZIN = os.path.join(BASE_DIR, "bin", "cutter_dist", "usr", "bin", "rizin")
STATIC_RIZIN = os.path.join(BASE_DIR, "bin", "bin", "rizin")
RIZIN_BIN = DYNAMIC_RIZIN if os.path.exists(DYNAMIC_RIZIN) else STATIC_RIZIN

if __package__:
    from .qir_parser import QIRParser
    from .qir_disasm import QIRDisassembler
    from .qir_deobfuscator import QIRDeobfuscator
else:
    _PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
    if _PLUGIN_DIR not in sys.path:
        sys.path.insert(0, _PLUGIN_DIR)
    from qir_parser import QIRParser
    from qir_disasm import QIRDisassembler
    from qir_deobfuscator import QIRDeobfuscator


class RzQIRPlugin:
    """Rizin Plugin Driver for LLVM Quantum Intermediate Representation."""
    def __init__(self, target_file):
        self.target_file = target_file
        self.parser = QIRParser()
        self.disasm = QIRDisassembler(self.parser)
        self.deobf = QIRDeobfuscator()

        self.instructions = self.parser.parse_file(target_file)
        self.rz = None

    def attach_rizin(self):
        """Initializes Rizin subprocess session attached to the target binary."""
        if rzpipe is None:
            raise RuntimeError(
                "rzpipe Python package is not installed; run '--disasm'/'--deobf' offline or install rzpipe."
            )
        if os.path.exists(RIZIN_BIN):
            self.rz = rzpipe.open(self.target_file, rizin_home=os.path.dirname(RIZIN_BIN))
        else:
            self.rz = rzpipe.open(self.target_file)

        # Annotate quantum operations as sanitized flags and comments in Rizin database
        for idx, inst in enumerate(self.instructions):
            addr = inst.address
            mnem = inst.mnemonic()
            safe_mnem = re.sub(r"[^a-zA-Z0-9_.]", "_", mnem.replace("->", "to"))
            safe_comment = re.sub(r"[;|><`$\\]", " ", mnem.replace("->", "to")).strip()
            self.rz.cmd(f"f+ qir.{idx}_{safe_mnem} 4 @ {addr}")
            self.rz.cmd(f"CCu {safe_comment} @ {addr}")

        return self.rz

    def cmd_info(self):
        opcode_count = sum(1 for i in self.instructions if not i.is_control_flow)
        print(f"[Rizin QIR Plugin] Author: Matvei Igumnov <matvei@iqumnov.com>")
        print(f"Target File      : {self.target_file}")
        print(f"QIR Profile      : Custom Adaptive Quantum Profile")
        print(f"Allocated Qubits : {self.parser.num_qubits}")
        print(f"Result Registers : {self.parser.num_results}")
        print(f"Decoded Opcodes  : {opcode_count}")

    def cmd_disasm(self):
        print(self.disasm.format_disassembly(self.instructions, self.target_file))

    def cmd_circuit(self):
        print(self.disasm.render_circuit_diagram(self.instructions))

    def cmd_deobf(self):
        cleaned = self.deobf.deobfuscate(self.instructions)
        print(self.deobf.format_report())
        print("\n[+] Cleaned Deobfuscated Circuit Listing:")
        print(self.disasm.format_disassembly(cleaned, self.target_file + " [DEOBFUSCATED]"))
        print()
        print(self.disasm.render_circuit_diagram(cleaned))

    def cmd_export(self):
        cleaned = self.deobf.deobfuscate(self.instructions)
        print(self.disasm.export_openqasm(cleaned))

    def run_interactive(self):
        """Runs interactive Rizin QIR console session."""
        print(f"[*] Initializing Rizin QIR Environment for: {self.target_file}")
        self.attach_rizin()
        self.cmd_info()
        print("\nAvailable QIR Commands:")
        print("  1. disasm   - View full Quantum Assembly disassembly listing")
        print("  2. circuit  - View 2D ASCII Quantum Circuit diagram")
        print("  3. deobf    - Run Quantum Obfuscation Detector & Deobfuscator")
        print("  4. export   - Export canonical OpenQASM 3.0")
        print("  5. rz       - Drop into raw Rizin shell")
        print("  q. quit     - Exit\n")

        try:
            while True:
                try:
                    cmd = input("rizin-qir> ").strip().lower()
                except (EOFError, KeyboardInterrupt):
                    break

                if cmd in ('q', 'exit', 'quit'):
                    break
                elif cmd in ('1', 'disasm', 'qir.disasm'):
                    self.cmd_disasm()
                elif cmd in ('2', 'circuit', 'qir.circuit'):
                    self.cmd_circuit()
                elif cmd in ('3', 'deobf', 'qir.deobf'):
                    self.cmd_deobf()
                elif cmd in ('4', 'export', 'qir.export'):
                    self.cmd_export()
                elif cmd in ('info', 'qir'):
                    self.cmd_info()
                elif cmd in ('5', 'rz'):
                    print("[*] Entering interactive Rizin core. Type 'exit' to return to QIR prompt.")
                    while True:
                        rz_cmd = input(f"[{self.target_file}]> ").strip()
                        if rz_cmd in ('exit', 'q'):
                            break
                        if rz_cmd:
                            res = self.rz.cmd(rz_cmd)
                            if res:
                                print(res)
                else:
                    print(f"Unknown command: '{cmd}'. Try: disasm, circuit, deobf, export, rz, exit")
        finally:
            if self.rz is not None:
                try:
                    self.rz.quit()
                except Exception:
                    pass


if __name__ == "__main__":
    if len(sys.argv) > 1:
        target = sys.argv[1]
        plugin = RzQIRPlugin(target)
        if len(sys.argv) > 2 and sys.argv[2] == "--deobf":
            plugin.cmd_deobf()
        elif len(sys.argv) > 2 and sys.argv[2] == "--disasm":
            plugin.cmd_disasm()
        elif len(sys.argv) > 2 and sys.argv[2] == "--circuit":
            plugin.cmd_circuit()
        elif len(sys.argv) > 2 and sys.argv[2] == "--export":
            plugin.cmd_export()
        else:
            plugin.run_interactive()
    else:
        print("Usage: python3 rz_qir_plugin.py <path_to_bc_or_ll> [--disasm | --circuit | --deobf | --export]")



def create_cutter_plugin():
    return None
