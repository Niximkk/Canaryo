"use strict";

const http = require("node:http");
const port = Number(process.argv[2]);
const upstreamPort = Number(process.argv[3]);

http.createServer((incomingRequest, response) => {
    if (incomingRequest.url === "/health") {
        response.end("healthy");
        return;
    }
    const outbound = http.get({
        hostname: "127.0.0.1",
        port: upstreamPort,
        path: "/source?value=42",
        headers: { "x-canaryo-client": "yes" }
    }, upstreamResponse => {
        const chunks = [];
        upstreamResponse.on("data", chunk => {
            chunks.push(chunk);
            if (incomingRequest.url === "/stream" && chunks.length === 1) {
                upstreamResponse.pause();
                setTimeout(() => upstreamResponse.resume(), 25);
            }
        });
        upstreamResponse.on("end", () => {
            if (incomingRequest.url === "/stream") {
                response.end(`${chunks.length}:${Buffer.concat(chunks).length}`);
                return;
            }
            response.statusCode = upstreamResponse.statusCode;
            response.setHeader("x-upstream", upstreamResponse.headers["x-upstream"]);
            response.end(Buffer.concat(chunks));
        });
    });
    outbound.on("error", error => {
        response.statusCode = 500;
        response.end(error.message);
    });
    if (incomingRequest.url === "/timeout") {
        outbound.setTimeout(25, () => {
            outbound.abort();
            response.statusCode = 504;
            response.end("outbound-timeout");
        });
    }
}).listen(port);
