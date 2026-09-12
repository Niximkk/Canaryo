"use strict";

const http = require("node:http");
const port = Number(process.argv[2]);
const upstreamPort = Number(process.argv[3]);
let pooledAgent;
let separateAgent;
let limitedAgent;

function readOutbound(agent, path, callback) {
    const request = http.get({
        hostname: "127.0.0.1",
        port: upstreamPort,
        path,
        agent
    }, upstreamResponse => {
        const chunks = [];
        upstreamResponse.on("data", chunk => chunks.push(chunk));
        upstreamResponse.on("end", () => callback(null, {
            statusCode: upstreamResponse.statusCode,
            body: Buffer.concat(chunks).toString()
        }, request));
    });
    request.on("error", error => callback(error));
}

http.createServer((incomingRequest, response) => {
    if (incomingRequest.url === "/health") {
        response.end("healthy");
        return;
    }
    if (incomingRequest.url === "/agent") {
        pooledAgent ||= new http.Agent({ keepAlive: true, maxFreeSockets: 1 });
        separateAgent ||= new http.Agent({ keepAlive: true, maxFreeSockets: 1 });
        readOutbound(pooledAgent, "/agent/one", (firstError, first) => {
            if (firstError) return response.end(firstError.message);
            readOutbound(pooledAgent, "/agent/two", (secondError, second, secondRequest) => {
                if (secondError) return response.end(secondError.message);
                readOutbound(separateAgent, "/agent/three", (thirdError, third) => {
                    if (thirdError) return response.end(thirdError.message);
                    const correctAgent = secondRequest.agent === pooledAgent;
                    const name = pooledAgent.getName({ hostname: "127.0.0.1", port: upstreamPort });
                    response.end(`${first.body}${second.body}${third.body}:${correctAgent}:${name}`);
                });
            });
        });
        return;
    }
    if (incomingRequest.url === "/redirect") {
        readOutbound(false, "/redirect/source", (error, result) => {
            if (error) return response.end(error.message);
            response.end(`${result.statusCode}:${result.body}`);
        });
        return;
    }
    if (incomingRequest.url === "/agent-limit") {
        limitedAgent ||= new http.Agent({ keepAlive: true, maxSockets: 1, maxTotalSockets: 1 });
        const bodies = [];
        const finish = (error, result) => {
            if (error) return response.end(error.message);
            bodies.push(result.body);
            if (bodies.length === 2) response.end(bodies.sort().join(""));
        };
        readOutbound(limitedAgent, "/limit/one", finish);
        readOutbound(limitedAgent, "/limit/two", finish);
        return;
    }
    if (incomingRequest.url === "/upload") {
        const outbound = http.request({
            hostname: "127.0.0.1",
            port: upstreamPort,
            path: "/upload",
            method: "POST",
            headers: { "content-length": "11" }
        }, upstreamResponse => {
            const chunks = [];
            upstreamResponse.on("data", chunk => chunks.push(chunk));
            upstreamResponse.on("end", () => response.end(Buffer.concat(chunks)));
        });
        outbound.on("error", error => {
            response.statusCode = 500;
            response.end(error.message);
        });
        outbound.write("hello ");
        setTimeout(() => outbound.end("world"), 400);
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
