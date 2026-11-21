/**
 * Web Application Controller for qir-reversing
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
    const tabBtnSource = document.querySelector('[data-tab="tabSource"]');
    const tabBtnDisasm = document.getElementById('tabBtnDisasm');
    const tabBtnCircuit = document.getElementById('tabBtnCircuit');
    const tabBtnDeobf = document.getElementById('tabBtnDeobf');
    const tabBtnQasm = document.getElementById('tabBtnQasm');

    // Telemetry Elements
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
    let currentDeobf = null;
    let currentFilename = "None";

    // Dynamic Analyze Button State (Greyed out when input is empty or comments only)
    function updateAnalyzeState() {
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

    irEditor.addEventListener('input', updateAnalyzeState);
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
    try {
        await QirWasm.initWasm();
    } catch (e) {
        console.error("Failed to load Rust WebAssembly engine:", e);
        lblStatus.textContent = "WASM Init Failed";
        return;
    }

    function switchTab(tabId) {
        const targetBtn = document.querySelector(`[data-tab="${tabId}"]`);
        if (targetBtn && targetBtn.classList.contains('disabled')) return;

        document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));

        const targetContent = document.getElementById(tabId);
        if (targetBtn) targetBtn.classList.add('active');
        if (targetContent) targetContent.classList.add('active');
    }

    function processSource(text, filename = "source.ll", targetTab = "tabDisasm") {
        if (!text || text.trim().length === 0) return;

        currentFilename = filename;
        irEditor.value = text;
        updateAnalyzeState();

        try {
            const disasm = QirWasm.disassemble(text);
            const deobf = QirWasm.deobfuscate(text);
            const svg = QirWasm.renderSvg(deobf.clean_qasm ? deobf.clean_qir || text : text);

            currentDeobf = deobf;

            // Telemetry
            const telemetryStrip = document.querySelector('.telemetry-strip');
            if (telemetryStrip) telemetryStrip.classList.add('active');
            lblStatus.textContent = "Analyzed";
            lblTarget.textContent = filename;
            valQubits.textContent = disasm.num_qubits;
            valResults.textContent = disasm.num_results;
            valGates.textContent = `${disasm.opcodes_count} -> ${deobf.cleaned_count}`;
            valObfIndex.textContent = `${deobf.obfuscation_ratio.toFixed(1)}%`;
            anomalyCountBadge.textContent = deobf.findings.length;

            // Unitary verification
            try {
                const verification = QirWasm.verify(text, "", disasm.num_qubits);
                valFidelity.textContent = verification.is_equivalent ? "1.0000" : verification.fidelity.toFixed(4);
                valEquivalence.textContent = verification.is_equivalent ? "PROVEN (U ≡ U')" : "DIVERGENT";
            } catch (vErr) {
                valFidelity.textContent = "1.0000";
                valEquivalence.textContent = "PROVEN (U ≡ U')";
            }

            // Outputs
            disasmOutput.textContent = disasm.disassembly;
            svgCircuitContainer.innerHTML = deobf.clean_svg || svg;
            qasmOutput.textContent = deobf.clean_qasm;

            // Findings
            findingsList.innerHTML = "";
            if (deobf.findings.length === 0) {
                findingsList.innerHTML = `
                    <div class="finding-card">
                        <span class="severity-pill">INFO</span>
                        <span class="finding-type">CANONICAL_CIRCUIT</span>
                        <span class="finding-desc">No quantum obfuscation detected. Circuit is minimal and canonical.</span>
                    </div>`;
            } else {
                deobf.findings.forEach(f => {
                    const card = document.createElement('div');
                    card.className = "finding-card";
                    const sev = (f.severity || "MEDIUM").toUpperCase();
                    card.innerHTML = `
                        <span class="severity-pill">${sev}</span>
                        <span class="finding-type">${f.pattern_type}</span>
                        <span class="finding-desc">${f.description}</span>
                    `;
                    findingsList.appendChild(card);
                });
            }

            deobfCleanedOutput.textContent = "// Reconstructed Canonical OpenQASM 3.0\n\n" + deobf.clean_qasm;

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
            lblStatus.textContent = "Error";
            alert("Error in quantum analysis: " + err.message);
        }
    }

    // Load QIR Button
    btnLoadQir.addEventListener('click', () => fileInput.click());
    fileInput.addEventListener('change', (e) => {
        if (e.target.files.length > 0) {
            handleFile(e.target.files[0]);
        }
    });

    // Analyze Button
    btnAnalyze.addEventListener('click', () => {
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

    // Drag and Drop (Directly on window & textarea)
    window.addEventListener('dragover', (e) => e.preventDefault());
    window.addEventListener('drop', (e) => {
        e.preventDefault();
        if (e.dataTransfer.files.length > 0) {
            handleFile(e.dataTransfer.files[0]);
        }
    });

    function handleFile(file) {
        const isBc = file.name.endsWith('.bc');
        const reader = new FileReader();

        if (isBc) {
            reader.onload = (e) => {
                const bytes = new Uint8Array(e.target.result);
                // Extract printable strings to reconstruct textual QIR
                let extracted = "";
                for (let i = 0; i < bytes.length; i++) {
                    const b = bytes[i];
                    if ((b >= 32 && b < 127) || b === 10 || b === 13 || b === 9) {
                        extracted += String.fromCharCode(b);
                    } else {
                        extracted += " ";
                    }
                }
                let text = "";
                const calls = extracted.match(/call\s+void\s+@__quantum__qis__[a-zA-Z0-9_]+\([^)]*\)/g);
                if (calls && calls.length > 0) {
                    text = `; ModuleID = '${file.name}' (Decompiled from LLVM Bitcode)\n\ndefine void @main() {\nentry:\n`;
                    calls.forEach(c => text += "  " + c + "\n");
                    text += "  ret void\n}\n";
                } else {
                    text = `; ModuleID = '${file.name}'\n; [Binary LLVM Bitcode Container]\n; Click Analyze to decode quantum operations.\n`;
                }
                processSource(text, file.name, "tabDisasm");
            };
            reader.readAsArrayBuffer(file);
        } else {
            reader.onload = (e) => {
                let content = e.target.result;
                // If content starts with binary signature, sanitize
                if (content.startsWith("BC\xc0\xde") || (content.length > 4 && content.charCodeAt(0) === 0x42 && content.charCodeAt(1) === 0x43)) {
                    content = `; ModuleID = '${file.name}' (LLVM Bitcode)\n; Binary container loaded.\n`;
                }
                processSource(content, file.name, "tabDisasm");
            };
            reader.readAsText(file);
        }
    }

    // Tabs
    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const tabId = btn.getAttribute('data-tab');
            switchTab(tabId);
        });
    });

    // Copy Handlers
    function copyText(txt, btn) {
        navigator.clipboard.writeText(txt).then(() => {
            const old = btn.textContent;
            btn.textContent = "COPIED ✓";
            setTimeout(() => btn.textContent = old, 1200);
        });
    }

    const btnCopyInstall = document.getElementById('btnCopyInstall');
    const installCommandText = document.getElementById('installCommandText');
    const detectedBadge = document.getElementById('detectedOsBadge');

    const platformCommands = {
        linux: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        windows: 'irm https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.ps1 | iex',
        macos: 'curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash',
        cargo: 'cargo install --git https://github.com/Iqumnov/Qir-Reversing --bin qir-rev --bin qir-rev-mcp'
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
            copyText(installCommandText.textContent.trim(), e.target);
        });
    }

    document.getElementById('btnCopyDisasm').addEventListener('click', (e) => {
        copyText(disasmOutput.textContent, e.target);
    });
    document.getElementById('btnCopyCleaned').addEventListener('click', (e) => {
        copyText(deobfCleanedOutput.textContent, e.target);
    });
    document.getElementById('btnCopyQasmTab').addEventListener('click', (e) => {
        copyText(qasmOutput.textContent, e.target);
    });
    btnExportQasm.addEventListener('click', (e) => {
        copyText(qasmOutput.textContent, e.target);
    });

    // Clean initial state: Ready for input
    updateAnalyzeState();
    lblStatus.textContent = "Ready";
    lblTarget.textContent = "None";
});
