/**
 * QIR Reversing - Cloudflare Workers Entry Point
 * Uses static build-time WebAssembly.Module import compliant with V8 embedder rules.
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

import wasmModule from './wasm/qir_core.wasm';
import qirWasm from './js/qir_wasm.js';
import workerCore from './worker-core.js';

export default workerCore.createWorkerHandler(qirWasm, wasmModule);
