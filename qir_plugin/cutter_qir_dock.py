"""
Cutter GUI Quantum Reversing Dock Plugin (QIR Inspector & Deobfuscator)
Author: Matvei Igumnov <matvei@iqumnov.com>

Integrates directly into Cutter (Qt5/Qt6 GUI for Rizin):
1. Dedicated "QIR Quantum Disassembler" dockable panel
2. Quantum Circuit Matrix Diagram visualizer
3. Anti-Obfuscation Audit & Clean Circuit Decompiler
"""

import sys
import os

CUTTER_AVAILABLE = False
try:
    import cutter  # type: ignore
    CUTTER_AVAILABLE = True
except ImportError:
    cutter = None

QT_AVAILABLE = False
QPalette = None
QColor = None
try:
    from PySide6.QtWidgets import (  # type: ignore
        QDockWidget, QWidget, QVBoxLayout, QHBoxLayout,
        QPushButton, QLabel, QTextEdit, QTabWidget, QFileDialog
    )
    from PySide6.QtGui import QFont, QPalette, QColor  # type: ignore
    from PySide6.QtCore import Qt  # type: ignore
    QT_AVAILABLE = True
except ImportError:
    try:
        from PySide2.QtWidgets import (  # type: ignore
            QDockWidget, QWidget, QVBoxLayout, QHBoxLayout,
            QPushButton, QLabel, QTextEdit, QTabWidget, QFileDialog
        )
        from PySide2.QtGui import QFont, QPalette, QColor  # type: ignore
        from PySide2.QtCore import Qt  # type: ignore
        QT_AVAILABLE = True
    except ImportError:
        QT_AVAILABLE = False

if __package__:
    from .qir_parser import QIRParser  # type: ignore
    from .qir_disasm import QIRDisassembler  # type: ignore
    from .qir_deobfuscator import QIRDeobfuscator  # type: ignore
else:
    _PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
    if _PLUGIN_DIR not in sys.path:
        sys.path.insert(0, _PLUGIN_DIR)
    from qir_parser import QIRParser  # type: ignore
    from qir_disasm import QIRDisassembler  # type: ignore
    from qir_deobfuscator import QIRDeobfuscator  # type: ignore


TACTICAL_QSS = """
QDockWidget {
    background-color: #000000;
    color: #ebebeb;
    font-family: 'Hack', monospace;
    font-size: 11px;
}
QDockWidget::title {
    background-color: #060606;
    color: #ffffff;
    border: 1px solid #202020;
    padding: 6px 10px;
    font-family: 'Hack', monospace;
    font-weight: bold;
    letter-spacing: 1px;
}
QWidget#QIRContainer {
    background-color: #000000;
    border: 1px solid #181818;
}
QLabel#HeaderTitle {
    color: #ffffff;
    font-family: 'Hack', monospace;
    font-size: 11px;
    font-weight: bold;
    letter-spacing: 1px;
    padding: 2px 0px 1px 0px;
}
QLabel#HeaderSub {
    color: #828282;
    font-family: 'Hack', monospace;
    font-size: 9px;
    letter-spacing: 0.5px;
    padding-bottom: 6px;
}
QPushButton {
    background-color: #0c0c0c;
    color: #ebebeb;
    border: 1px solid #2a2a2a;
    border-radius: 0px;
    padding: 6px 10px;
    font-family: 'Hack', monospace;
    font-size: 10px;
    font-weight: bold;
}
QPushButton:hover {
    background-color: #1a1a1a;
    border-color: #666666;
    color: #ffffff;
}
QPushButton:pressed {
    background-color: #282828;
    border-color: #888888;
    color: #ffffff;
}
QPushButton#PrimaryBtn {
    background-color: #141414;
    border: 1px solid #444444;
    color: #ffffff;
}
QPushButton#PrimaryBtn:hover {
    background-color: #222222;
    border-color: #888888;
    color: #ffffff;
}
QTabWidget::pane {
    border: 1px solid #202020;
    background-color: #000000;
    top: -1px;
}
QTabBar::tab {
    background-color: #060606;
    color: #828282;
    border: 1px solid #202020;
    border-bottom: none;
    padding: 7px 12px;
    font-family: 'Hack', monospace;
    font-size: 10px;
    font-weight: bold;
    border-radius: 0px;
    margin-right: 2px;
}
QTabBar::tab:selected {
    background-color: #121212;
    color: #ffffff;
    border-top: 2px solid #ffffff;
    border-bottom: 1px solid #121212;
}
QTabBar::tab:hover:!selected {
    color: #ebebeb;
    border-color: #444444;
}
QTextEdit {
    background-color: #000000;
    color: #ebebeb;
    border: 1px solid #181818;
    font-family: 'Hack', monospace;
    font-size: 11px;
    line-height: 1.4;
    selection-background-color: #282828;
    selection-color: #ffffff;
}
QScrollBar:vertical {
    background: #000000;
    width: 8px;
    border: none;
}
QScrollBar::handle:vertical {
    background: #1c1c1c;
    min-height: 20px;
}
QScrollBar::handle:vertical:hover {
    background: #444444;
}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0px;
}
QScrollBar:horizontal {
    background: #000000;
    height: 8px;
    border: none;
}
QScrollBar::handle:horizontal {
    background: #1c1c1c;
    min-width: 20px;
}
QScrollBar::handle:horizontal:hover {
    background: #444444;
}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {
    width: 0px;
}
"""


