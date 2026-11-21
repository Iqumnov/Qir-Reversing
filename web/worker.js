/**
 * qir-reversing - Cloudflare Worker / Pages Edge API
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

const {
    initWasm,
    qirWasmDisassemble,
    qirWasmDeobfuscate,
    qirWasmRenderAscii,
    qirWasmRenderSvg,
    qirWasmVerify,
} = require('./js/qir_wasm.js');

const CORS_HEADERS = {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

function jsonResponse(data, status = 200) {
    return new Response(JSON.stringify(data, null, 2), {
        status,
        headers: {
            'Content-Type': 'application/json; charset=utf-8',
            ...CORS_HEADERS,
        },
    });
}

async function extractQIRCode(request) {
    const contentType = request.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
        const body = await request.json();
        return body.code || body.qir || body.ir || '';
    } else {
        return await request.text();
    }
}

async function handleApiRequest(request, pathname) {
    if (request.method === 'OPTIONS') {
        return new Response(null, { headers: CORS_HEADERS });
    }

    if (pathname === '/api/health' || pathname === '/api') {
        return jsonResponse({
            status: 'online',
            service: 'QIR Edge API',
            version: '0.1.0',
            engine: 'Rust wasm32-unknown-unknown / V8 Isolate',
            supportedEndpoints: [
                'POST /api/analyze',
                'POST /api/deobfuscate',
                'POST /api/circuit',
                'POST /api/verify',
                'POST /api/full',
                'GET /api/health'
            ]
        });
    }

    if (request.method !== 'POST') {
        return jsonResponse({ error: 'Method Not Allowed. Use POST.' }, 405);
    }

    let code = '';
    try {
        code = await extractQIRCode(request);
    } catch (err) {
        return jsonResponse({ error: 'Malformed request payload', details: err.message }, 400);
    }

    if (!code || !code.trim()) {
        return jsonResponse({ error: 'Missing QIR/LLVM bytecode input in request body.' }, 400);
    }

    await initWasm();

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
                obfuscationSeverityPct: res.obfuscation_ratio,
                findings: res.findings,
                auditReport: res.report,
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
            const res = qirWasmVerify(code);
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
            const disasm = qirWasmDisassemble(code);
            const deobf = qirWasmDeobfuscate(code);
            const ascii = qirWasmRenderAscii(code);
            const verifyRes = qirWasmVerify(code);
            return jsonResponse({
                status: 'ok',
                disassemblyText: disasm.disassembly,
                deobfuscation: {
                    originalCount: deobf.original_count,
                    cleanCount: deobf.cleaned_count,
                    obfuscationSeverityPct: deobf.obfuscation_ratio,
                    findings: deobf.findings,
                    auditReport: deobf.report,
                },
                asciiCircuit: ascii,
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

// Cloudflare Workers & Pages Functions Entry Point
const worker = {
    async fetch(request, env, ctx) {
        const url = new URL(request.url);

        // API routing
        if (url.pathname.startsWith('/api')) {
            return await handleApiRequest(request, url.pathname);
        }

        // Static Asset Fallback (for Cloudflare Pages / Workers Sites)
        if (env && env.ASSETS && typeof env.ASSETS.fetch === 'function') {
            return await env.ASSETS.fetch(request);
        }

        return new Response('qir-reversing', {
            status: 200,
            headers: { 'Content-Type': 'text/plain; charset=utf-8' }
        });
    }
};

module.exports = worker;
if (typeof exports !== 'undefined') {
    exports.default = worker;
}
