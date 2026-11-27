#!/usr/bin/env python3
"""
Standalone Graphical User Interface (GUI) for QIR Reverse Engineering Suite
Author: Matvei Igumnov <matvei@iqumnov.com>

Interactive Desktop Application:
- LLVM QIR Bitcode (.bc) and Textual IR (.ll) Loader
- Quantum Disassembly Viewer with semantic annotations
- Interactive 2D Quantum Circuit Matrix Diagram
- Quantum Anti-Obfuscation Analyzer & Automated Circuit Optimizer
- OpenQASM 3.0 Exporter
"""

import os
import sys
import webbrowser
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

if __package__:
    from .qir_parser import QIRParser
    from .qir_disasm import QIRDisassembler
    from .qir_deobfuscator import QIRDeobfuscator
else:
    BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    sys.path.insert(0, os.path.join(BASE_DIR, "qir_plugin"))
    from qir_parser import QIRParser
    from qir_disasm import QIRDisassembler
    from qir_deobfuscator import QIRDeobfuscator


class QIRApplication(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("QIR Reversing")
        self.geometry("1120x720")
        self.minsize(900, 600)

        self.parser = QIRParser()
        self.disasm = QIRDisassembler(self.parser)
        self.deobf = QIRDeobfuscator()

        self.current_filepath = ""
        self.raw_instructions = []
        self.cleaned_instructions = None

        self._apply_dark_theme()
        self._build_layout()
        self.lift()

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
        self.style.configure("Icon.TButton", background=self.c_card, foreground=self.c_fg,
                             borderwidth=1, relief="solid", bordercolor=self.c_border_bright,
                             font=("Hack Bold", 8), padding=(5, 6))
        self.style.map("Icon.TButton",
                       background=[("active", "#1a1a1a"), ("pressed", "#282828")],
                       foreground=[("active", self.c_fg_bright)])

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
        # Top Unified Navigation & Action Header (Controls on left, Divider, Tabs on right)
        self.header_frame = ttk.Frame(self, style="Panel.TFrame", padding=(10, 6, 10, 6))
        self.header_frame.pack(fill=tk.X, side=tk.TOP)

        # Actions Group on the Left: Info, Load QIR, Analyze, Export OpenQASM
        actions_frame = ttk.Frame(self.header_frame, style="Panel.TFrame")
        actions_frame.pack(side=tk.LEFT)

        self.btn_info = ttk.Button(actions_frame, text="i", width=2, style="Icon.TButton", command=self.on_show_info)
        self.btn_info.pack(side=tk.LEFT, padx=(0, 6))

        self.btn_load = ttk.Button(actions_frame, text="Load QIR", style="Action.TButton", command=self.on_open_file)
        self.btn_load.pack(side=tk.LEFT, padx=(0, 6))

        self.btn_analyze = ttk.Button(actions_frame, text="Analyze", style="Action.TButton", command=self.on_analyze_source, state=tk.DISABLED)
        self.btn_analyze.pack(side=tk.LEFT, padx=(0, 6))

        self.btn_qasm = ttk.Button(actions_frame, text="Export OpenQASM", style="Action.TButton", command=self.on_export_qasm, state=tk.DISABLED)
        self.btn_qasm.pack(side=tk.LEFT, padx=(0, 10))

        # Vertical Divider
        lbl_div = ttk.Label(self.header_frame, text="|", foreground=self.c_border_bright, font=("Hack", 10))
        lbl_div.pack(side=tk.LEFT, padx=(0, 10))

        # Tactical Tabs on the Right
        self.tabs_frame = ttk.Frame(self.header_frame, style="Panel.TFrame")
        self.tabs_frame.pack(side=tk.LEFT)

        self.tab_names = ["Source", "Disassembly", "Circuit", "Deobfuscation", "OpenQASM"]
        self.tab_buttons = []
        for i, name in enumerate(self.tab_names):
            btn = tk.Button(
                self.tabs_frame,
                text=name,
                font=("Hack Bold", 8),
                bg="#050505",
                fg=self.c_fg_dim if i == 0 else "#404040",
                activebackground="#141414",
                activeforeground="#ffffff",
                relief=tk.FLAT,
                bd=1,
                padx=12,
                pady=4,
                cursor="hand2" if i == 0 else "arrow",
                command=lambda idx=i: self.select_tab(idx)
            )
            btn.pack(side=tk.LEFT, padx=(0, 2))
            self.tab_buttons.append(btn)

        # Status telemetry strip (hidden initially until file/sample is loaded)
        self.status_bar = ttk.Frame(self, style="Panel.TFrame", padding=(14, 5))
        # Not packed initially - packed dynamically on analysis

        self.lbl_telemetry = ttk.Label(self.status_bar, text="Status: Ready | Target: None | Qubits: 0 | Results: 0 | Opcodes: 0",
                                       style="Sub.TLabel", font=("Hack Bold", 8))
        self.lbl_telemetry.pack(side=tk.LEFT)

        self.lbl_obf_index = ttk.Label(self.status_bar, text="Reduction: 0.0%",
                                       style="Sub.TLabel", font=("Hack Bold", 8))
        self.lbl_obf_index.pack(side=tk.RIGHT)

        # Main Workspace Container (Full height, 0 wasted rows)
        self.workspace_frame = ttk.Frame(self, style="TFrame")
        self.workspace_frame.pack(fill=tk.BOTH, expand=True, padx=6, pady=(4, 6))

        # View Frames
        self.tab_views = []
        
        # Tab 0: Source
        frame_source, self.txt_source = self._create_editor_view(self.workspace_frame)
        self.placeholder_text = "; Paste LLVM QIR assembly here or click Load QIR..."
        self._set_placeholder()
        self.txt_source.bind("<FocusIn>", self._on_source_focus_in)
        self.txt_source.bind("<FocusOut>", self._on_source_focus_out)
        self.txt_source.bind("<KeyRelease>", self._update_analyze_state)
        self.txt_source.bind("<<Modified>>", self._on_source_modified)
        self.tab_views.append(frame_source)

        # Tab 1: Disassembly
        frame_disasm, self.txt_disasm = self._create_editor_view(self.workspace_frame)
        self._set_text(self.txt_disasm, "; No QIR bytecode loaded. Paste source or load a file to analyze.")
        self.tab_views.append(frame_disasm)

        # Tab 2: Quantum Circuit
        frame_circuit, self.txt_circuit = self._create_editor_view(self.workspace_frame)
        self._set_text(self.txt_circuit, "; No circuit diagram to display.")
        self.tab_views.append(frame_circuit)

        # Tab 3: Deobfuscation
        frame_report, self.txt_report = self._create_editor_view(self.workspace_frame)
        self._set_text(self.txt_report, "; No deobfuscation audit performed yet.")
        self.tab_views.append(frame_report)

        # Tab 4: OpenQASM
        frame_qasm, self.txt_qasm = self._create_editor_view(self.workspace_frame)
        self._set_text(self.txt_qasm, "; OpenQASM 3.0 output will render here after analysis...")
        self.tab_views.append(frame_qasm)

        # Select Tab 0 initially
        self.current_tab_idx = -1
        self.has_results = False
        self.select_tab(0)

    def select_tab(self, index):
        if not self.has_results and index != 0:
            return
        if self.current_tab_idx == index:
            return
        # Hide previous
        if self.current_tab_idx >= 0 and self.current_tab_idx < len(self.tab_views):
            self.tab_views[self.current_tab_idx].pack_forget()
            self.tab_buttons[self.current_tab_idx].configure(
                bg="#050505", fg=self.c_fg_dim if self.has_results or self.current_tab_idx == 0 else "#404040", relief=tk.FLAT, bd=1
            )
        # Show new
        self.current_tab_idx = index
        self.tab_views[index].pack(fill=tk.BOTH, expand=True)
        self.tab_buttons[index].configure(
            bg="#000000", fg="#ffffff", relief=tk.SOLID, bd=1
        )

    def _create_editor_view(self, parent):
        frame = ttk.Frame(parent, style="Panel.TFrame")
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
        return frame, txt

    def load_file(self, filepath, display_name=None):
        if not os.path.exists(filepath):
            return
        self.current_filepath = display_name or filepath
        try:
            self.raw_instructions = self.parser.parse_file(filepath)
        except Exception as err:
            messagebox.showerror("QIR Parse Error", f"Failed to parse {os.path.basename(self.current_filepath)}:\n{err}")
            return
        if not self.raw_instructions:
            # Verify raw text or decoded bitcode IR actually looks like LLVM IR / QIR before presenting empty 0-op analysis
            looks_like_qir = False
            try:
                txt_probe = self.parser.last_decoded_ir or ""
                if not txt_probe and (not display_name or not display_name.lower().endswith(".bc")):
                    with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
                        txt_probe = f.read()
                looks_like_qir = any(
                    kw in txt_probe
                    for kw in ("define ", "declare ", "@__quantum__", "attributes ")
                )
            except Exception:
                pass
            if not looks_like_qir:
                messagebox.showwarning(
                    "No QIR Found",
                    f"No valid LLVM QIR instructions or declarations found in '{os.path.basename(self.current_filepath)}'."
                )
                return
        self.cleaned_instructions = self.deobf.deobfuscate(self.raw_instructions)

        # Show status bar if hidden
        if not self.status_bar.winfo_ismapped():
            self.status_bar.pack(fill=tk.X, side=tk.TOP, after=self.header_frame)

        # Enable results state and export button
        self.has_results = True
        self.is_placeholder_active = False
        self.txt_source.configure(fg="#ebebeb")
        self.btn_qasm.config(state=tk.NORMAL)
        for btn in self.tab_buttons:
            btn.config(cursor="hand2", fg=self.c_fg_dim)

        # Update telemetry (count quantum operations excluding control-flow labels)
        n_raw = sum(1 for i in self.raw_instructions if not i.is_control_flow)
        n_clean = sum(1 for i in self.cleaned_instructions if not i.is_control_flow)
        fname = display_name or os.path.basename(filepath)
        self.lbl_telemetry.config(
            text=f"Status: Analyzed | Target: {fname} | Qubits: {self.parser.num_qubits} | Results: {self.parser.num_results} | Opcodes: {n_raw} -> {n_clean}"
        )
        obf_text = f"Reduction: {self.deobf.obfuscation_ratio:.1f}%" if self.deobf.obfuscation_ratio > 0 else "Reduction: 0.0%"
        self.lbl_obf_index.config(
            text=obf_text,
            foreground="#ffffff" if self.deobf.obfuscation_ratio > 0 else "#828282"
        )

        # Populate Tabs
        # 0. Source (Handle binary .bc vs text .ll without dumping unprintable characters)
        source_text = ""
        is_bitcode = False
        try:
            with open(filepath, "rb") as bf:
                hdr = bf.read(4)
                if hdr in (b"\x42\x43\xc0\xde", b"\xde\xc0\x17\x0b") or filepath.lower().endswith(".bc"):
                    is_bitcode = True
        except Exception:
            pass

        if is_bitcode:
            if self.parser.last_decoded_ir and self.parser.last_decoded_ir.strip():
                source_text = self.parser.last_decoded_ir
            else:
                # Fallback textual reconstruction from decoded instructions
                lines = [
                    f"; ModuleID = '{fname}'",
                    f"; Decompiled from LLVM Bitcode ({n_raw} operations, {self.parser.num_qubits} qubits)",
                    "",
                    f"define void @{self.parser.entry_point or 'main'}() #0 {{",
                    "entry:"
                ]
                for inst in self.raw_instructions:
                    if inst.is_control_flow and inst.label == "entry":
                        continue
                    if inst.raw_call:
                        lines.append(f"  {inst.raw_call}")
                    else:
                        lines.append(f"  ; {inst.mnemonic()}")
                lines.extend([
                    "  ret void",
                    "}",
                    "",
                    f'attributes #0 = {{ "required_num_qubits"="{self.parser.num_qubits}" "required_num_results"="{self.parser.num_results}" }}'
                ])
                source_text = "\n".join(lines)
        else:
            try:
                with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
                    source_text = f.read()
            except Exception:
                pass

        self._set_text(self.txt_source, source_text)
        self._update_analyze_state()

        # 1. Disassembly
        dis_output = self.disasm.format_disassembly(self.raw_instructions, fname)
        self._set_text(self.txt_disasm, dis_output)

        # 2. Circuit Diagram
        display_insts = self.cleaned_instructions if self.cleaned_instructions is not None else self.raw_instructions
        circ_output = self.disasm.render_circuit_diagram(display_insts)
        self._set_text(self.txt_circuit, circ_output)

        # 3. Deobfuscation Report
        rep = self.deobf.format_report()
        clean_dis = self.disasm.format_disassembly(self.cleaned_instructions, fname + " [DEOBFUSCATED]")
        full_rep = f"{rep}\n\n[+] CANONICAL RECONSTRUCTED INSTRUCTIONS ({n_clean} of {n_raw}):\n{clean_dis}"
        self._set_text(self.txt_report, full_rep)

        # 4. OpenQASM
        clean_qasm = self.disasm.export_openqasm(display_insts)
        self._set_text(self.txt_qasm, clean_qasm)

        self.select_tab(1)

    def _update_analyze_state(self, event=None):
        if getattr(self, "is_placeholder_active", False):
            self.btn_analyze.config(state=tk.DISABLED)
            return
        content = self.txt_source.get("1.0", tk.END).strip()
        if not content or content == self.placeholder_text:
            self.btn_analyze.config(state=tk.DISABLED)
            return
        non_comment_lines = [line.strip() for line in content.splitlines() if line.strip() and not line.strip().startswith(";")]
        if not non_comment_lines:
            self.btn_analyze.config(state=tk.DISABLED)
        else:
            self.btn_analyze.config(state=tk.NORMAL)

    def _on_source_modified(self, event=None):
        self._update_analyze_state()
        try:
            self.txt_source.edit_modified(False)
        except Exception:
            pass

    def _set_placeholder(self):
        self.txt_source.delete("1.0", tk.END)
        self.txt_source.insert(tk.END, self.placeholder_text)
        self.txt_source.configure(fg="#444444")
        self.is_placeholder_active = True
        self._update_analyze_state()

    def _on_source_focus_in(self, event):
        if getattr(self, "is_placeholder_active", False):
            self.txt_source.delete("1.0", tk.END)
            self.txt_source.configure(fg="#ebebeb")
            self.is_placeholder_active = False
            self._update_analyze_state()

    def _on_source_focus_out(self, event):
        content = self.txt_source.get("1.0", tk.END).strip()
        if not content:
            self._set_placeholder()
        else:
            self._update_analyze_state()

    def _set_text(self, widget, text):
        widget.delete("1.0", tk.END)
        widget.insert(tk.END, text)

    def on_show_info(self):
        dialog = tk.Toplevel(self)
        dialog.title("QIR Reversing // Platform Overview & Guide")
        dialog.geometry("620x520")
        dialog.configure(bg="#000000")
        dialog.transient(self)
        dialog.grab_set()

        # Center dialog relative to self
        self.update_idletasks()
        x = self.winfo_x() + (self.winfo_width() - 620) // 2
        y = self.winfo_y() + (self.winfo_height() - 520) // 2
        dialog.geometry(f"+{max(0, x)}+{max(0, y)}")

        # Container with border
        card = tk.Frame(dialog, bg="#000000", highlightthickness=1, highlightbackground="#333333")
        card.pack(fill=tk.BOTH, expand=True, padx=12, pady=12)

        # Header Frame
        hdr = tk.Frame(card, bg="#050505", highlightthickness=1, highlightbackground="#202020", padx=14, pady=10)
        hdr.pack(fill=tk.X, side=tk.TOP)

        title_frame = tk.Frame(hdr, bg="#050505")
        title_frame.pack(side=tk.LEFT, fill=tk.X, expand=True)

        lbl_title = tk.Label(title_frame, text="QIR Reversing", font=("Hack Bold", 11), fg="#ffffff", bg="#050505")
        lbl_title.pack(anchor="w")
        lbl_sub = tk.Label(title_frame, text="Quantum Reverse Engineering Platform", font=("Hack", 8), fg="#828282", bg="#050505")
        lbl_sub.pack(anchor="w")

        # Footer Frame (Pack at bottom FIRST so it is always pinned and never clipped)
        ftr = tk.Frame(card, bg="#050505", highlightthickness=1, highlightbackground="#202020", padx=14, pady=10)
        ftr.pack(fill=tk.X, side=tk.BOTTOM)

        def open_github():
            webbrowser.open_new_tab("https://github.com/Iqumnov/Qir-Reversing")

        btn_gh = tk.Button(ftr, text="GitHub & Full Docs ↗", font=("Hack Bold", 8),
                           bg="#0c0c0c", fg="#ebebeb", activebackground="#1a1a1a", activeforeground="#ffffff",
                           relief=tk.FLAT, bd=1, highlightthickness=1, highlightbackground="#333333",
                           padx=12, pady=5, cursor="hand2", command=open_github)
        btn_gh.pack(side=tk.LEFT)

        btn_ok = tk.Button(ftr, text="OK", font=("Hack Bold", 8),
                           bg="#141414", fg="#ffffff", activebackground="#222222", activeforeground="#ffffff",
                           relief=tk.FLAT, bd=1, highlightthickness=1, highlightbackground="#444444",
                           padx=16, pady=5, cursor="hand2", command=dialog.destroy)
        btn_ok.pack(side=tk.RIGHT)

        # Body Text / Sections (Occupies middle remaining area)
        body_frame = tk.Frame(card, bg="#000000", padx=16, pady=14)
        body_frame.pack(fill=tk.BOTH, expand=True)

        txt_info = tk.Text(body_frame, bg="#000000", fg="#ebebeb", font=("Hack", 9),
                           relief=tk.FLAT, wrap=tk.WORD, padx=4, pady=4,
                           selectbackground="#333333", selectforeground="#ffffff",
                           highlightthickness=0)
        txt_info.pack(fill=tk.BOTH, expand=True)

        info_content = (
            "PURPOSE\n"
            "------------------------------------------------------------------------\n"
            "qir-reversing is a static analysis and reverse engineering platform\n"
            "for quantum software compiled to LLVM Quantum Intermediate Representation\n"
            "(binary bitcode .bc and textual IR .ll). It decompiles quantum circuits,\n"
            "detects anti-analysis obfuscation, and formally proves functional\n"
            "equivalence.\n\n"
            "WORKFLOW\n"
            "------------------------------------------------------------------------\n"
            "1. Load:    Click 'Load QIR' to open a bitcode (.bc) or IR (.ll) file,\n"
            "            or paste assembly directly into the Source editor.\n"
            "2. Analyze: Click 'Analyze' to decompile instructions, detect\n"
            "            obfuscation patterns, and reconstruct canonical OpenQASM.\n"
            "            (For Dense 2^N / QMDD proofs up to 64q, use 'qir-rev verify' or Web UI.)\n"
            "3. Inspect: Examine findings across the Disassembly, Circuit,\n"
            "            Deobfuscation, and OpenQASM viewports.\n\n"
            "INTERFACE MODES\n"
            "------------------------------------------------------------------------\n"
            "• Source:        Raw QIR assembly editor and bitcode file drop zone.\n"
            "• Disassembly:   Linear instruction stream with decoded qubits and angles.\n"
            "• Circuit:       Interactive topological schematic of quantum wires & gates.\n"
            "• Deobfuscation: Structural audit report and minimal canonical circuit.\n"
            "• OpenQASM:      Decompiled OpenQASM 3.0 code ready for quantum backends.\n\n"
            "LOCAL INSTALLATION & TOOLCHAIN\n"
            "------------------------------------------------------------------------\n"
            "cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp\n\n"
            "• qir-rev:     High-throughput local CLI reversing & batch pipelines (alias: qrev).\n"
            "• qir-rev-mcp: Model Context Protocol server for AI assistant integration.\n"
            "• Cutter Dock: Native C-ABI plugin (librz_qir.so) & GUI dock for Rizin.\n"
            "• qir-gui:     Standalone offline desktop application.\n\n"
            "DOCUMENTATION & SPECIFICATIONS\n"
            "------------------------------------------------------------------------\n"
            "For full technical documentation, CLI toolchain usage, MCP server\n"
            "integration, and formal verification proofs, visit the GitHub repository.\n"
        )
        txt_info.insert("1.0", info_content)
        txt_info.configure(state=tk.DISABLED)

        dialog.bind("<Escape>", lambda e: dialog.destroy())

    def on_open_file(self):
        f = filedialog.askopenfilename(
            title="Open Quantum Intermediate Representation File",
            filetypes=[("QIR Files (*.bc, *.ll)", "*.bc *.ll"), ("All Files", "*.*")]
        )
        if f:
            self.load_file(f)

    def on_analyze_source(self):
        if getattr(self, "is_placeholder_active", False):
            messagebox.showinfo("Analyze", "No quantum instructions to analyze. Please click Load QIR or paste LLVM assembly code.")
            return

        content = self.txt_source.get("1.0", tk.END).strip()
        if not content or content == self.placeholder_text:
            messagebox.showinfo("Analyze", "No quantum instructions to analyze. Please click Load QIR or paste LLVM assembly code.")
            return

        non_comment_lines = [line.strip() for line in content.splitlines() if line.strip() and not line.strip().startswith(";")]
        if not non_comment_lines:
            messagebox.showinfo("Analyze", "Source buffer contains only comments. Please provide valid LLVM QIR instructions.")
            return

        import tempfile
        tmp_path = None
        try:
            with tempfile.NamedTemporaryFile(mode="w", suffix=".ll", encoding="utf-8", delete=False) as tf:
                tf.write(content)
                tmp_path = tf.name
            self.load_file(tmp_path, display_name="source_buffer.ll")
        finally:
            if tmp_path and os.path.exists(tmp_path):
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass

    def on_export_qasm(self):
        insts = self.cleaned_instructions if self.cleaned_instructions is not None else self.raw_instructions
        if insts is None or (not insts and not self.raw_instructions):
            messagebox.showinfo("Export", "No quantum instructions loaded.")
            return

        f = filedialog.asksaveasfilename(
            title="Export Decompiled OpenQASM 3.0",
            defaultextension=".qasm",
            filetypes=[("OpenQASM (*.qasm)", "*.qasm"), ("All Files", "*.*")]
        )
        if f:
            try:
                self.disasm.export_openqasm(insts, out_path=f)
                messagebox.showinfo("Export Success", f"Successfully exported OpenQASM 3.0 to:\n{f}")
            except OSError as err:
                messagebox.showerror("Export Error", f"Failed to write OpenQASM file:\n{err}")


def main():
    app = QIRApplication()
    if len(sys.argv) > 1 and os.path.isfile(sys.argv[1]):
        initial_path = sys.argv[1]
        app.after(50, lambda p=initial_path: app.load_file(p))
    app.mainloop()


if __name__ == "__main__":
    main()
