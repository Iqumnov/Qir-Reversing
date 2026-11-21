/**
 * qir-reversing Test Suite
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

const fs = require('fs');
const path = require('path');
const {
    initWasm,
    qirWasmDisassemble,
    qirWasmDeobfuscate,
    qirWasmRenderAscii,
    qirWasmRenderSvg,
    qirWasmVerify,
} = require('./js/qir_wasm.js');
const worker = require('./worker.js');

function assert(condition, message) {
    if (!condition) {
        console.error(`  [FAIL] ${message}`);
        process.exit(1);
    }
    console.log(`  [PASS] ${message}`);
}

async function runAllTests() {
    console.log('════════════════════════════════════════════════════════════════');
    console.log('[TEST SUITE] qir-reversing');
    console.log('════════════════════════════════════════════════════════════════\n');

    await initWasm();

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

    console.log('\n════════════════════════════════════════════════════════════════');
    console.log('[SUCCESS] ALL TESTS PASSED.');
    console.log('════════════════════════════════════════════════════════════════\n');
}

runAllTests().catch(err => {
    console.error('[ERROR]', err);
    process.exit(1);
});
