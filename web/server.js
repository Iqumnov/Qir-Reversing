const http = require('http');
const fs = require('fs');
const path = require('path');
const qirWasm = require('./js/qir_wasm.js');
const { createWorkerHandler } = require('./worker-core.js');

const worker = createWorkerHandler(qirWasm);

const PORT = process.env.PORT || 8088;
const WEB_ROOT = path.resolve(__dirname);
const MAX_POST_BYTES = 16 * 1024 * 1024;
const BLOCKED_FILES = new Set([
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
const MIME_TYPES = {
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.js': 'application/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.webmanifest': 'application/manifest+json; charset=utf-8',
    '.svg': 'image/svg+xml',
    '.png': 'image/png',
    '.xml': 'application/xml; charset=utf-8',
    '.ico': 'image/x-icon',
    '.txt': 'text/plain; charset=utf-8',
    '.wasm': 'application/wasm',
};

const SECURITY_HEADERS = {
    'Content-Security-Policy': "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
    'X-Content-Type-Options': 'nosniff',
    'X-Frame-Options': 'DENY',
    'Referrer-Policy': 'strict-origin-when-cross-origin',
};

const server = http.createServer(async (req, res) => {
    let url;
    try {
        const host = req.headers.host || `localhost:${PORT}`;
        url = new URL(req.url || '/', `http://${host}`);
    } catch {
        res.writeHead(400, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
        res.end('400 Bad Request');
        return;
    }

    if (url.pathname === '/api' || url.pathname.startsWith('/api/')) {
        // Collect raw binary body when present so .bc uploads remain byte-exact.
        let body = null;
        if (['POST', 'PUT', 'PATCH'].includes(req.method)) {
            const chunks = [];
            let totalBytes = 0;
            try {
                for await (const chunk of req) {
                    totalBytes += chunk.length;
                    if (totalBytes > MAX_POST_BYTES) {
                        res.writeHead(413, { 'Content-Type': 'application/json', ...SECURITY_HEADERS });
                        res.end(JSON.stringify({ error: 'Payload Too Large' }));
                        req.destroy();
                        return;
                    }
                    chunks.push(chunk);
                }
            } catch (err) {
                if (!res.headersSent) {
                    res.writeHead(400, { 'Content-Type': 'application/json', ...SECURITY_HEADERS });
                    res.end(JSON.stringify({ error: 'Request stream interrupted', details: err.message }));
                }
                return;
            }
            body = Buffer.concat(chunks);
        }

        const workerReq = new Request(url.href, {
            method: req.method,
            headers: req.headers,
            body: body !== null ? body : undefined,
        });

        try {
            const workerRes = await worker.fetch(workerReq);
            res.writeHead(workerRes.status, {
                ...SECURITY_HEADERS,
                ...Object.fromEntries(workerRes.headers.entries()),
            });
            if (req.method === 'HEAD') {
                res.end();
                return;
            }
            const outBuf = Buffer.from(await workerRes.arrayBuffer());
            res.end(outBuf);
        } catch (err) {
            if (!res.headersSent) {
                res.writeHead(500, { 'Content-Type': 'application/json', ...SECURITY_HEADERS });
                res.end(JSON.stringify({ error: err.message }));
            }
        }
        return;
    }

    if (req.method !== 'GET' && req.method !== 'HEAD') {
        res.writeHead(405, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
        res.end('405 Method Not Allowed');
        return;
    }

    let decodedPath;
    try {
        decodedPath = decodeURIComponent(url.pathname);
    } catch {
        res.writeHead(400, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
        res.end('400 Bad Request');
        return;
    }

    const relPath = decodedPath === '/' ? 'index.html' : decodedPath.replace(/^\/+/, '');
    const isHiddenSegment = relPath.split('/').some(seg => seg.startsWith('.'));
    if (isHiddenSegment || BLOCKED_FILES.has(path.basename(relPath).toLowerCase())) {
        res.writeHead(404, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
        res.end('404 Not Found');
        return;
    }
    const filePath = path.resolve(WEB_ROOT, relPath);
    if (filePath !== WEB_ROOT && !filePath.startsWith(WEB_ROOT + path.sep)) {
        res.writeHead(403, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
        res.end('403 Forbidden');
        return;
    }

    fs.stat(filePath, (err, stats) => {
        if (err || !stats.isFile()) {
            res.writeHead(404, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
            res.end('404 Not Found');
            return;
        }

        const ext = path.extname(filePath).toLowerCase();
        const contentType = MIME_TYPES[ext] || 'application/octet-stream';
        res.writeHead(200, { 'Content-Type': contentType, ...SECURITY_HEADERS });
        if (req.method === 'HEAD') {
            res.end();
            return;
        }
        const stream = fs.createReadStream(filePath);
        stream.on('error', () => {
            if (!res.headersSent) {
                res.writeHead(500, { 'Content-Type': 'text/plain', ...SECURITY_HEADERS });
            }
            res.end();
        });
        stream.pipe(res);
    });
});

if (require.main === module) {
    server.on('error', (err) => {
        console.error(`Failed to start server on port ${PORT}: ${err.message}`);
        process.exit(1);
    });

    server.listen(PORT, '0.0.0.0', () => {
        console.log(`QIR Reversing Web Dashboard & Edge API running at http://localhost:${PORT}`);
    });
}

module.exports = { server, PORT };

