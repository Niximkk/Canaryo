"use strict";

const http = require("node:http");
const port = Number(process.argv[2]);
const upstreamPort = Number(process.argv[3]);

http.createServer((_request, response) => {
    const outbound = http.get({
        hostname: "127.0.0.1",
        port: upstreamPort,
        path: "/source?value=42",
        headers: { "x-canaryo-client": "yes" }
    }, upstreamResponse => {
        const chunks = [];
        upstreamResponse.on("data", chunk => chunks.push(chunk));
        upstreamResponse.on("end", () => {
            response.statusCode = upstreamResponse.statusCode;
            response.setHeader("x-upstream", upstreamResponse.headers["x-upstream"]);
            response.end(Buffer.concat(chunks));
        });
    });
    outbound.on("error", error => {
        response.statusCode = 500;
        response.end(error.message);
    });
}).listen(port);
