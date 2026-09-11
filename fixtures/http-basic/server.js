const http = require("node:http");
const port = Number(process.argv[2] || 3000);

http.createServer((request, response) => {
    if (request.url === "/echo") {
        const chunks = [];
        request.on("data", chunk => chunks.push(chunk));
        request.on("end", () => {
            response.setHeader("X-Request-Trailer", request.trailers["x-checksum"] || "none");
            response.end(Buffer.concat(chunks));
        });
        return;
    }
    if (request.url === "/binary") {
        response.setHeader("Content-Type", "application/octet-stream");
        response.end(Buffer.from([0, 255, 128, 65]));
        return;
    }

    response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Hello from Canaryo!");
}).listen(port);
