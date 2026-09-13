const http = require("node:http");
const port = Number(process.argv[2] || 3000);
let listenReturned = false;

http.createServer((request, response) => {
    if (request.url === "/async-iterate") {
        (async () => {
            const chunks = [];
            for await (const chunk of request) chunks.push(chunk);
            response.end(Buffer.concat(chunks));
        })().catch(error => {
            response.statusCode = 500;
            response.end(error.message);
        });
        return;
    }
    if (request.url === "/first-chunk") {
        request.once("data", chunk => response.end(`first:${chunk.toString()}`));
        return;
    }
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
        response.appendHeader("X-Present", "again");
        response.setHeaders(new Map([["X-Map", "ready"]]));
        response.flushHeaders();
        response.end(String(
            response.getHeaderNames().includes("x-present") &&
            response.getHeader("x-present").join(",") === "yes,again"
        ));
        return;
    }

    response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    response.end(listenReturned ? "Hello from Canaryo!" : "listen did not return");
}).listen(port);

listenReturned = true;
