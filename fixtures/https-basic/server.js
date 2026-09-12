const fs = require("node:fs");
const https = require("node:https");

const server = https.createServer(
    {
        cert: fs.readFileSync("fixtures/https-basic/cert.pem"),
        key: fs.readFileSync("fixtures/https-basic/key.pem")
    },
    (request, response) => {
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({
            secure: request.socket.encrypted,
            method: request.method,
            url: request.url
        }));
    }
);

server.listen(Number(process.argv[2]), "127.0.0.1");
