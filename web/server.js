const http = require('http');
const fs = require('fs');
const path = require('path');
const worker = require('./worker.js');

const PORT = process.env.PORT || 8088;
const MIME_TYPES = {
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.js': 'application/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.svg': 'image/svg+xml',
    '.png': 'image/png',
    '.ico': 'image/x-icon',
    '.txt': 'text/plain; charset=utf-8',
    '.wasm': 'application/wasm',
};

const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost:' + PORT}`);

    if (url.pathname.startsWith('/api')) {
        // Collect body if POST
        let body = null;
        if (req.method === 'POST') {
            const chunks = [];
            for await (const chunk of req) chunks.push(chunk);
            body = Buffer.concat(chunks).toString();
        }

        const workerReq = new Request(url.href, {
            method: req.method,
            headers: req.headers,
            body: ['POST', 'PUT', 'PATCH'].includes(req.method) ? body : undefined,
        });

        try {
            const workerRes = await worker.fetch(workerReq);
            res.writeHead(workerRes.status, Object.fromEntries(workerRes.headers.entries()));
            const text = await workerRes.text();
            res.end(text);
        } catch (err) {
            res.writeHead(500, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: err.message }));
        }
        return;
    }

    let filePath = path.join(__dirname, url.pathname === '/' ? 'index.html' : url.pathname);
    fs.stat(filePath, (err, stats) => {
        if (err || !stats.isFile()) {
            res.writeHead(404, { 'Content-Type': 'text/plain' });
            res.end('404 Not Found');
            return;
        }

        const ext = path.extname(filePath).toLowerCase();
        const contentType = MIME_TYPES[ext] || 'application/octet-stream';
        res.writeHead(200, { 'Content-Type': contentType });
        fs.createReadStream(filePath).pipe(res);
    });
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`QIR Reversing Web Dashboard & Edge API running at http://localhost:${PORT}`);
});

