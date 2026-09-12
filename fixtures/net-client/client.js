"use strict";

const net = require("node:net");
const port = Number(process.argv[2]);
const chunks = [];
const socket = net.connect(port, "127.0.0.1", () => socket.end("standalone"));

socket.on("data", chunk => chunks.push(chunk));
socket.on("end", () => process.stdout.write(Buffer.concat(chunks).toString()));
socket.on("error", error => {
    process.stderr.write(error.message);
    process.exitCode = 1;
});
