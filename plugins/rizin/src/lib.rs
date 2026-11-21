//! Native Rizin Architecture & Disassembler Plugin for Quantum Intermediate Representation (QIR)
//! Author: Matvei Igumnov <matvei@iqumnov.com>
//! Quantum Intermediate Representation Reverse Engineering

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

pub type RzAsmDisasmCb =
    unsafe extern "C" fn(a: *const c_void, op: *mut RzAsmOp, buf: *const u8, len: c_int) -> c_int;

#[repr(C)]
pub struct RzStrBuf {
    pub len: usize,
    pub ptr: *mut c_char,
    pub buf: [c_char; 64],
}

#[repr(C)]
pub struct RzAsmOp {
    pub size: c_int,
    pub buf_asm: [c_char; 256],
    pub buf_hex: [c_char; 128],
    pub payload: *mut c_void,
}

#[repr(C)]
pub struct RzAsmPlugin {
    pub name: *const c_char,
    pub arch: *const c_char,
    pub author: *const c_char,
    pub version: *const c_char,
    pub cpus: *const c_char,
    pub desc: *const c_char,
    pub license: *const c_char,
    pub bits: c_int,
    pub endian: c_int,
    pub init: Option<unsafe extern "C" fn(user: *mut *mut c_void) -> bool>,
    pub fini: Option<unsafe extern "C" fn(user: *mut c_void) -> bool>,
    pub disassemble: Option<RzAsmDisasmCb>,
    pub assemble: *const c_void,
    pub mnemonics: *const c_void,
    pub get_config: *const c_void,
    pub features: *const c_char,
    pub platforms: *const c_char,
    pub get_cpu_desc: *const c_void,
    pub sw_breakpoint: *const c_void,
}

#[repr(C)]
pub struct RzLibStruct {
    pub r#type: c_int, // RZ_LIB_TYPE_ASM = 3
    pub data: *const c_void,
    pub version: *const c_char,
    pub free: Option<unsafe extern "C" fn(data: *mut c_void)>,
}

unsafe impl Sync for RzAsmPlugin {}
unsafe impl Sync for RzLibStruct {}

unsafe extern "C" fn qir_disassemble_native(
    _a: *const c_void,
    op: *mut RzAsmOp,
    buf: *const u8,
    len: c_int,
) -> c_int {
    if op.is_null() || buf.is_null() || len < 1 {
        return 0;
    }

    let b0 = *buf;
    let b1 = if len > 1 { *buf.add(1) } else { 0 };
    let b2 = if len > 2 { *buf.add(2) } else { 0 };
    let b3 = if len > 3 { *buf.add(3) } else { 0 };

    let (mnemonic, size) = match b0 {
        0x01 => (format!("qir.h q[{}]", b1), 4),
        0x02 => (format!("qir.x q[{}]", b1), 4),
        0x03 => (format!("qir.y q[{}]", b1), 4),
        0x04 => (format!("qir.z q[{}]", b1), 4),
        0x05 => (format!("qir.s q[{}]", b1), 4),
        0x06 => (format!("qir.sdg q[{}]", b1), 4),
        0x07 => (format!("qir.t q[{}]", b1), 4),
        0x08 => (format!("qir.tdg q[{}]", b1), 4),
        0x10 => (format!("qir.cx q[{}], q[{}]", b1, b2), 4),
        0x11 => (format!("qir.cz q[{}], q[{}]", b1, b2), 4),
        0x12 => (format!("qir.swap q[{}], q[{}]", b1, b2), 4),
        0x13 => (format!("qir.ccx q[{}], q[{}], q[{}]", b1, b2, b3), 4),
        0x20 => {
            let rad = (b2 as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.rx({:.6}) q[{}]", rad, b1), 4)
        }
        0x21 => {
            let rad = (b2 as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.ry({:.6}) q[{}]", rad, b1), 4)
        }
        0x22 => {
            let rad = (b2 as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.rz({:.6}) q[{}]", rad, b1), 4)
        }
        0x30 => (format!("qir.mz q[{}] -> r[{}]", b1, b2), 4),
        0x3f => (format!("qir.reset q[{}]", b1), 4),
        0x42 => (
            "qir.header magic=0x4243 (LLVM Bitcode Container)".to_string(),
            4,
        ),
        _ => (format!("qir.invalid (0x{:02x})", b0), 1),
    };

    let actual_size = std::cmp::min(size, len);
    (*op).size = actual_size;

    // Write formatted mnemonic into RzAsmOp.buf_asm
    let bytes = mnemonic.as_bytes();
    let copy_len = std::cmp::min(bytes.len(), 255);
    for (i, &byte) in bytes.iter().take(copy_len).enumerate() {
        (*op).buf_asm[i] = byte as c_char;
    }
    (*op).buf_asm[copy_len] = 0;

    actual_size
}

static RZ_ASM_PLUGIN_QIR: RzAsmPlugin = RzAsmPlugin {
    name: c"qir".as_ptr(),
    arch: c"qir".as_ptr(),
    author: c"Matvei Igumnov <matvei@iqumnov.com>".as_ptr(),
    version: c"0.1.0".as_ptr(),
    cpus: c"qir_base,qir_adaptive".as_ptr(),
    desc: c"Quantum Intermediate Representation (QIR) Architecture Plugin".as_ptr(),
    license: c"LGPL-3.0-or-later".as_ptr(),
    bits: 64,
    endian: 1, // Little Endian
    init: None,
    fini: None,
    disassemble: Some(qir_disassemble_native),
    assemble: std::ptr::null(),
    mnemonics: std::ptr::null(),
    get_config: std::ptr::null(),
    features: std::ptr::null(),
    platforms: std::ptr::null(),
    get_cpu_desc: std::ptr::null(),
    sw_breakpoint: std::ptr::null(),
};

#[no_mangle]
pub static mut rizin_plugin: RzLibStruct = RzLibStruct {
    r#type: 3, // RZ_LIB_TYPE_ASM
    data: &RZ_ASM_PLUGIN_QIR as *const _ as *const c_void,
    version: c"0.9.1".as_ptr(),
    free: None,
};
