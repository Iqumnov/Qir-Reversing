//! Native Rizin Architecture & Disassembler Plugin for Quantum Intermediate Representation (QIR)
//! Author: Matvei Igumnov <matvei@iqumnov.com>
//! Quantum Intermediate Representation Reverse Engineering

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

pub type RzAsmDisasmCb =
    unsafe extern "C" fn(a: *const c_void, op: *mut RzAsmOp, buf: *const u8, len: c_int) -> c_int;

#[repr(C)]
pub struct RzStrBuf {
    pub buf: [c_char; 32],
    pub len: usize,
    pub ptr: *mut c_char,
    pub ptrlen: usize,
}

#[repr(C)]
pub struct RzAsmOp {
    pub size: c_int,
    pub bitsize: c_int,
    pub payload: c_int,
    pub buf: RzStrBuf,
    pub buf_asm: RzStrBuf,
    pub buf_inc: *mut c_void,
    pub asm_toks: *mut c_void,
}

extern "C" {
    fn free(ptr: *mut c_void);
}

unsafe fn write_strbuf(sb: &mut RzStrBuf, text: &str) {
    if !sb.ptr.is_null() && sb.ptrlen > 0 {
        free(sb.ptr as *mut c_void);
    }
    sb.buf = [0; 32];
    let bytes = text.as_bytes();
    let mut copy_len = std::cmp::min(bytes.len(), 31);
    while copy_len > 0 && !text.is_char_boundary(copy_len) {
        copy_len -= 1;
    }
    for (i, &b) in bytes.iter().take(copy_len).enumerate() {
        sb.buf[i] = b as c_char;
    }
    sb.buf[copy_len] = 0;
    sb.len = copy_len;
    sb.ptr = std::ptr::null_mut();
    sb.ptrlen = 0;
}