class QIRInspectorDock(QDockWidget if (CUTTER_AVAILABLE and QT_AVAILABLE) else object):
    """Interactive Cutter Dock for Quantum Bytecode Disassembly & Deobfuscation."""
    def __init__(self, parent=None):
        if not (CUTTER_AVAILABLE and QT_AVAILABLE):
            return
        super().__init__("QIR Reversing", parent)
        self.setObjectName("QIRInspectorDock")

        self.parser = QIRParser()
        self.disasm = QIRDisassembler(self.parser)
        self.deobf = QIRDeobfuscator()
        self.current_file = ""

        self._build_ui()
        self.setStyleSheet(TACTICAL_QSS)

    def _build_ui(self):
        container = QWidget()
        container.setObjectName("QIRContainer")
        layout = QVBoxLayout(container)
        layout.setContentsMargins(6, 6, 6, 6)
        layout.setSpacing(6)

        # Action Buttons Bar
        btn_bar = QHBoxLayout()
        btn_bar.setSpacing(4)
        self.btn_load = QPushButton("Load QIR")
        self.btn_load.clicked.connect(self.on_load_clicked)
        btn_bar.addWidget(self.btn_load)

        self.btn_disasm = QPushButton("Disassemble")
        self.btn_disasm.clicked.connect(self.on_disasm_clicked)
        btn_bar.addWidget(self.btn_disasm)

        self.btn_circuit = QPushButton("Circuit")
        self.btn_circuit.clicked.connect(self.on_circuit_clicked)
        btn_bar.addWidget(self.btn_circuit)

        self.btn_deobf = QPushButton("Deobfuscate")
        self.btn_deobf.setObjectName("PrimaryBtn")
        self.btn_deobf.clicked.connect(self.on_deobf_clicked)
        btn_bar.addWidget(self.btn_deobf)

        layout.addLayout(btn_bar)

        # Multi-tab view
        self.tabs = QTabWidget()

        font_mono = QFont("Hack", 10)
        font_mono.setStyleHint(QFont.Monospace)

        # Tab 1: Disassembly
        self.txt_disasm = QTextEdit()
        self.txt_disasm.setFont(font_mono)
        self.txt_disasm.setReadOnly(True)
        self.tabs.addTab(self.txt_disasm, "Disassembly")

        # Tab 2: Circuit
        self.txt_circuit = QTextEdit()
        self.txt_circuit.setFont(font_mono)
        self.txt_circuit.setReadOnly(True)
        self.tabs.addTab(self.txt_circuit, "Circuit")

        # Tab 3: Deobfuscation
        self.txt_deobf = QTextEdit()
        self.txt_deobf.setFont(font_mono)
        self.txt_deobf.setReadOnly(True)
        self.tabs.addTab(self.txt_deobf, "Deobfuscation")

        layout.addWidget(self.tabs)
        self.setWidget(container)

    def on_load_clicked(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self, "Select LLVM QIR Bytecode or Textual IR", "", "QIR Files (*.bc *.ll);;All Files (*)"
        )
        if file_path:
            self.load_qir_file(file_path)

    def load_qir_file(self, filepath):
        self.txt_circuit.setPlainText("; Click 'Circuit' to render timeline diagram for the loaded file.")
        self.txt_deobf.setPlainText("; Click 'Deobfuscate' to audit and simplify the loaded file.")
        try:
            insts = self.parser.parse_file(filepath)
            self.current_file = filepath
            out = self.disasm.format_disassembly(insts, filepath)
            self.txt_disasm.setPlainText(out)
            self.tabs.setCurrentIndex(0)
        except Exception as err:
            self.current_file = ""
            self.txt_disasm.setPlainText(f"; Failed to parse {os.path.basename(filepath)}: {err}")
            self.tabs.setCurrentIndex(0)

    def on_disasm_clicked(self):
        if not self.current_file:
            self.txt_disasm.setPlainText("; Please open a QIR bytecode file (.bc or .ll) first.")
            return
        try:
            if self.parser.instructions:
                out = self.disasm.format_disassembly(self.parser.instructions, self.current_file)
            else:
                out = self.disasm.disassemble_file(self.current_file)
            self.txt_disasm.setPlainText(out)
        except Exception as err:
            self.txt_disasm.setPlainText(f"; Disassembly error: {err}")
        self.tabs.setCurrentIndex(0)

    def on_circuit_clicked(self):
        if not self.current_file:
            self.txt_circuit.setPlainText("; No QIR bytecode loaded.")
            return
        try:
            cleaned = self.deobf.deobfuscate(self.parser.instructions)
            diagram = self.disasm.render_circuit_diagram(cleaned)
            self.txt_circuit.setPlainText(diagram)
        except Exception as err:
            self.txt_circuit.setPlainText(f"; Circuit render error: {err}")
        self.tabs.setCurrentIndex(1)

    def on_deobf_clicked(self):
        if not self.current_file:
            self.txt_deobf.setPlainText("; No QIR bytecode loaded.")
            return
        try:
            instructions = self.parser.instructions
            cleaned = self.deobf.deobfuscate(instructions)
            report = self.deobf.format_report()
            clean_dis = self.disasm.format_disassembly(cleaned, self.current_file + " [DEOBFUSCATED]")
            clean_circ = self.disasm.render_circuit_diagram(cleaned)

            full_output = f"{report}\n\n[+] CANONICAL DEOBFUSCATED ASSEMBLY:\n{clean_dis}\n\n{clean_circ}"
            self.txt_deobf.setPlainText(full_output)
        except Exception as err:
            self.txt_deobf.setPlainText(f"; Deobfuscation error: {err}")
        self.tabs.setCurrentIndex(2)


