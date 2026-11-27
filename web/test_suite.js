/**
 * QIR Reversing Test Suite
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

const fs = require('fs');
const path = require('path');
const qirWasm = require('./js/qir_wasm.js');
const {
    initWasm,
    qirWasmDecodeBitcode,
    qirWasmDisassemble,
    qirWasmDeobfuscate,
    qirWasmRenderAscii,
    qirWasmRenderSvg,
    qirWasmVerify,
} = qirWasm;
const { createWorkerHandler } = require('./worker-core.js');

function assert(condition, message) {
    if (!condition) {
        console.error(`  [FAIL] ${message}`);
        process.exit(1);
    }
    console.log(`  [PASS] ${message}`);
}

async function runAllTests() {
    console.log('════════════════════════════════════════════════════════════════');
    console.log('[TEST SUITE] QIR Reversing');
    console.log('════════════════════════════════════════════════════════════════\n');

    const wasmBytes = fs.readFileSync(path.join(__dirname, 'wasm/qir_core.wasm'));
    const precompiledModule = await WebAssembly.compile(wasmBytes);
    const worker = createWorkerHandler(qirWasm, precompiledModule);
    await initWasm(precompiledModule);

    const demoDir = path.resolve(__dirname, '../demo_programs');
    const demoCleanPath = path.join(demoDir, 'lr2_quantum_demo.ll');
    const demoObfPath = path.join(demoDir, 'lr2_obfuscated.ll');

    const cleanCode = fs.readFileSync(demoCleanPath, 'utf8');
    const obfCode = fs.readFileSync(demoObfPath, 'utf8');

    // 1. WebAssembly Disassembly
    console.log('1. RUST WEBASSEMBLY DISASSEMBLER VERIFICATION');
    const cleanDisasm = qirWasmDisassemble(cleanCode);
    assert(cleanDisasm.opcodes_count === 5, `Clean sample parsed into 5 opcodes (got ${cleanDisasm.opcodes_count})`);
    assert(cleanDisasm.num_qubits === 2, `Clean sample allocated 2 qubits (got ${cleanDisasm.num_qubits})`);
    assert(cleanDisasm.disassembly.includes('Superposition') && cleanDisasm.disassembly.includes('Entanglement'), 'Disassembly includes semantic annotations');

    // 2. WebAssembly Circuit Rendering
    console.log('\n2. RUST WEBASSEMBLY CIRCUIT VISUALIZER');
    const ascii = qirWasmRenderAscii(cleanCode);
    assert(ascii.includes('q[0]:') && ascii.includes('●') && ascii.includes('X'), 'ASCII circuit topology renders correctly');
    const svg = qirWasmRenderSvg(cleanCode);
    assert(svg.includes('<svg') && svg.includes('</svg>'), 'Vector SVG generated cleanly with SVG container');

    // 3. WebAssembly Deobfuscation
    console.log('\n3. RUST WEBASSEMBLY DEOBFUSCATOR VERIFICATION');
    const deobf = qirWasmDeobfuscate(obfCode);
    assert(deobf.original_count === 18, `Obfuscated count: 18 (got ${deobf.original_count})`);
    assert(deobf.cleaned_count === 5, `Cleaned count: 5 (got ${deobf.cleaned_count})`);
    assert(deobf.findings.length === 5, `Detected 5 obfuscation sites (got ${deobf.findings.length})`);
    assert(deobf.obfuscation_ratio > 70.0, `Obfuscation ratio > 70% (got ${deobf.obfuscation_ratio.toFixed(1)}%)`);
    assert(deobf.clean_qasm.includes('OPENQASM 3.0;'), 'Reconstructed OpenQASM 3.0 output is sound');

    // 4. WebAssembly Unitary Verifier
    console.log('\n4. RUST WEBASSEMBLY 2^N UNITARY VERIFIER');
    const verifyRes = qirWasmVerify(obfCode);
    assert(verifyRes.is_equivalent === true, 'Formal unitary equivalence proven on observable subspace');
    assert(verifyRes.frobenius_error < 1e-10, `Frobenius matrix error is 0.0 (got ${verifyRes.frobenius_error})`);
    assert((1.0 - verifyRes.fidelity) < 1e-6, `Quantum process fidelity is 1.0 (got ${verifyRes.fidelity})`);

    // 5. Cloudflare Worker Edge API
    console.log('\n5. CLOUDFLARE WORKER EDGE API VERIFICATION');
    const healthReq = new Request('https://edge.quantum.internal/api/health');
    const healthRes = await worker.fetch(healthReq);
    assert(healthRes.status === 200, 'GET /api/health returned 200 OK');

    const deobfReq = new Request('https://edge.quantum.internal/api/deobfuscate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: obfCode })
    });
    const deobfRes = await worker.fetch(deobfReq);
    assert(deobfRes.status === 200, 'POST /api/deobfuscate returned 200 OK');
    const deobfData = await deobfRes.json();
    assert(deobfData.cleanCount === 5, `Edge API reduced instructions to 5 (got ${deobfData.cleanCount})`);
    assert(deobfData.findings.length === 5, 'Edge API reported 5 findings');

    const fullReq = new Request('https://edge.quantum.internal/api/full', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: obfCode })
    });
    const fullRes = await worker.fetch(fullReq);
    assert(fullRes.status === 200, 'POST /api/full returned 200 OK');
    const fullData = await fullRes.json();
    assert(fullData.verification.is_equivalent === true, 'Full Edge API proved unitary equivalence');

    // Additional Edge API coverage: /api/analyze, /api/circuit, /api/verify, 405, 400, 404
    const analyzeRes = await worker.fetch(new Request('https://edge.quantum.internal/api/analyze', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: cleanCode })
    }));
    assert(analyzeRes.status === 200, 'POST /api/analyze returned 200 OK');

    const circuitRes = await worker.fetch(new Request('https://edge.quantum.internal/api/circuit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: cleanCode })
    }));
    assert(circuitRes.status === 200, 'POST /api/circuit returned 200 OK');

    const verifyApiRes = await worker.fetch(new Request('https://edge.quantum.internal/api/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: obfCode })
    }));
    const verifyApiData = await verifyApiRes.json();
    assert(verifyApiRes.status === 200 && verifyApiData.verification.is_equivalent === true, 'POST /api/verify proved equivalence');

    const methodNotAllowed = await worker.fetch(new Request('https://edge.quantum.internal/api/analyze', { method: 'GET' }));
    assert(methodNotAllowed.status === 405, 'GET /api/analyze rejects with 405 Method Not Allowed');

    const emptyPayload = await worker.fetch(new Request('https://edge.quantum.internal/api/deobfuscate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: '   ' })
    }));
    assert(emptyPayload.status === 400, 'Blank code payload rejects with 400 Bad Request');

    const notFoundApi = await worker.fetch(new Request('https://edge.quantum.internal/api/nonexistent', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: cleanCode })
    }));
    assert(notFoundApi.status === 404, 'Unknown /api endpoint returns 404 Not Found');

    // Verify static WebAssembly.Module import in Cloudflare edge workers (V8 isolate compliance)
    const workerJsSrc = fs.readFileSync(path.join(__dirname, 'worker.js'), 'utf8');
    const pagesWorkerSrc = fs.readFileSync(path.join(__dirname, '_worker.js'), 'utf8');
    assert(workerJsSrc.includes("import wasmModule from './wasm/qir_core.wasm'") && pagesWorkerSrc.includes("import wasmModule from './wasm/qir_core.wasm'"), 'Cloudflare Workers/Pages entrypoints use static WebAssembly.Module imports');

    const badPrefixReq = await worker.fetch(new Request('https://edge.quantum.internal/apiv2/health'));
    assert(badPrefixReq.status === 200 && (await badPrefixReq.text()) === 'qir-reversing', 'Non-/api/ path /apiv2 does not trigger /api router');

    const badHealthMethod = await worker.fetch(new Request('https://edge.quantum.internal/api/health', { method: 'DELETE' }));
    assert(badHealthMethod.status === 405, 'DELETE /api/health rejects with 405 Method Not Allowed');

    // Real binary LLVM Bitcode (.bc) decoding & end-to-end analysis inside WebAssembly (zero CLI / llvm-dis / pyqir)
    const samplesDir = path.resolve(__dirname, '../samples');
    const bellBcBytes = new Uint8Array(fs.readFileSync(path.join(samplesDir, 'bell_state.bc')));
    const obfBcBytes = new Uint8Array(fs.readFileSync(path.join(samplesDir, 'obfuscated_circuit.bc')));

    const decodedBell = qirWasmDecodeBitcode(bellBcBytes);
    assert(decodedBell.opcodes_count === 5 && decodedBell.num_qubits === 2, `Pure-Rust WASM decoded binary bell_state.bc (5 opcodes, 2 qubits; got ${decodedBell.opcodes_count})`);
    assert(decodedBell.ir.includes('@__quantum__qis__h__body'), 'Recovered textual LLVM IR from bell_state.bc');

    const obfBcDeobf = qirWasmDeobfuscate(obfBcBytes);
    assert(obfBcDeobf.original_count === 18 && obfBcDeobf.cleaned_count === 5, 'Pure-Rust WASM deobfuscated binary obfuscated_circuit.bc directly (18 -> 5)');

    const obfBcVerify = qirWasmVerify(obfBcBytes);
    assert(obfBcVerify.is_equivalent === true && Math.abs(1.0 - obfBcVerify.fidelity) < 1e-6, 'Pure-Rust WASM proved unitary equivalence on binary obfuscated_circuit.bc');

    // Edge API binary .bc + base64 bitcode endpoints
    const decodeBcApiRes = await worker.fetch(new Request('https://edge.quantum.internal/api/decode-bitcode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: bellBcBytes
    }));
    const decodeBcApiJson = await decodeBcApiRes.json();
    assert(decodeBcApiRes.status === 200 && decodeBcApiJson.instructionsCount === 5, 'POST /api/decode-bitcode decoded raw binary .bc stream');

    const analyzeB64Res = await worker.fetch(new Request('https://edge.quantum.internal/api/full', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bitcodeBase64: Buffer.from(obfBcBytes).toString('base64') })
    }));
    const analyzeB64Json = await analyzeB64Res.json();
    assert(analyzeB64Res.status === 200 && analyzeB64Json.verification.is_equivalent === true, 'POST /api/full processed base64 .bc payload and verified equivalence');

    // Scalable 32-qubit QMDD equivalence checking inside WebAssembly (beyond 10-qubit dense cap)
    let largeIr = 'define void @ghz32() #0 {\nentry:\n  call void @__quantum__qis__h__body(ptr null)\n';
    for (let q = 0; q < 31; q++) {
        largeIr += `  call void @__quantum__qis__cnot__body(ptr inttoptr (i64 ${q} to ptr), ptr inttoptr (i64 ${q + 1} to ptr))\n`;
        if (q % 6 === 0) {
            largeIr += `  call void @__quantum__qis__x__body(ptr inttoptr (i64 ${q} to ptr))\n`;
            largeIr += `  call void @__quantum__qis__x__body(ptr inttoptr (i64 ${q} to ptr))\n`;
        }
    }
    largeIr += '  ret void\n}\nattributes #0 = { "entry_point" "required_num_qubits"="32" }\n';
    const largeVerify = qirWasmVerify(largeIr);
    assert(largeVerify.is_equivalent === true && largeVerify.num_qubits === 32 && largeVerify.fidelity > 0.999999, 'WASM QMDD verifier proved 32-qubit GHZ+obfuscation equivalence');

    const faultyLargeIr = largeIr.replace('  ret void', '  call void @__quantum__qis__z__body(ptr inttoptr (i64 31 to ptr))\n  ret void');
    const faultyLargeVerify = qirWasmVerify(largeIr, faultyLargeIr, 32);
    assert(faultyLargeVerify.is_equivalent === false, 'WASM QMDD verifier detected single-qubit Z fault on 32-qubit circuit');

    // Verify wasm memory deallocation across repeated invocations does not leak/trap
    for (let k = 0; k < 50; k++) {
        const res = qirWasmDeobfuscate(obfCode);
        if (res.cleaned_count !== 5) assert(false, 'Repeated WASM deobfuscation remained deterministic');
    }
    assert(true, 'Repeated WASM allocator/deallocator stress cycle (50x) completed cleanly');

    // Empty / non-QIR rejection & two-circuit /api/verify & URL-safe base64 & default content-type binary .bc
    let emptyCaught = false;
    try { qirWasmDisassemble('   '); } catch { emptyCaught = true; }
    assert(emptyCaught, 'WASM disassembler rejects whitespace-only IR');

    let garbageCaught = false;
    try { qirWasmVerify('hello world'); } catch { garbageCaught = true; }
    assert(garbageCaught, 'WASM verifier rejects non-QIR arbitrary text instead of proving I == I');

    const curlStyleBcRes = await worker.fetch(new Request('https://edge.quantum.internal/api/analyze', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: bellBcBytes
    }));
    const curlStyleBcJson = await curlStyleBcRes.json();
    assert(curlStyleBcRes.status === 200 && curlStyleBcJson.instructionsCount === 5, 'Worker preserves raw binary .bc bytes even with curl default Content-Type');

    const urlSafeB64 = Buffer.from(obfBcBytes).toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    const deobfUrlB64Res = await worker.fetch(new Request('https://edge.quantum.internal/api/deobfuscate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bitcodeBase64: urlSafeB64 })
    }));
    const deobfUrlB64Json = await deobfUrlB64Res.json();
    assert(
        deobfUrlB64Res.status === 200 &&
        deobfUrlB64Json.cleanCount === 5 &&
        deobfUrlB64Json.liveQubits === 2,
        'Worker handles unpadded URL-safe base64 and returns liveQubits'
    );
    const deobfDirect = qirWasmDeobfuscate(obfCode);
    assert(
        typeof deobfDirect.clean_disassembly === 'string' && deobfDirect.clean_disassembly.includes('OFFSET'),
        'WASM deobfuscator includes clean_disassembly for Deobfuscation viewport'
    );

    const pagesBlockedRes = await worker.fetch(
        new Request('https://edge.quantum.internal/server.js'),
        { ASSETS: { fetch: () => new Response('leaked', { status: 200 }) } }
    );
    assert(pagesBlockedRes.status === 404, 'Worker ASSETS routing blocks internal files (server.js -> 404)');

    const verifyPairRes = await worker.fetch(new Request('https://edge.quantum.internal/api/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: obfCode, cleanCode: cleanCode, numQubits: 2 })
    }));
    const verifyPairJson = await verifyPairRes.json();
    assert(verifyPairRes.status === 200 && verifyPairJson.verification.is_equivalent === true, 'POST /api/verify supports explicit cleanCode comparison');

    // Live Node.js HTTP server.js binary .bc, CSP, blocked file, & HEAD parity test
    const { server } = require('./server.js');
    try {
        await new Promise((resolve, reject) => {
            server.once('error', reject);
            server.listen(0, '127.0.0.1', resolve);
        });
        const addr = server.address();
        const baseUrl = `http://127.0.0.1:${addr.port}`;

        const httpHome = await fetch(`${baseUrl}/`);
        const csp = httpHome.headers.get('content-security-policy') || '';
        assert(httpHome.status === 200 && csp.includes("script-src 'self' 'wasm-unsafe-eval'"), 'Node server.js serves / with CSP header');

        const headRes = await fetch(`${baseUrl}/index.html`, { method: 'HEAD' });
        assert(headRes.status === 200 && (await headRes.text()) === '', 'Node server HEAD /index.html returns 200 without body');

        const httpBlocked = await fetch(`${baseUrl}/server.js`);
        assert(httpBlocked.status === 404, 'Node server.js blocks internal source files (404)');

        const octFullRes = await fetch(`${baseUrl}/api/full`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/octet-stream' },
            body: obfBcBytes,
        });
        const octFullJson = await octFullRes.json();
        const apiCsp = octFullRes.headers.get('content-security-policy') || '';
        const apiXfo = octFullRes.headers.get('x-frame-options') || '';
        assert(
            octFullRes.status === 200 &&
            octFullJson.deobfuscation.originalCount === 18 &&
            octFullJson.deobfuscation.cleanCount === 5 &&
            octFullJson.verification.is_equivalent === true &&
            apiCsp.includes("frame-ancestors 'none'") &&
            apiXfo === 'DENY',
            'Node server.js POST /api/full preserved raw binary .bc bytes and CSP/XFO headers'
        );

        // Simulate `curl --data-binary @file.bc` default Content-Type (application/x-www-form-urlencoded)
        const curlFullRes = await fetch(`${baseUrl}/api/full`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: obfBcBytes,
        });
        const curlFullJson = await curlFullRes.json();
        assert(
            curlFullRes.status === 200 &&
            curlFullJson.verification.is_equivalent === true,
            'Node server.js & worker detected BC magic without application/octet-stream header'
        );
    } finally {
        await new Promise(resolve => server.close(() => resolve()));
    }

    let badBcCaught = false;
    try { qirWasmDecodeBitcode(new Uint8Array([1, 2, 3, 4])); } catch { badBcCaught = true; }
    assert(badBcCaught, 'Pure-Rust WASM rejects invalid binary bitcode header cleanly');

    // 6. Reactive UI Logic Verification
    console.log('\n6. REACTIVE ANALYZE & UI STATE VALIDATION');
    function evaluateAnalyzeState(text) {
        const nonCommentLines = (text || '').split('\n').filter(l => l.trim() && !l.trim().startsWith(';'));
        return nonCommentLines.length > 0;
    }
    assert(evaluateAnalyzeState('') === false, 'Empty buffer keeps Analyze button disabled');
    assert(evaluateAnalyzeState('; Single comment') === false, 'Single comment line keeps Analyze button disabled');
    assert(evaluateAnalyzeState('; Comment 1\n; Comment 2\n   ; Indented comment') === false, 'Multiple comment lines keep Analyze button disabled');
    assert(evaluateAnalyzeState('; Paste LLVM QIR assembly here, drag & drop a file, or click Load QIR...') === false, 'Default placeholder keeps Analyze button disabled');
    assert(evaluateAnalyzeState('call void @__quantum__qis__h__body(%Qubit* null)') === true, 'Valid instruction enables Analyze button');
    assert(evaluateAnalyzeState('; Header\ncall void @__quantum__qis__h__body(%Qubit* null)\n; Footer') === true, 'Instructions mixed with comments enable Analyze button');

    // 7. Platform Auto-Detection & Install Command Validation
    console.log('\n7. PLATFORM AUTO-DETECTION & INSTALL COMMAND VALIDATION');
    const platformCommands = {
        linux: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        windows: 'irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex',
        macos: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        cargo: 'cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp'
    };

    function detectPlatformMock(ua, plat) {
        const u = (ua || '').toLowerCase();
        const p = (plat || '').toLowerCase();
        if (p.includes('win') || u.includes('windows')) return 'windows';
        if (p.includes('mac') || u.includes('macintosh') || u.includes('mac os')) return 'macos';
        return 'linux';
    }

    assert(detectPlatformMock('Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32') === 'windows', 'Windows user agent detects windows');
    assert(platformCommands[detectPlatformMock('Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32')].includes('install.ps1'), 'Windows command points to install.ps1');
    assert(detectPlatformMock('Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64') === 'linux', 'Linux user agent detects linux');
    assert(platformCommands[detectPlatformMock('Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64')].includes('install.sh'), 'Linux command points to install.sh');
    assert(detectPlatformMock('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', 'MacIntel') === 'macos', 'macOS user agent detects macos');
    assert(platformCommands[detectPlatformMock('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', 'MacIntel')].includes('install.sh'), 'macOS command points to install.sh');

    console.log('\n════════════════════════════════════════════════════════════════');
    console.log('[SUCCESS] ALL TESTS PASSED.');
    console.log('════════════════════════════════════════════════════════════════\n');
}

runAllTests().catch(err => {
    console.error('[ERROR]', err);
    process.exit(1);
});
