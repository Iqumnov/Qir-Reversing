"""
QIR Reversing: Quantum Intermediate Representation Reverse Engineering Platform
Author: Matvei Igumnov <matvei@iqumnov.com>
"""

__version__ = "0.1.0"
__author__ = "Matvei Igumnov <matvei@iqumnov.com>"

from .qir_parser import QIRParser
from .qir_disasm import QIRDisassembler
from .qir_deobfuscator import QIRDeobfuscator

__all__ = ["QIRParser", "QIRDisassembler", "QIRDeobfuscator"]