unsafe fn write_rawbuf(sb: &mut RzStrBuf, raw: &[u8]) {
    if !sb.ptr.is_null() && sb.ptrlen > 0 {
        free(sb.ptr as *mut c_void);
    }
    sb.buf = [0; 32];
    let copy_len = std::cmp::min(raw.len(), 31);
    for (i, &b) in raw.iter().take(copy_len).enumerate() {
        sb.buf[i] = b as c_char;
    }
    sb.len = copy_len;
    sb.ptr = std::ptr::null_mut();
    sb.ptrlen = 0;
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
    let is_known_4byte = matches!(
        b0,
        0x01..=0x08 | 0x0f | 0x10..=0x13 | 0x20..=0x22 | 0x30 | 0x3f | 0x40..=0x42 | 0x50 | 0x90 | 0xeb
    );
    if is_known_4byte && len < 4 {
        (*op).size = 0;
        write_strbuf(&mut (*op).buf_asm, "qir.truncated");
        write_rawbuf(
            &mut (*op).buf,
            std::slice::from_raw_parts(buf, len as usize),
        );
        return 0;
    }

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
        0x0f => (format!("qir.rt(0x{:02x})", b1), 4),
        0x10 => (format!("qir.cx q[{}], q[{}]", b1, b2), 4),
        0x11 => (format!("qir.cz q[{}], q[{}]", b1, b2), 4),
        0x12 => (format!("qir.swap q[{}], q[{}]", b1, b2), 4),
        0x13 => (format!("qir.ccx q[{}], q[{}], q[{}]", b1, b2, b3), 4),
        0x20 => {
            let deg = (b2 as u16) | ((b3 as u16) << 8);
            let rad = (deg as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.rx({:.6}) q[{}]", rad, b1), 4)
        }
        0x21 => {
            let deg = (b2 as u16) | ((b3 as u16) << 8);
            let rad = (deg as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.ry({:.6}) q[{}]", rad, b1), 4)
        }
        0x22 => {
            let deg = (b2 as u16) | ((b3 as u16) << 8);
            let rad = (deg as f64) * std::f64::consts::PI / 180.0;
            (format!("qir.rz({:.6}) q[{}]", rad, b1), 4)
        }
        0x30 => (format!("qir.mz q[{}] -> r[{}]", b1, b2), 4),
        0x3f => (format!("qir.reset q[{}]", b1), 4),
        0x40 => (format!("qir.qalloc -> q[{}]", b1), 4),
        0x41 => (format!("qir.qfree q[{}]", b1), 4),
        0x42 if b1 == 0x43 && b2 == 0xc0 && b3 == 0xde => {
            ("qir.bc_header magic=0x4243c0de".to_string(), 4)
        }
        0x42 => (format!("qir.record_output r[{}]", b1), 4),
        0x50 => ("qir.barrier".to_string(), 4),
        0x90 => ("qir.label".to_string(), 4),
        0xeb => ("qir.br".to_string(), 4),
        _ => (format!("qir.invalid (0x{:02x})", b0), 1),
    };

    let actual_size = std::cmp::min(size, len);
    (*op).size = actual_size;
    write_strbuf(&mut (*op).buf_asm, &mnemonic);
    write_rawbuf(
        &mut (*op).buf,
        std::slice::from_raw_parts(buf, actual_size as usize),
    );

    actual_size
}

static RZ_ASM_PLUGIN_QIR: RzAsmPlugin = RzAsmPlugin {
    name: c"qir".as_ptr(),
    arch: c"qir".as_ptr(),
    author: c"Matvei Igumnov <matvei@iqumnov.com>".as_ptr(),
    version: c"1.0.0".as_ptr(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::mem::{offset_of, size_of};

    fn empty_strbuf() -> RzStrBuf {
        RzStrBuf {
            buf: [0; 32],
            len: 0,
            ptr: std::ptr::null_mut(),
            ptrlen: 0,
        }
    }

    fn empty_asm_op() -> RzAsmOp {
        RzAsmOp {
            size: 0,
            bitsize: 0,
            payload: 0,
            buf: empty_strbuf(),
            buf_asm: empty_strbuf(),
            buf_inc: std::ptr::null_mut(),
            asm_toks: std::ptr::null_mut(),
        }
    }

    #[test]
    fn test_rizin_c_abi_layout() {
        // RzStrBuf layout in Rizin 0.9.1 rz_strbuf.h: buf[32], len, ptr, ptrlen
        assert_eq!(offset_of!(RzStrBuf, buf), 0);
        assert_eq!(offset_of!(RzStrBuf, len), 32);
        assert_eq!(size_of::<RzStrBuf>(), 32 + 3 * size_of::<usize>());
        // RzAsmOp fields after buf and buf_asm are pointers (buf_inc, asm_toks)
        assert_eq!(
            size_of::<RzAsmOp>(),
            16 + 2 * size_of::<RzStrBuf>() + 2 * size_of::<*mut c_void>()
        );
    }

    #[test]
    fn test_qir_disassemble_native_opcodes() {
        let mut op = empty_asm_op();

        unsafe {
            assert_eq!(
                qir_disassemble_native(std::ptr::null(), &mut op, std::ptr::null(), 4),
                0
            );

            let h_bytes = [0x01u8, 0x02, 0x00, 0x00];
            let sz = qir_disassemble_native(std::ptr::null(), &mut op, h_bytes.as_ptr(), 4);
            assert_eq!(sz, 4);
            assert_eq!(op.size, 4);
            let s = CStr::from_ptr(op.buf_asm.buf.as_ptr()).to_str().unwrap();
            assert_eq!(s, "qir.h q[2]");
            assert_eq!(op.buf_asm.len, s.len());

            let bc_magic = [0x42u8, 0x43, 0xc0, 0xde];
            qir_disassemble_native(std::ptr::null(), &mut op, bc_magic.as_ptr(), 4);
            let magic_str = CStr::from_ptr(op.buf_asm.buf.as_ptr()).to_str().unwrap();
            assert_eq!(magic_str, "qir.bc_header magic=0x4243c0de");

            let rz_90 = [0x22u8, 0x01, 90, 0];
            qir_disassemble_native(std::ptr::null(), &mut op, rz_90.as_ptr(), 4);
            let rz_str = CStr::from_ptr(op.buf_asm.buf.as_ptr()).to_str().unwrap();
            assert!(rz_str.starts_with("qir.rz(1.570796) q[1]"));

            let unk = [0xfeu8, 0x00, 0x00, 0x00];
            let inv_sz = qir_disassemble_native(std::ptr::null(), &mut op, unk.as_ptr(), 4);
            assert_eq!(inv_sz, 1);

            let trunc = [0x10u8, 0x05];
            let tr_sz = qir_disassemble_native(std::ptr::null(), &mut op, trunc.as_ptr(), 2);
            assert_eq!(tr_sz, 0);
            assert_eq!(op.size, 0);
            let tr_str = CStr::from_ptr(op.buf_asm.buf.as_ptr()).to_str().unwrap();
            assert_eq!(tr_str, "qir.truncated");
        }
    }
}
