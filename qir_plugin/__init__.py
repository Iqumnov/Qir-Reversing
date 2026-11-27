"""
QIR Reversing: Quantum Intermediate Representation Reverse Engineering Platform
Author: Matvei Igumnov <matvei@iqumnov.com>
"""

__version__ = "1.0.0"
__author__ = "Matvei Igumnov <matvei@iqumnov.com>"

from .qir_parser import QIRParser
from .qir_disasm import QIRDisassembler
from .qir_deobfuscator import QIRDeobfuscator
from .cutter_qir_dock import create_cutter_plugin

__all__ = ["QIRParser", "QIRDisassembler", "QIRDeobfuscator", "create_cutter_plugin"]

