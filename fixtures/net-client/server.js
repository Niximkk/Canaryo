"use strict";

const http = require("node:http");
const net = require("node:net");
const port = Number(process.argv[2]);
const upstreamPort = Number(process.argv[3]);

http.createServer((_request, response) => {
    const chunks = [];
    const socket = net.createConnection({ host: "127.0.0.1", port: upstreamPort }, () => {
        socket.end("ping");
    });
    socket.on("data", chunk => chunks.push(chunk));
    socket.on("end", () => {
        response.end(JSON.stringify({
            body: Buffer.concat(chunks).toString(),
            socket: socket instanceof net.Socket,
            remoteAddress: socket.remoteAddress,
            remotePort: socket.remotePort,
            bytesRead: socket.bytesRead,
            bytesWritten: socket.bytesWritten
        }));
    });
    socket.on("error", error => {
        response.statusCode = 502;
        response.end(error.message);
    });
}).listen(port);
