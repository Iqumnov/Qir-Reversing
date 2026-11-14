#!/usr/bin/env python3
"""
Standalone Graphical User Interface (GUI) for QIR Reverse Engineering Suite
Student: Igumnov (Group 231-351)
Course: Reverse Engineering of Quantum Bytecode

Interactive Desktop Application:
- LLVM QIR Bitcode (.bc) and Textual IR (.ll) Loader
- Quantum Disassembly Viewer with semantic annotations
- Interactive 2D Quantum Circuit Matrix Diagram
- Quantum Anti-Obfuscation Analyzer & Automated Circuit Optimizer
- OpenQASM 3.0 Exporter
"""

import os
import sys
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

# Adjust path for imports
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(BASE_DIR, "qir_plugin"))

from qir_parser import QIRParser
from qir_disasm import QIRDisassembler
from qir_deobfuscator import QIRDeobfuscator


class QIRApplication(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("QIR Quantum Reverse Engineering Suite // Igumnov (231-351)")
        self.geometry("1120x720")
        self.minsize(900, 600)

        self.parser = QIRParser()
        self.disasm = QIRDisassembler(self.parser)
        self.deobf = QIRDeobfuscator()

        self.current_filepath = ""
        self.raw_instructions = []
        self.cleaned_instructions = []

        self._apply_dark_theme()
        self._build_layout()

        # Load clean demo by default if available
        default_demo = os.path.join(BASE_DIR, "demo_programs", "lr2_quantum_demo.bc")
        if os.path.exists(default_demo):
            self.load_file(default_demo)

    def _apply_dark_theme(self):
        self.configure(bg="#000000")
        self.style = ttk.Style(self)
        self.style.theme_use("clam")

        # Monochrome Tactical Palette
        self.c_bg = "#000000"
        self.c_panel = "#050505"
        self.c_card = "#0c0c0c"
        self.c_border = "#202020"
        self.c_border_bright = "#333333"
        self.c_fg = "#ebebeb"
        self.c_fg_dim = "#828282"
        self.c_fg_bright = "#ffffff"

        self.style.configure(".", background=self.c_bg, foreground=self.c_fg)
        self.style.configure("TFrame", background=self.c_bg)
        self.style.configure("Panel.TFrame", background=self.c_panel)
        self.style.configure("TLabel", background=self.c_bg, foreground=self.c_fg, font=("Hack", 9))
        self.style.configure("Header.TLabel", background=self.c_panel, foreground=self.c_fg_bright, font=("Hack Bold", 11))
        self.style.configure("Sub.TLabel", background=self.c_panel, foreground=self.c_fg_dim, font=("Hack", 8))

        # Monochrome Tactical Buttons
        self.style.configure("Action.TButton", background=self.c_card, foreground=self.c_fg,
                             borderwidth=1, relief="solid", bordercolor=self.c_border_bright,
                             font=("Hack Bold", 8), padding=(10, 6))
        self.style.map("Action.TButton",
                       background=[("active", "#1a1a1a"), ("pressed", "#282828")],
                       foreground=[("active", self.c_fg_bright)])

        self.style.configure("Primary.TButton", background="#141414", foreground=self.c_fg_bright,
                             borderwidth=1, relief="solid", bordercolor="#444444",
                             font=("Hack Bold", 8), padding=(12, 6))
        self.style.map("Primary.TButton",
                       background=[("active", "#222222"), ("pressed", "#333333")],
                       foreground=[("active", self.c_fg_bright), ("pressed", self.c_fg_bright)])

        # Notebook tabs (Monochrome Square Brackets)
        self.style.configure("TNotebook", background=self.c_bg, borderwidth=0)
        self.style.configure("TNotebook.Tab", background="#060606", foreground=self.c_fg_dim,
                             padding=[14, 6], font=("Hack Bold", 8), borderwidth=1, relief="solid")
        self.style.map("TNotebook.Tab",
                       background=[("selected", "#121212")],
                       foreground=[("selected", self.c_fg_bright)])

    def _build_layout(self):
        # Top Header Bar
        hdr_frame = ttk.Frame(self, style="Panel.TFrame", padding=(14, 10, 14, 8))
        hdr_frame.pack(fill=tk.X, side=tk.TOP)

        title = ttk.Label(hdr_frame, text="[ QIR QUANTUM BYTECODE DISASSEMBLER & DEOBFUSCATOR ]", style="Header.TLabel")
        title.pack(anchor=tk.W)

        sub = ttk.Label(hdr_frame, text="OPERATOR: IGUMNOV // GROUP 231-351 | LLVM QIR REVERSE ENGINEERING", style="Sub.TLabel")
        sub.pack(anchor=tk.W)

        # Action Toolbar
        tb_frame = ttk.Frame(self, style="Panel.TFrame", padding=(14, 4, 14, 10))
        tb_frame.pack(fill=tk.X, side=tk.TOP)

        btn_open = ttk.Button(tb_frame, text="[ LOAD QIR ]", style="Action.TButton", command=self.on_open_file)
        btn_open.pack(side=tk.LEFT, padx=(0, 6))

        btn_demo1 = ttk.Button(tb_frame, text="[ CLEAN DEMO ]", style="Action.TButton", command=self.on_load_clean_demo)
        btn_demo1.pack(side=tk.LEFT, padx=6)

        btn_demo2 = ttk.Button(tb_frame, text="[ OBFUSCATED DEMO ]", style="Action.TButton", command=self.on_load_obfuscated_demo)
        btn_demo2.pack(side=tk.LEFT, padx=6)

        btn_deobf = ttk.Button(tb_frame, text="[ DEOBFUSCATE CIRCUIT ]", style="Primary.TButton", command=self.on_run_deobfuscator)
        btn_deobf.pack(side=tk.LEFT, padx=14)

        btn_qasm = ttk.Button(tb_frame, text="[ EXPORT OPENQASM 3.0 ]", style="Action.TButton", command=self.on_export_qasm)
        btn_qasm.pack(side=tk.RIGHT, padx=0)

        # Status telemetry strip
        self.status_bar = ttk.Frame(self, style="Panel.TFrame", padding=(14, 6))
        self.status_bar.pack(fill=tk.X, side=tk.TOP)

        self.lbl_telemetry = ttk.Label(self.status_bar, text="STATUS: IDLE | TARGET: NONE | QUBITS: 0 | RESULTS: 0 | OPCODES: 0",
                                       style="Sub.TLabel", font=("Hack Bold", 8))
        self.lbl_telemetry.pack(side=tk.LEFT)

        self.lbl_obf_index = ttk.Label(self.status_bar, text="OBFUSCATION INDEX: 0.0% [CLEAN]",
                                       style="Sub.TLabel", font=("Hack Bold", 8))
        self.lbl_obf_index.pack(side=tk.RIGHT)

        # Main Tabbed Workspace
        self.notebook = ttk.Notebook(self)
        self.notebook.pack(fill=tk.BOTH, expand=True, padx=10, pady=(6, 10))

        # Tab 1: Disassembly
        self.txt_disasm = self._create_text_editor(self.notebook, "[ 01 // DISASSEMBLY (QASM) ]")

        # Tab 2: Quantum Circuit 2D Diagram
        self.txt_circuit = self._create_text_editor(self.notebook, "[ 02 // QUANTUM CIRCUIT ]")

        # Tab 3: Deobfuscator Audit Report
        self.txt_report = self._create_text_editor(self.notebook, "[ 03 // DEOBFUSCATION AUDIT ]")

        # Tab 4: OpenQASM 3.0 Export Preview
        self.txt_qasm = self._create_text_editor(self.notebook, "[ 04 // OPENQASM 3.0 ]")

    def _create_text_editor(self, parent, tab_title):
        frame = ttk.Frame(parent, style="Panel.TFrame", padding=4)
        parent.add(frame, text=tab_title)

        txt = tk.Text(frame, bg="#000000", fg="#ebebeb", insertbackground="#ffffff",
                      selectbackground="#282828", selectforeground="#ffffff",
                      font=("Hack", 10), borderwidth=1, relief=tk.SOLID, wrap=tk.NONE,
                      padx=8, pady=8)
        scroll_y = ttk.Scrollbar(frame, orient=tk.VERTICAL, command=txt.yview)
        scroll_x = ttk.Scrollbar(frame, orient=tk.HORIZONTAL, command=txt.xview)
        txt.configure(xscrollcommand=scroll_x.set, yscrollcommand=scroll_y.set)

        scroll_y.pack(side=tk.RIGHT, fill=tk.Y)
        scroll_x.pack(side=tk.BOTTOM, fill=tk.X)
        txt.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        return txt

    def load_file(self, filepath):
        if not os.path.exists(filepath):
            return
        self.current_filepath = filepath
        self.raw_instructions = self.parser.parse_file(filepath)
        self.deobf.analyze(self.raw_instructions)

        # Update telemetry
        fname = os.path.basename(filepath)
        self.lbl_telemetry.config(
            text=f"Target: {fname} | Qubits: {self.parser.num_qubits} | Results: {self.parser.num_results} | Opcodes: {len(self.raw_instructions)}"
        )
        obf_text = f"OBFUSCATION INDEX: {self.deobf.obfuscation_ratio:.1f}% [!] REDUNDANCY DETECTED" if self.deobf.obfuscation_ratio > 0 else "OBFUSCATION INDEX: 0.0% [OK // CANONICAL]"
        self.lbl_obf_index.config(
            text=obf_text,
            foreground="#ffffff" if self.deobf.obfuscation_ratio > 0 else "#828282"
        )

        # Populate Tabs
        # 1. Disassembly
        dis_output = self.disasm.format_disassembly(self.raw_instructions, filepath)
        self._set_text(self.txt_disasm, dis_output)

        # 2. Circuit Diagram
        circ_output = self.disasm.render_circuit_diagram(self.raw_instructions)
        self._set_text(self.txt_circuit, circ_output)

        # 3. Deobfuscation Report
        rep_output = self.deobf.format_report()
        self._set_text(self.txt_report, rep_output)

        # 4. OpenQASM
        qasm_output = self.disasm.export_openqasm(self.raw_instructions)
        self._set_text(self.txt_qasm, qasm_output)

    def _set_text(self, widget, text):
        widget.delete("1.0", tk.END)
        widget.insert(tk.END, text)

    def on_open_file(self):
        f = filedialog.askopenfilename(
            title="Open Quantum Intermediate Representation File",
            filetypes=[("QIR Files (*.bc, *.ll)", "*.bc *.ll"), ("All Files", "*.*")]
        )
        if f:
            self.load_file(f)

    def on_load_clean_demo(self):
        p = os.path.join(BASE_DIR, "demo_programs", "lr2_quantum_demo.bc")
        self.load_file(p)

    def on_load_obfuscated_demo(self):
        p = os.path.join(BASE_DIR, "demo_programs", "lr2_obfuscated.bc")
        self.load_file(p)

    def on_run_deobfuscator(self):
        if not self.raw_instructions:
            messagebox.showinfo("Deobfuscator", "Please open a QIR bytecode file first.")
            return

        self.cleaned_instructions = self.deobf.deobfuscate(self.raw_instructions)

        # Show results
        rep = self.deobf.format_report()
        clean_dis = self.disasm.format_disassembly(self.cleaned_instructions, self.current_filepath + " [DEOBFUSCATED]")
        clean_circ = self.disasm.render_circuit_diagram(self.cleaned_instructions)

        full_rep = f"{rep}\n\n[+] CANONICAL DEOBFUSCATED INSTRUCTIONS ({len(self.cleaned_instructions)} gates remaining of {len(self.raw_instructions)}):\n{clean_dis}\n\n{clean_circ}"
        self._set_text(self.txt_report, full_rep)

        # Update QASM tab with clean code
        clean_qasm = self.disasm.export_openqasm(self.cleaned_instructions)
        self._set_text(self.txt_qasm, clean_qasm)

        # Switch to Report tab
        self.notebook.select(2)
        messagebox.showinfo("Deobfuscation Complete",
                            f"Identified {len(self.deobf.findings)} obfuscation artifacts.\n"
                            f"Reduced circuit from {len(self.raw_instructions)} to {len(self.cleaned_instructions)} canonical operations.\n"
                            f"Obfuscation ratio: {self.deobf.obfuscation_ratio:.1f}%")

    def on_export_qasm(self):
        insts = self.cleaned_instructions or self.raw_instructions
        if not insts:
            messagebox.showinfo("Export", "No quantum instructions loaded.")
            return

        f = filedialog.asksaveasfilename(
            title="Export Decompiled OpenQASM 3.0",
            defaultextension=".qasm",
            filetypes=[("OpenQASM (*.qasm)", "*.qasm"), ("All Files", "*.*")]
        )
        if f:
            self.disasm.export_openqasm(insts, out_path=f)
            messagebox.showinfo("Export Success", f"Successfully exported OpenQASM 3.0 to:\n{f}")


if __name__ == "__main__":
    app = QIRApplication()
    app.mainloop()
