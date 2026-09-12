"use strict";

const net = require("node:net");
const port = Number(process.argv[2]);

const server = net.createServer({ allowHalfOpen: true }, socket => {
    const chunks = [];
    socket.on("data", chunk => chunks.push(chunk));
    socket.on("end", () => {
        const address = server.address();
        socket.end(JSON.stringify({
            body: Buffer.concat(chunks).toString(),
            remoteAddress: socket.remoteAddress,
            localPort: socket.localPort,
            serverPort: address.port
        }));
    });
    socket.on("close", () => server.close());
});

server.listen(port, "127.0.0.1");
