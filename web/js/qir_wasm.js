/**
 * qir-reversing WebAssembly Bridge
 * Pure Rust WebAssembly execution in Browser & Cloudflare Edge Isolates
 * Author: Matvei Igumnov <iqumnov@proton.me>
 */

let wasmInstance = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

async function initWasm(wasmInput) {
    if (wasmInstance) return wasmInstance;

    let wasmBytes;
    if (wasmInput instanceof Uint8Array || wasmInput instanceof ArrayBuffer) {
        wasmBytes = wasmInput;
    } else if (typeof window !== 'undefined') {
        const wasmUrl = new URL('wasm/qir_core.wasm', window.location.href);
        const res = await fetch(wasmUrl);
        wasmBytes = await res.arrayBuffer();
    } else if (typeof require !== 'undefined') {
        const fs = require('fs');
        const path = require('path');
        const wasmPath = path.join(__dirname, '../wasm/qir_core.wasm');
        wasmBytes = fs.readFileSync(wasmPath);
    }

    const { instance } = await WebAssembly.instantiate(wasmBytes);
    wasmInstance = instance;
    return wasmInstance;
}

function callWasmString(fn, str) {
    if (!wasmInstance) throw new Error("QIR Rust WebAssembly engine not initialized. Call initWasm() first.");
    const exp = wasmInstance.exports;
    const bytes = encoder.encode(str);
    const ptr = exp.qir_alloc(bytes.length);
    new Uint8Array(exp.memory.buffer, ptr, bytes.length).set(bytes);
    
    const outPtr = fn(ptr, bytes.length);
    exp.qir_dealloc(ptr, bytes.length);

    const len = new DataView(exp.memory.buffer, outPtr, 4).getUint32(0, true);
    const outBytes = new Uint8Array(exp.memory.buffer, outPtr + 4, len);
    const result = decoder.decode(outBytes);
    exp.qir_dealloc(outPtr, len + 4);
    return result;
}

function qirWasmDisassemble(irCode) {
    const raw = callWasmString(wasmInstance.exports.qir_wasm_disassemble_c, irCode);
    return JSON.parse(raw);
}

function qirWasmDeobfuscate(irCode) {
    const raw = callWasmString(wasmInstance.exports.qir_wasm_deobfuscate_c, irCode);
    return JSON.parse(raw);
}

function qirWasmRenderAscii(irCode) {
    return callWasmString(wasmInstance.exports.qir_wasm_render_ascii_c, irCode);
}

function qirWasmRenderSvg(irCode) {
    return callWasmString(wasmInstance.exports.qir_wasm_render_svg_c, irCode);
}

function qirWasmVerify(origIr, cleanIr = "", numQubits = 0) {
    if (!wasmInstance) throw new Error("QIR Rust WebAssembly engine not initialized.");
    const exp = wasmInstance.exports;
    
    const bOrig = encoder.encode(origIr);
    const ptrOrig = exp.qir_alloc(bOrig.length);
    new Uint8Array(exp.memory.buffer, ptrOrig, bOrig.length).set(bOrig);

    const bClean = encoder.encode(cleanIr);
    const ptrClean = exp.qir_alloc(bClean.length);
    new Uint8Array(exp.memory.buffer, ptrClean, bClean.length).set(bClean);

    const outPtr = exp.qir_wasm_verify_c(ptrOrig, bOrig.length, ptrClean, bClean.length, numQubits);
    exp.qir_dealloc(ptrOrig, bOrig.length);
    exp.qir_dealloc(ptrClean, bClean.length);

    const len = new DataView(exp.memory.buffer, outPtr, 4).getUint32(0, true);
    const outBytes = new Uint8Array(exp.memory.buffer, outPtr + 4, len);
    const result = decoder.decode(outBytes);
    exp.qir_dealloc(outPtr, len + 4);
    return JSON.parse(result);
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        initWasm,
        qirWasmDisassemble,
        qirWasmDeobfuscate,
        qirWasmRenderAscii,
        qirWasmRenderSvg,
        qirWasmVerify,
    };
}

if (typeof window !== 'undefined') {
    window.QirWasm = {
        initWasm,
        disassemble: qirWasmDisassemble,
        deobfuscate: qirWasmDeobfuscate,
        renderAscii: qirWasmRenderAscii,
        renderSvg: qirWasmRenderSvg,
        verify: qirWasmVerify,
    };
}