class QIRCutterPlugin(cutter.CutterPlugin if CUTTER_AVAILABLE else object):
    name = "QIR Quantum Disassembler & Deobfuscator"
    description = "LLVM Quantum Intermediate Representation plugin for Cutter"
    version = "1.0.0"
    author = "Matvei Igumnov"

    def getName(self):
        return self.name

    def getAuthor(self):
        return self.author

    def getDescription(self):
        return self.description

    def getVersion(self):
        return self.version

    def setupPlugin(self):
        print(f"[✓] {self.name} v{self.version} initialized in Cutter core.")

    def setupInterface(self, main):
        if not (CUTTER_AVAILABLE and QT_AVAILABLE):
            return
        dock = QIRInspectorDock(main)
        main.addDockWidget(Qt.RightDockWidgetArea, dock)
        dock.setFloating(False)
        dock.setVisible(True)
        dock.show()
        dock.raise_()
        print(f"[✓] Added QIR Inspector panel to Cutter interface.")

        # Ensure native menubar is disabled so the top menu bar is ALWAYS visible inside Cutter's window
        try:
            if hasattr(main, "menuBar") and main.menuBar():
                mb = main.menuBar()
                mb.setNativeMenuBar(False)
                mb.setVisible(True)
                mb.show()
                print(f"[✓] Ensured Cutter top menu bar is active and visible.")
        except Exception as e:
            print(f"[*] Menubar init notice: {e}")

        # Automatically load current file if already opened in Cutter or passed via arguments
        try:
            fn = ""
            if hasattr(cutter, "core"):
                c = cutter.core()
                if hasattr(c, "currentlyOpenFile"):
                    fn = c.currentlyOpenFile
                elif hasattr(c, "getFilename"):
                    fn = c.getFilename()
            if not fn and len(sys.argv) > 1:
                cand = sys.argv[-1]
                if cand.lower().endswith((".bc", ".ll")):
                    fn = cand
            if fn and os.path.exists(fn) and fn.lower().endswith((".bc", ".ll")):
                dock.load_qir_file(fn)
                print(f"[✓] Auto-loaded QIR file into Inspector: {fn}")
        except Exception as e:
            print(f"[*] Auto-load notice: {e}")

    def terminate(self):
        print(f"[✓] {self.name} terminated.")


def create_cutter_plugin():
    return QIRCutterPlugin()


if __name__ == "__main__":
    print("[*] Cutter QIR Dock Plugin module loaded.")
    if not CUTTER_AVAILABLE:
        print("[*] (PySide2/Cutter GUI environment is required to display Qt widgets interactively)")
