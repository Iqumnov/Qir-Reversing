/**
 * QIR Reversing - Shared Edge API Worker Handler
 * Compatible with Cloudflare Workers ESM (precompiled WebAssembly.Module),
 * Cloudflare Pages Functions (_worker.js), local Node server.js, and test_suite.js.
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

const CORS_HEADERS = {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, HEAD, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

const SECURITY_HEADERS = {
    'X-Content-Type-Options': 'nosniff',
    'Referrer-Policy': 'strict-origin-when-cross-origin',
    'Cache-Control': 'no-store',
};

const BLOCKED_STATIC_FILES = new Set([
    'server.js',
    'worker.js',
    '_worker.js',
    'worker-core.js',
    'test_suite.js',
    'wrangler.toml',
    'package.json',
    '_headers',
    'readme.md',
    '.assetsignore',
]);

function jsonResponse(data, status = 200) {
    return new Response(JSON.stringify(data, null, 2), {
        status,
        headers: {
            'Content-Type': 'application/json; charset=utf-8',
            ...CORS_HEADERS,
            ...SECURITY_HEADERS,
        },
    });
}

function hasBitcodeMagic(bytes) {
    if (!(bytes instanceof Uint8Array)) return false;
    if (bytes.length >= 4 && bytes[0] === 0x42 && bytes[1] === 0x43 && bytes[2] === 0xc0 && bytes[3] === 0xde) {
        return true;
    }
    return bytes.length >= 20 && bytes[0] === 0xde && bytes[1] === 0xc0 && bytes[2] === 0x17 && bytes[3] === 0x0b;
}

function decodeBase64Bytes(b64) {
    let normalized = b64.trim().replace(/\s+/g, '').replace(/-/g, '+').replace(/_/g, '/');
    while (normalized.length % 4 !== 0) {
        normalized += '=';
    }
    const bin = atob(normalized);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) {
        out[i] = bin.charCodeAt(i);
    }
    return out;
}

async function extractRequestPayload(request) {
    const contentType = (request.headers.get('content-type') || '').toLowerCase();
    if (contentType.includes('application/json')) {
        const body = await request.json();
        if (!body || typeof body !== 'object' || Array.isArray(body)) {
            throw new Error('JSON body must be an object.');
        }
        let code = '';
        if (typeof body.bitcodeBase64 === 'string' && body.bitcodeBase64.trim()) {
            code = decodeBase64Bytes(body.bitcodeBase64);
        } else {
            const raw = body.code ?? body.qir ?? body.ir ?? '';
            if (raw !== '' && typeof raw !== 'string') {
                throw new Error('Payload code/qir/ir field must be a string.');
            }
            code = raw;
        }
        let cleanCode = '';
        if (typeof body.cleanBitcodeBase64 === 'string' && body.cleanBitcodeBase64.trim()) {
            cleanCode = decodeBase64Bytes(body.cleanBitcodeBase64);
        } else {
            const rawClean = body.cleanCode ?? body.cleanQir ?? body.cleanIr ?? '';
            if (rawClean !== '' && typeof rawClean !== 'string') {
                throw new Error('Payload cleanCode/cleanQir/cleanIr field must be a string.');
            }
            cleanCode = rawClean;
        }
        let numQubits = 0;
        if (body.numQubits !== undefined && body.numQubits !== null) {
            if (!Number.isInteger(body.numQubits) || body.numQubits < 0 || body.numQubits > 64) {
                throw new Error('Payload numQubits must be an integer between 0 and 64.');
            }
            numQubits = body.numQubits;
        }
        return { code, cleanCode, numQubits };
    }
    const bytes = new Uint8Array(await request.arrayBuffer());
    if (contentType.includes('application/octet-stream') || hasBitcodeMagic(bytes)) {
        return { code: bytes, cleanCode: '', numQubits: 0 };
    }
    try {
        const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
        return { code: text, cleanCode: '', numQubits: 0 };
    } catch {
        throw new Error('Malformed UTF-8 payload or missing LLVM bitcode magic header (BC\\xc0\\xde or 0x0B17C0DE).');
    }
}

function createWorkerHandler(qirWasm, precompiledWasmModule = null) {
    const {
        initWasm,
        isWasmInitialized,
        qirWasmDecodeBitcode,
        qirWasmDisassemble,
        qirWasmDeobfuscate,
        qirWasmRenderAscii,
        qirWasmRenderSvg,
        qirWasmVerify,
    } = qirWasm;

    let initializedPromise = null;

    async function ensureWasmInitialized() {
        if (typeof isWasmInitialized === 'function' && !isWasmInitialized()) {
            initializedPromise = null;
        }
        if (!initializedPromise) {
            initializedPromise = (precompiledWasmModule
                ? initWasm(precompiledWasmModule)
                : initWasm()
            ).catch((err) => {
                initializedPromise = null;
                throw err;
            });
        }
        return await initializedPromise;
    }

    async function handleApiRequest(request, pathname) {
        if (request.method === 'OPTIONS') {
            return new Response(null, {
                status: 204,
                headers: { ...CORS_HEADERS, ...SECURITY_HEADERS },
            });
        }

        if (pathname === '/api/health' || pathname === '/api') {
            if (request.method !== 'GET' && request.method !== 'HEAD') {
                return jsonResponse({ error: 'Method Not Allowed. Use GET.' }, 405);
            }
            const payload = {
                status: 'online',
                service: 'QIR Edge API',
                version: '1.0.0',
                engine: 'Rust wasm32-unknown-unknown / V8 Isolate',
                supportedEndpoints: [
                    'POST /api/decode-bitcode',
                    'POST /api/analyze',
                    'POST /api/deobfuscate',
                    'POST /api/circuit',
                    'POST /api/verify',
                    'POST /api/full',
                    'GET /api/health',
                ],
            };
            if (request.method === 'HEAD') {
                return new Response(null, {
                    status: 200,
                    headers: {
                        'Content-Type': 'application/json; charset=utf-8',
                        ...CORS_HEADERS,
                        ...SECURITY_HEADERS,
                    },
                });
            }
            return jsonResponse(payload);
        }

        if (request.method !== 'POST') {
            return jsonResponse({ error: 'Method Not Allowed. Use POST.' }, 405);
        }

        let payload;
        try {
            payload = await extractRequestPayload(request);
        } catch (err) {
            return jsonResponse({ error: 'Malformed request payload', details: err.message }, 400);
        }

        const { code, cleanCode, numQubits } = payload;
        const isEmpty = code instanceof Uint8Array
            ? code.length === 0
            : (typeof code !== 'string' || !code.trim());
        if (isEmpty) {
            return jsonResponse({ error: 'Missing QIR/LLVM bitcode input in request body.' }, 400);
        }

        try {
            await ensureWasmInitialized();
        } catch (err) {
            return jsonResponse({ error: 'WASM runtime initialization failed', details: err.message }, 500);
        }

        if (pathname === '/api/decode-bitcode') {
            try {
                const res = qirWasmDecodeBitcode(code);
                return jsonResponse({
                    status: 'ok',
                    ir: res.ir,
                    instructionsCount: res.opcodes_count,
                    numQubits: res.num_qubits,
                    numResults: res.num_results,
                    entryPoint: res.entry_point,
                });
            } catch (err) {
                return jsonResponse({ error: 'Bitcode decode failed', details: err.message }, 422);
            }
        }

        if (pathname === '/api/analyze') {
            try {
                const res = qirWasmDisassemble(code);
                return jsonResponse({
                    status: 'ok',
                    instructionsCount: res.opcodes_count,
                    numQubits: res.num_qubits,
                    numResults: res.num_results,
                    entryPoint: res.entry_point,
                    disassemblyText: res.disassembly,
                });
            } catch (err) {
                return jsonResponse({ error: 'Disassembly failed', details: err.message }, 422);
            }
        }

        if (pathname === '/api/deobfuscate') {
            try {
                const res = qirWasmDeobfuscate(code);
                return jsonResponse({
                    status: 'ok',
                    originalCount: res.original_count,
                    cleanCount: res.cleaned_count,
                    eliminatedCount: res.original_count - res.cleaned_count,
                    liveQubits: res.live_qubits,
                    obfuscationSeverityPct: res.obfuscation_ratio,
                    findings: res.findings,
                    auditReport: res.report,
                    cleanDisassembly: res.clean_disassembly,
                    cleanQasm: res.clean_qasm,
                    cleanAscii: res.clean_ascii,
                    cleanSvg: res.clean_svg,
                });
            } catch (err) {
                return jsonResponse({ error: 'Deobfuscation failed', details: err.message }, 422);
            }
        }

        if (pathname === '/api/circuit') {
            try {
                const ascii = qirWasmRenderAscii(code);
                const svg = qirWasmRenderSvg(code);
                return jsonResponse({
                    status: 'ok',
                    asciiCircuit: ascii,
                    svgCircuit: svg,
                });
            } catch (err) {
                return jsonResponse({ error: 'Circuit rendering failed', details: err.message }, 422);
            }
        }

        if (pathname === '/api/verify') {
            try {
                const res = qirWasmVerify(code, cleanCode, numQubits);
                return jsonResponse({
                    status: 'ok',
                    verification: res,
                });
            } catch (err) {
                return jsonResponse({ error: 'Unitary verification failed', details: err.message }, 422);
            }
        }

        if (pathname === '/api/full') {
            try {
                // Decode binary bitcode once instead of 4x across sub-calls
                const irSource = code instanceof Uint8Array
                    ? (hasBitcodeMagic(code) ? qirWasmDecodeBitcode(code).ir : new TextDecoder('utf-8', { fatal: true }).decode(code))
                    : code;
                const disasm = qirWasmDisassemble(irSource);
                const deobf = qirWasmDeobfuscate(irSource);
                const ascii = qirWasmRenderAscii(irSource);
                const svg = qirWasmRenderSvg(irSource);
                const verifyRes = qirWasmVerify(irSource, cleanCode, numQubits);
                return jsonResponse({
                    status: 'ok',
                    numQubits: disasm.num_qubits,
                    liveQubits: deobf.live_qubits,
                    numResults: disasm.num_results,
                    entryPoint: disasm.entry_point,
                    disassemblyText: disasm.disassembly,
                    deobfuscation: {
                        originalCount: deobf.original_count,
                        cleanCount: deobf.cleaned_count,
                        eliminatedCount: deobf.original_count - deobf.cleaned_count,
                        obfuscationSeverityPct: deobf.obfuscation_ratio,
                        findings: deobf.findings,
                        auditReport: deobf.report,
                    },
                    asciiCircuit: ascii,
                    svgCircuit: svg,
                    cleanDisassembly: deobf.clean_disassembly,
                    cleanAsciiCircuit: deobf.clean_ascii,
                    cleanSvgCircuit: deobf.clean_svg,
                    cleanQasm: deobf.clean_qasm,
                    verification: verifyRes,
                });
            } catch (err) {
                return jsonResponse({ error: 'Pipeline processing failed', details: err.message }, 422);
            }
        }

        return jsonResponse({ error: `Endpoint not found: ${pathname}` }, 404);
    }

    return {
        async fetch(request, env, _ctx) {
            const url = new URL(request.url);

            if (url.pathname === '/api' || url.pathname.startsWith('/api/')) {
                return await handleApiRequest(request, url.pathname);
            }

            if (env && env.ASSETS && typeof env.ASSETS.fetch === 'function') {
                const segs = url.pathname.replace(/^\/+/, '').split('/');
                const lastSeg = (segs[segs.length - 1] || '').toLowerCase();
                if (segs.some(s => s.startsWith('.')) || BLOCKED_STATIC_FILES.has(lastSeg)) {
                    return new Response('404 Not Found', { status: 404, headers: SECURITY_HEADERS });
                }
                return await env.ASSETS.fetch(request);
            }

            return new Response('qir-reversing', {
                status: 200,
                headers: {
                    'Content-Type': 'text/plain; charset=utf-8',
                    ...SECURITY_HEADERS,
                },
            });
        },
    };
}

module.exports = { createWorkerHandler };

