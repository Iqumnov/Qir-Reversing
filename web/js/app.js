/**
 * Web Application Controller for QIR Reversing
 * 100% Pure Rust WebAssembly (wasm32-unknown-unknown)
 * Author: Matvei Igumnov <matvei@iqumnov.com>
 */

document.addEventListener('DOMContentLoaded', async () => {
    // Toolbar Elements
    const btnInfo = document.getElementById('btnInfo');
    const btnLoadQir = document.getElementById('btnLoadQir');
    const fileInput = document.getElementById('fileInput');
    const btnAnalyze = document.getElementById('btnAnalyze');
    const btnExportQasm = document.getElementById('btnExportQasm');
    const irEditor = document.getElementById('irEditor');

    // Modal Elements
    const infoModal = document.getElementById('infoModal');
    const btnModalOk = document.getElementById('btnModalOk');

    // Tab Buttons
    const tabBtnDisasm = document.getElementById('tabBtnDisasm');
    const tabBtnCircuit = document.getElementById('tabBtnCircuit');
    const tabBtnDeobf = document.getElementById('tabBtnDeobf');
    const tabBtnQasm = document.getElementById('tabBtnQasm');

    // Telemetry Elements
    const telemetryStrip = document.querySelector('.telemetry-strip');
    const lblStatus = document.getElementById('lblStatus');
    const lblTarget = document.getElementById('lblTarget');
    const valQubits = document.getElementById('valQubits');
    const valResults = document.getElementById('valResults');
    const valGates = document.getElementById('valGates');
    const valObfIndex = document.getElementById('valObfIndex');
    const valFidelity = document.getElementById('valFidelity');
    const valEquivalence = document.getElementById('valEquivalence');
    const anomalyCountBadge = document.getElementById('anomalyCountBadge');

    // Outputs
    const disasmOutput = document.getElementById('disasmOutput');
    const svgCircuitContainer = document.getElementById('svgCircuitContainer');
    const findingsList = document.getElementById('findingsList');
    const deobfCleanedOutput = document.getElementById('deobfCleanedOutput');
    const qasmOutput = document.getElementById('qasmOutput');

    // State
    let currentFilename = "None";

    // Dynamic Analyze Button State (Greyed out when WASM failed or input is empty/comments-only)
    function updateAnalyzeState() {
        if (!wasmReady) {
            btnAnalyze.classList.add('disabled');
            btnAnalyze.disabled = true;
            return;
        }
        const text = irEditor.value.trim();
        const nonCommentLines = text.split('\n').filter(l => l.trim() && !l.trim().startsWith(';'));
        const hasContent = nonCommentLines.length > 0;
        if (hasContent) {
            btnAnalyze.classList.remove('disabled');
            btnAnalyze.disabled = false;
        } else {
            btnAnalyze.classList.add('disabled');
            btnAnalyze.disabled = true;
        }
    }

    function looksLikeLlvmQir(text) {
        for (const rawLine of text.split('\n')) {
            const code = rawLine.split(';')[0].trim();
            if (!code) continue;
            if (
                code.includes('@__quantum__') ||
                code.startsWith('define ') ||
                code.startsWith('declare ') ||
                code.startsWith('attributes ') ||
                code.startsWith('target ') ||
                code.startsWith('source_filename') ||
                code.includes(' = call ') ||
                code.includes(' = tail call ') ||
                code.startsWith('call ') ||
                code.startsWith('tail call ') ||
                code.startsWith('ret ') ||
                code === 'ret void' ||
                code.startsWith('inttoptr ')
            ) {
                return true;
            }
        }
        return false;
    }

    function setEmptyMessage(container, text) {
        const div = document.createElement('div');
        div.className = 'empty-state';
        div.textContent = text;
        container.replaceChildren(div);
    }

    function resetResultsUi(statusLabel = "Error") {
        if (telemetryStrip) telemetryStrip.classList.add('active');
        lblStatus.textContent = statusLabel;
        valQubits.textContent = "0";
        valResults.textContent = "0";
        valGates.textContent = "0";
        valObfIndex.textContent = "0.0%";
        valFidelity.textContent = "--";
        valEquivalence.textContent = "--";
        valEquivalence.title = "";
        anomalyCountBadge.textContent = "0";

        disasmOutput.textContent = "; No valid disassembly available.";
        setEmptyMessage(svgCircuitContainer, "; No circuit diagram to display.");
        setEmptyMessage(findingsList, "; No deobfuscation audit available.");
        deobfCleanedOutput.textContent = "; Reconstructed canonical circuit will appear here...";
        qasmOutput.textContent = "; OpenQASM 3.0 output will render here after analysis...";

        btnExportQasm.classList.add('disabled');
        btnExportQasm.disabled = true;
        [tabBtnDisasm, tabBtnCircuit, tabBtnDeobf, tabBtnQasm].forEach(b => {
            if (b) b.classList.add('disabled');
        });
        switchTab("tabSource");
    }

    irEditor.addEventListener('input', () => {
        currentFilename = "custom_source.ll";
        updateAnalyzeState();
    });
    irEditor.addEventListener('change', updateAnalyzeState);

    // Info Modal Handlers (Initialized synchronously on DOM ready)
    function closeModal() {
        if (infoModal) {
            infoModal.classList.remove('show');
            try {
                localStorage.setItem('qir_reversing_intro_seen', 'true');
            } catch (e) { }
        }
    }

    function openModal() {
        if (infoModal) {
            infoModal.classList.add('show');
        }
    }

    if (btnInfo) {
        btnInfo.addEventListener('click', openModal);
    }
    if (btnModalOk) {
        btnModalOk.addEventListener('click', closeModal);
    }
    if (infoModal) {
        infoModal.addEventListener('click', (e) => {
            if (e.target === infoModal) closeModal();
        });
    }
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && infoModal && infoModal.classList.contains('show')) {
            closeModal();
        }
    });

    // Web-only first visit auto-popup: show immediately if not seen
    try {
        const urlParams = new URLSearchParams(window.location.search);
        if (urlParams.get('intro') !== '0' && (urlParams.get('intro') === '1' || !localStorage.getItem('qir_reversing_intro_seen'))) {
            openModal();
        }
    } catch (e) { }

    // Initialize Rust WebAssembly Engine
    let wasmReady = false;
    try {
        await QirWasm.initWasm();
        wasmReady = true;
    } catch (e) {
        console.error("Failed to load Rust WebAssembly engine:", e);
        if (telemetryStrip) telemetryStrip.classList.add('active');
        lblStatus.textContent = "WASM Init Failed";
        btnAnalyze.classList.add('disabled');
        btnAnalyze.disabled = true;
        btnLoadQir.classList.add('disabled');
        btnLoadQir.disabled = true;
        fileInput.disabled = true;
        return;
    }

    function switchTab(tabId) {
        const targetBtn = document.querySelector(`[data-tab="${tabId}"]`);
        if (targetBtn && targetBtn.classList.contains('disabled')) return;

        document.querySelectorAll('.tab-btn').forEach(b => {
            b.classList.remove('active');
            b.setAttribute('aria-selected', 'false');
        });
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));

        const targetContent = document.getElementById(tabId);
        if (targetBtn) {
            targetBtn.classList.add('active');
            targetBtn.setAttribute('aria-selected', 'true');
        }
        if (targetContent) targetContent.classList.add('active');
    }

    function renderSvgSafely(svgString) {
        try {
            const parser = new DOMParser();
            const doc = parser.parseFromString(svgString, 'image/svg+xml');
            const root = doc.documentElement;
            if (root && root.nodeName.toLowerCase() === 'svg' && !doc.querySelector('parsererror')) {
                svgCircuitContainer.replaceChildren(document.importNode(root, true));
                return;
            }
        } catch (e) { }
        setEmptyMessage(svgCircuitContainer, "; Invalid SVG circuit output.");
    }

    function buildFindingCard(severity, patternType, description) {
        const card = document.createElement('div');
        card.className = "finding-card";

        const sevSpan = document.createElement('span');
        sevSpan.className = "severity-pill";
        sevSpan.textContent = severity;

        const typeSpan = document.createElement('span');
        typeSpan.className = "finding-type";
        typeSpan.textContent = patternType;

        const descSpan = document.createElement('span');
        descSpan.className = "finding-desc";
        descSpan.textContent = description;

        card.appendChild(sevSpan);
        card.appendChild(typeSpan);
        card.appendChild(descSpan);
        return card;
    }

    async function ensureWasmReady() {
        if (!wasmReady) return false;
        if (typeof QirWasm.isWasmInitialized === 'function' && !QirWasm.isWasmInitialized()) {
            await QirWasm.initWasm();
        }
        return true;
    }

    async function processSource(text, filename = "source.ll", targetTab = "tabDisasm") {
        const rawText = typeof text === 'string' ? text : '';
        currentFilename = filename;
        irEditor.value = rawText;
        updateAnalyzeState();

        if (!rawText.trim()) {
            resetResultsUi("Empty Input");
            lblTarget.textContent = filename;
            return;
        }

        try {
            if (!(await ensureWasmReady())) return;
            const disasm = QirWasm.disassemble(rawText);
            if (disasm.opcodes_count === 0 && !looksLikeLlvmQir(rawText)) {
                resetResultsUi("No QIR Found");
                lblTarget.textContent = filename;
                alert(`No valid LLVM QIR instructions or declarations found in '${filename}'.`);
                return;
            }

            const deobf = QirWasm.deobfuscate(rawText);
            const liveQubits = deobf.live_qubits ?? disasm.num_qubits;
            const svg = deobf.clean_svg || QirWasm.renderSvg(rawText);

            // Telemetry
            if (telemetryStrip) telemetryStrip.classList.add('active');
            lblStatus.textContent = "Analyzed";
            lblTarget.textContent = filename;
            valQubits.textContent = disasm.num_qubits !== liveQubits && disasm.num_qubits > 0
                ? `${disasm.num_qubits} (${liveQubits} live)`
                : String(disasm.num_qubits);
            valResults.textContent = disasm.num_results;
            valGates.textContent = `${disasm.opcodes_count} -> ${deobf.cleaned_count}`;
            valObfIndex.textContent = `${deobf.obfuscation_ratio.toFixed(1)}%`;
            anomalyCountBadge.textContent = deobf.findings.length;

            // Unitary verification
            let verificationNotice = null;
            if (disasm.opcodes_count === 0) {
                valFidelity.textContent = "N/A";
                valEquivalence.textContent = "NO QUANTUM OPS";
                valEquivalence.title = "Circuit contains zero executable quantum instructions.";
            } else {
                try {
                    const verification = QirWasm.verify(rawText, "", 0);
                    if (verification.error_message) {
                        const isLimit = /limit|exceeds|timeout|node cap|memory budget|node table/i.test(verification.error_message);
                        valFidelity.textContent = "N/A";
                        valEquivalence.textContent = isLimit ? "UNVERIFIED (LIMIT)" : "UNVERIFIED (INVALID)";
                        valEquivalence.title = verification.error_message;
                        verificationNotice = verification.error_message;
                    } else {
                        const method = verification.verification_method || (disasm.num_qubits > 10 ? "Canonical QMDD" : "Dense 2^N");
                        valFidelity.textContent = Number.isFinite(verification.fidelity)
                            ? verification.fidelity.toFixed(4)
                            : "N/A";
                        valEquivalence.textContent = verification.is_equivalent
                            ? `PROVEN (${method})`
                            : `DIVERGENT (${method})`;
                        const froStr = Number.isFinite(verification.frobenius_error)
                            ? Number(verification.frobenius_error).toExponential(3)
                            : "N/A";
                        const uniStr = Number.isFinite(verification.unitarity_error)
                            ? Number(verification.unitarity_error).toExponential(3)
                            : "N/A";
                        valEquivalence.title = `Engine: ${method} | Frobenius error: ${froStr} | Unitarity drift: ${uniStr}`;
                    }
                } catch (vErr) {
                    valFidelity.textContent = "N/A";
                    valEquivalence.textContent = "UNVERIFIED (ERR)";
                    valEquivalence.title = vErr.message;
                    verificationNotice = vErr.message;
                }
            }

            // Outputs
            disasmOutput.textContent = disasm.disassembly;
            renderSvgSafely(svg);
            qasmOutput.textContent = deobf.clean_qasm;

            // Findings
            findingsList.replaceChildren();
            if (verificationNotice) {
                findingsList.appendChild(
                    buildFindingCard("WARNING", "VERIFICATION_NOTICE", verificationNotice)
                );
            }
            if (deobf.findings.length === 0) {
                findingsList.appendChild(
                    buildFindingCard(
                        "INFO",
                        "CANONICAL_CIRCUIT",
                        "No quantum obfuscation detected. Circuit is minimal and canonical."
                    )
                );
            } else {
                deobf.findings.forEach(f => {
                    findingsList.appendChild(
                        buildFindingCard(
                            (f.severity || "MEDIUM").toUpperCase(),
                            f.pattern_type || "",
                            f.description || ""
                        )
                    );
                });
            }

            deobfCleanedOutput.textContent =
                deobf.clean_disassembly ||
                ("// Reconstructed Canonical OpenQASM 3.0\n\n" + deobf.clean_qasm);

            // Enable Export button and result tabs
            btnExportQasm.classList.remove('disabled');
            btnExportQasm.disabled = false;
            [tabBtnDisasm, tabBtnCircuit, tabBtnDeobf, tabBtnQasm].forEach(b => {
                if (b) b.classList.remove('disabled');
            });

            if (targetTab) {
                switchTab(targetTab);
            }
        } catch (err) {
            console.error("Error processing QIR:", err);
            resetResultsUi("Error");
            lblTarget.textContent = filename;
            alert("Error in quantum analysis: " + err.message);
        }
    }

    // Load QIR Button
    btnLoadQir.addEventListener('click', () => {
        if (!wasmReady) return;
        fileInput.click();
    });
    fileInput.addEventListener('change', (e) => {
        if (e.target.files.length > 0) {
            handleFile(e.target.files[0]);
            e.target.value = "";
        }
    });

    // Analyze Button
    btnAnalyze.addEventListener('click', () => {
        if (!wasmReady) return;
        const text = irEditor.value.trim();
        if (!text) {
            alert("No quantum instructions to analyze. Please click Load QIR or paste LLVM assembly code.");
            return;
        }
        const nonCommentLines = text.split('\n').filter(l => l.trim() && !l.trim().startsWith(';'));
        if (nonCommentLines.length === 0) {
            alert("Source buffer contains only comments. Please provide valid LLVM QIR instructions.");
            return;
        }
        processSource(text, currentFilename !== "None" ? currentFilename : "custom_source.ll", "tabDisasm");
    });

    // Drag and Drop (Only intercept file drags when WASM engine is ready)
    window.addEventListener('dragover', (e) => {
        if (wasmReady && e.dataTransfer && Array.from(e.dataTransfer.types || []).includes('Files')) {
            e.preventDefault();
        }
    });
    window.addEventListener('drop', (e) => {
        if (wasmReady && e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files.length > 0) {
            e.preventDefault();
            handleFile(e.dataTransfer.files[0]);
        }
    });

    const MAX_BROWSER_BC_BYTES = 16 * 1024 * 1024;
    function hasBitcodeMagic(buf) {
        if (buf.length >= 4 && buf[0] === 0x42 && buf[1] === 0x43 && buf[2] === 0xc0 && buf[3] === 0xde) {
            return true;
        }
        return buf.length >= 20 && buf[0] === 0xde && buf[1] === 0xc0 && buf[2] === 0x17 && buf[3] === 0x0b;
    }

    function handleFile(file) {
        if (!wasmReady) return;
        const isBcExt = file.name.toLowerCase().endsWith('.bc');
        if (file.size > MAX_BROWSER_BC_BYTES) {
            currentFilename = file.name;
            resetResultsUi("File Too Large");
            lblTarget.textContent = file.name;
            alert(`File '${file.name}' exceeds 16 MiB browser safety limit. Use CLI: qir-rev disasm "${file.name}"`);
            return;
        }
        const reader = new FileReader();
        reader.onerror = () => {
            currentFilename = file.name;
            resetResultsUi("File Read Error");
            lblTarget.textContent = file.name;
            alert(`Failed to read file '${file.name}'.`);
        };
        reader.onload = async (e) => {
            const bytes = new Uint8Array(e.target.result);
            if (isBcExt || hasBitcodeMagic(bytes)) {
                try {
                    await ensureWasmReady();
                    const decoded = QirWasm.decodeBitcode(bytes);
                    await processSource(decoded.ir, file.name, "tabDisasm");
                } catch (err) {
                    const comment = `; ModuleID = '${file.name}'\n; Failed to decode LLVM bitcode in-browser: ${err.message}\n`;
                    irEditor.value = comment;
                    currentFilename = file.name;
                    updateAnalyzeState();
                    resetResultsUi("Bitcode Decode Error");
                    lblTarget.textContent = file.name;
                    alert(`Failed to decode binary LLVM bitcode '${file.name}':\n${err.message}`);
                }
                return;
            }
            let content;
            try {
                content = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
            } catch {
                currentFilename = file.name;
                resetResultsUi("Encoding Error");
                lblTarget.textContent = file.name;
                alert(`File '${file.name}' is neither valid LLVM bitcode (.bc) nor UTF-8 text.`);
                return;
            }
            await processSource(content, file.name, "tabDisasm");
        };
        reader.readAsArrayBuffer(file);
    }

    // Tabs
    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const tabId = btn.getAttribute('data-tab');
            switchTab(tabId);
        });
    });

    // Copy Handlers
    function flashButtonLabel(btn, tempLabel) {
        if (!btn) return;
        if (!btn.dataset.origLabel) {
            btn.dataset.origLabel = btn.textContent;
        }
        if (btn._flashTimer) {
            clearTimeout(btn._flashTimer);
        }
        btn.textContent = tempLabel;
        btn._flashTimer = setTimeout(() => {
            btn.textContent = btn.dataset.origLabel;
            btn._flashTimer = null;
        }, 1200);
    }

    function fallbackCopyText(txt, btn) {
        try {
            const ta = document.createElement('textarea');
            ta.value = txt;
            ta.style.position = 'fixed';
            ta.style.opacity = '0';
            document.body.appendChild(ta);
            ta.select();
            if (document.execCommand('copy')) {
                flashButtonLabel(btn, "COPIED ✓");
            }
            document.body.removeChild(ta);
        } catch (e) { }
    }

    function copyText(txt, btn) {
        if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
            navigator.clipboard.writeText(txt).then(() => {
                flashButtonLabel(btn, "COPIED ✓");
            }).catch(() => {
                fallbackCopyText(txt, btn);
            });
            return;
        }
        fallbackCopyText(txt, btn);
    }

    const btnCopyInstall = document.getElementById('btnCopyInstall');
    const installCommandText = document.getElementById('installCommandText');
    const detectedBadge = document.getElementById('detectedOsBadge');

    const platformCommands = {
        linux: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        windows: 'irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex',
        macos: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        cargo: 'cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp'
    };

    function detectClientPlatform() {
        if (typeof navigator === 'undefined') return 'linux';
        const ua = (navigator.userAgent || '').toLowerCase();
        const platform = (navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
        if (platform.includes('win') || ua.includes('windows')) return 'windows';
        if (platform.includes('mac') || ua.includes('macintosh') || ua.includes('mac os')) return 'macos';
        return 'linux';
    }

    const detectedPlatform = detectClientPlatform();
    if (detectedBadge) {
        const platformNames = { linux: 'Linux', windows: 'Windows', macos: 'macOS' };
        detectedBadge.textContent = `Detected: ${platformNames[detectedPlatform] || 'Linux'}`;
    }

    function setActivePlatform(platformKey) {
        const command = platformCommands[platformKey] || platformCommands.linux;
        if (installCommandText) {
            installCommandText.textContent = command;
        }
        document.querySelectorAll('.platform-tab-btn').forEach(btn => {
            if (btn.getAttribute('data-platform') === platformKey) {
                btn.classList.add('active');
            } else {
                btn.classList.remove('active');
            }
        });
    }

    document.querySelectorAll('.platform-tab-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const p = btn.getAttribute('data-platform');
            if (p) setActivePlatform(p);
        });
    });

    if (detectedPlatform) {
        setActivePlatform(detectedPlatform);
    }

    if (btnCopyInstall && installCommandText) {
        btnCopyInstall.addEventListener('click', (e) => {
            copyText(installCommandText.textContent.trim(), e.currentTarget);
        });
    }

    document.getElementById('btnCopyDisasm').addEventListener('click', (e) => {
        copyText(disasmOutput.textContent, e.currentTarget);
    });
    document.getElementById('btnCopyCleaned').addEventListener('click', (e) => {
        copyText(deobfCleanedOutput.textContent, e.currentTarget);
    });
    document.getElementById('btnCopyQasmTab').addEventListener('click', (e) => {
        copyText(qasmOutput.textContent, e.currentTarget);
    });
    btnExportQasm.addEventListener('click', (e) => {
        const qasm = qasmOutput.textContent;
        if (!qasm || qasm.startsWith('; OpenQASM 3.0 output')) return;
        const base = (currentFilename && currentFilename !== 'None'
            ? currentFilename.replace(/\.(bc|ll|ir|txt)$/i, '')
            : 'circuit');
        const blob = new Blob([qasm], { type: 'text/plain;charset=utf-8' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${base}.qasm`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        const btn = e.currentTarget;
        flashButtonLabel(btn, "EXPORTED ✓");
    });

    // Clean initial state: Ready for input
    updateAnalyzeState();
    lblStatus.textContent = "Ready";
    lblTarget.textContent = "None";
});
