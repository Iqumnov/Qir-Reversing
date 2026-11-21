"""
Probe "Empty" / Dummy Plugin for Cutter GUI
Author: Matvei Igumnov <matvei@iqumnov.com>

Demonstrates Cutter's Python Plugin Architecture:
1. Subclassing cutter.CutterPlugin
2. Implementing setupPlugin() and setupInterface()
3. Creating a custom QDockWidget for quantum reversing inspection
"""

try:
    import cutter
    from PySide2.QtWidgets import QDockWidget, QLabel, QVBoxLayout, QWidget, QPushButton
    from PySide2.QtCore import Qt
    CUTTER_AVAILABLE = True
except ImportError:
    CUTTER_AVAILABLE = False


class DummyQIRDock(QDockWidget if CUTTER_AVAILABLE else object):
    """Minimal probe dock widget for Cutter interface."""
    def __init__(self, parent=None):
        if not CUTTER_AVAILABLE:
            return
        super().__init__("QIR Quantum Inspector (Dummy)", parent)
        self.setObjectName("DummyQIRDock")

        container = QWidget()
        layout = QVBoxLayout(container)

        title = QLabel("<h3>QIR Quantum Reversing Probe</h3>")
        title.setAlignment(Qt.AlignCenter)
        layout.addWidget(title)

        info = QLabel(
            "<b>Author:</b> Matvei Igumnov<br>"
            "<b>Target:</b> LLVM Quantum Intermediate Representation (QIR)<br>"
            "<b>Status:</b> Dummy probe module initialized successfully."
        )
        layout.addWidget(info)

        btn = QPushButton("Test Quantum Instruction Probe")
        btn.clicked.connect(self.on_test_clicked)
        layout.addWidget(btn)

        layout.addStretch()
        self.setWidget(container)

    def on_test_clicked(self):
        print("[QIR Dummy Plugin] Test trigger acknowledged in Cutter.")


class DummyQIRPlugin(cutter.CutterPlugin if CUTTER_AVAILABLE else object):
    """Cutter Plugin Manifest for Dummy Module."""
    name = "QIR Dummy Plugin"
    description = "Probe empty module for QIR architecture research"
    version = "1.0.0"
    author = "Matvei Igumnov"

    def setupPlugin(self):
        print(f"[✓] {self.name} v{self.version} registered in Cutter core.")

    def setupInterface(self, main):
        if not CUTTER_AVAILABLE:
            return
        dock = DummyQIRDock(main)
        main.addDockWidget(Qt.RightDockWidgetArea, dock)
        print(f"[✓] {self.name} UI dock added to Cutter main window.")


def create_cutter_plugin():
    return DummyQIRPlugin()


if __name__ == "__main__":
    print("[*] Dummy Cutter plugin module loaded directly.")
    plugin = create_cutter_plugin()
    if CUTTER_AVAILABLE:
        plugin.setupPlugin()
    else:
        print("[*] (PySide2/Cutter not in current CLI environment; file serves as Cutter extension script)")

