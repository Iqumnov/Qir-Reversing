/**
 * Probe "Empty" / Dummy Module for Rizin / Cutter
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 *
 * This module demonstrates the fundamental architecture of Rizin/Cutter plugins:
 * 1. Implementation of RzAsmPlugin structure (architecture identification, disassembly callback).
 * 2. Export of the standard 'rizin_plugin' RzLibStruct descriptor.
 * 3. Dynamic loading and opcode decoding verification.
 */

#include <stdio.h>
#include <string.h>
#include <librz/rz_types.h>
#include <librz/rz_lib.h>
#include <librz/rz_asm.h>

static int dummy_disassemble(const RzAsm *a, RzAsmOp *op, const ut8 *buf, int len) {
    if (!op || !buf || len < 1) {
        return 0;
    }

    op->size = 1;
    // Map sample test opcodes for verification
    switch (buf[0]) {
        case 0x01:
            rz_strbuf_setf(&op->buf_asm, "qir.h q[0]");
            break;
        case 0x02:
            if (len >= 2) {
                rz_strbuf_setf(&op->buf_asm, "qir.cx q[%d], q[%d]", buf[0] & 1, buf[1] & 1);
                op->size = 2;
            } else {
                rz_strbuf_setf(&op->buf_asm, "qir.cx q[0], q[1]");
            }
            break;
        case 0x03:
            rz_strbuf_setf(&op->buf_asm, "qir.mz q[0] -> r[0]");
            break;
        case 0x42: // Magic 'B' from LLVM BC
            rz_strbuf_setf(&op->buf_asm, "qir.bitcode_header magic=0x42");
            break;
        default:
            rz_strbuf_setf(&op->buf_asm, "qir.nop (0x%02x)", buf[0]);
            break;
    }

    return op->size;
}

static RzAsmPlugin rz_asm_plugin_dummy = {
    .name = "qir_dummy",
    .arch = "qir_dummy",
    .author = "Matvei Igumnov",
    .version = "1.0.0",
    .cpus = "qpu_virtual",
    .desc = "Probe Dummy QIR Architecture Plugin for Rizin/Cutter",
    .license = "LGPL3",
    .bits = 64,
    .endian = RZ_SYS_ENDIAN_LITTLE,
    .disassemble = &dummy_disassemble,
};

#ifndef RZ_PLUGIN_INCORE
RZ_API RzLibStruct rizin_plugin = {
    .type = RZ_LIB_TYPE_ASM,
    .data = &rz_asm_plugin_dummy,
    .version = RZ_VERSION,
};
#endif

