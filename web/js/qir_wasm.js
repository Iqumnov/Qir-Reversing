/**
 * QIR Reversing WebAssembly Bridge
 * Pure Rust WebAssembly execution in Browser & Cloudflare Edge Isolates
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

let wasmInstance = null;
let initPromise = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function resetWasmInstance() {
    wasmInstance = null;
    initPromise = null;
}

function isWasmInitialized() {
    return wasmInstance !== null;
}

async function initWasm(wasmInput) {
    if (wasmInstance) return wasmInstance;
    if (initPromise) return await initPromise;

    initPromise = (async () => {
        if (wasmInput instanceof WebAssembly.Module) {
            const instance = await WebAssembly.instantiate(wasmInput);
            wasmInstance = instance;
            return wasmInstance;
        }

        let wasmBytes;
        if (wasmInput instanceof Uint8Array || wasmInput instanceof ArrayBuffer) {
            wasmBytes = wasmInput;
        } else if (typeof window !== 'undefined') {
            const wasmUrl = new URL('wasm/qir_core.wasm', window.location.href);
            const res = await fetch(wasmUrl);
            if (!res.ok) throw new Error(`Failed to fetch WASM binary: ${res.status}`);
            wasmBytes = await res.arrayBuffer();
        } else if (typeof process !== 'undefined' && process.versions && process.versions.node && typeof require !== 'undefined') {
            const fs = require('fs');
            const path = require('path');
            const wasmPath = path.join(__dirname, '../wasm/qir_core.wasm');
            wasmBytes = fs.readFileSync(wasmPath);
        } else {
            throw new Error('QirWasm.initWasm(wasmModule) requires a compiled WebAssembly.Module/Uint8Array in Worker/edge runtimes.');
        }

        const { instance } = await WebAssembly.instantiate(wasmBytes);
        wasmInstance = instance;
        return wasmInstance;
    })();

    try {
        return await initPromise;
    } catch (err) {
        initPromise = null;
        throw err;
    }
}

function readPackedString(exp, outPtr) {
    const bufLen = exp.memory.buffer.byteLength;
    if (!outPtr || outPtr + 4 > bufLen) {
        throw new Error(`Invalid WASM output pointer: ${outPtr} (memory bytes: ${bufLen})`);
    }
    const len = new DataView(exp.memory.buffer, outPtr, 4).getUint32(0, true);
    if (outPtr + 4 + len > exp.memory.buffer.byteLength) {
        throw new Error(`WASM output length out of bounds: offset=${outPtr}, len=${len}`);
    }
    const outBytes = new Uint8Array(exp.memory.buffer, outPtr + 4, len);
    try {
        return decoder.decode(outBytes);
    } finally {
        exp.qir_dealloc(outPtr, len + 4);
    }
}

function toWasmBytes(input) {
    if (input instanceof Uint8Array) return input;
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    if (typeof input === 'string') return encoder.encode(input);
    throw new Error('QIR input must be a UTF-8 string or Uint8Array/ArrayBuffer.');
}

function callWasmInput(fn, input) {
    if (!wasmInstance) throw new Error("QIR Rust WebAssembly engine not initialized. Call initWasm() first.");
    const exp = wasmInstance.exports;
    const bytes = toWasmBytes(input);
    let ptr = 0;
    let outPtr = 0;
    try {
        ptr = exp.qir_alloc(bytes.length);
        new Uint8Array(exp.memory.buffer, ptr, bytes.length).set(bytes);
        outPtr = fn(ptr, bytes.length);
    } catch (err) {
        resetWasmInstance();
        throw err;
    } finally {
        if (ptr && wasmInstance) exp.qir_dealloc(ptr, bytes.length);
    }
    return readPackedString(exp, outPtr);
}

function qirWasmDecodeBitcode(bytes) {
    const raw = callWasmInput(wasmInstance.exports.qir_wasm_decode_bitcode_c, bytes);
    const parsed = JSON.parse(raw);
    if (parsed && parsed.error) throw new Error(parsed.error);
    return parsed;
}

function qirWasmDisassemble(irCodeOrBytes) {
    const raw = callWasmInput(wasmInstance.exports.qir_wasm_disassemble_c, irCodeOrBytes);
    const parsed = JSON.parse(raw);
    if (parsed && parsed.error) throw new Error(parsed.error);
    return parsed;
}

function qirWasmDeobfuscate(irCodeOrBytes) {
    const raw = callWasmInput(wasmInstance.exports.qir_wasm_deobfuscate_c, irCodeOrBytes);
    const parsed = JSON.parse(raw);
    if (parsed && parsed.error) throw new Error(parsed.error);
    return parsed;
}

function qirWasmRenderAscii(irCodeOrBytes) {
    const res = callWasmInput(wasmInstance.exports.qir_wasm_render_ascii_c, irCodeOrBytes);
    if (res.startsWith('; Error:')) throw new Error(res.slice(8).trim());
    return res;
}

function qirWasmRenderSvg(irCodeOrBytes) {
    const res = callWasmInput(wasmInstance.exports.qir_wasm_render_svg_c, irCodeOrBytes);
    if (res.trimStart().startsWith('{')) {
        const parsed = JSON.parse(res);
        if (parsed && parsed.error) throw new Error(parsed.error);
        if (parsed && parsed.svg) return parsed.svg;
    }
    if (/^<svg\b[^>]*>\s*<text\b[^>]*>Error:/i.test(res.trim())) {
        const msg = res
            .replace(/<[^>]+>/g, '')
            .replace(/^Error:\s*/i, '')
            .replace(/&lt;/g, '<')
            .replace(/&gt;/g, '>')
            .replace(/&amp;/g, '&')
            .trim();
        throw new Error(msg);
    }
    return res;
}

function qirWasmVerify(origIrOrBytes, cleanIrOrBytes = "", numQubits = 0) {
    if (!wasmInstance) throw new Error("QIR Rust WebAssembly engine not initialized.");
    const exp = wasmInstance.exports;

    const bOrig = toWasmBytes(origIrOrBytes);
    const bClean = toWasmBytes(cleanIrOrBytes);
    let ptrOrig = 0;
    let ptrClean = 0;
    let outPtr = 0;
    try {
        ptrOrig = exp.qir_alloc(bOrig.length);
        new Uint8Array(exp.memory.buffer, ptrOrig, bOrig.length).set(bOrig);
        ptrClean = exp.qir_alloc(bClean.length);
        new Uint8Array(exp.memory.buffer, ptrClean, bClean.length).set(bClean);
        outPtr = exp.qir_wasm_verify_c(ptrOrig, bOrig.length, ptrClean, bClean.length, numQubits);
    } catch (err) {
        resetWasmInstance();
        throw err;
    } finally {
        if (wasmInstance) {
            if (ptrClean) exp.qir_dealloc(ptrClean, bClean.length);
            if (ptrOrig) exp.qir_dealloc(ptrOrig, bOrig.length);
        }
    }

    const result = readPackedString(exp, outPtr);
    const parsed = JSON.parse(result);
    if (parsed && parsed.error) throw new Error(parsed.error);
    return parsed;
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        initWasm,
        isWasmInitialized,
        qirWasmDecodeBitcode,
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
        isWasmInitialized,
        decodeBitcode: qirWasmDecodeBitcode,
        disassemble: qirWasmDisassemble,
        deobfuscate: qirWasmDeobfuscate,
        renderAscii: qirWasmRenderAscii,
        renderSvg: qirWasmRenderSvg,
        verify: qirWasmVerify,
    };
}

