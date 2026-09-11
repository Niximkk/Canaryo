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
    if (request.url === "/headers") {
        response.statusCode = 201;
        response.statusMessage = "Canaryo Created";
        response.setHeader("X-Removed", "yes");
        response.removeHeader("X-Removed");
        response.setHeader("X-Present", "yes");
        response.flushHeaders();
        response.end(String(response.getHeaderNames().includes("x-present")));
        return;
    }

    response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Hello from Canaryo!");
}).listen(port);
