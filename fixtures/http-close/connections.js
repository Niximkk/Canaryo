const http = require("node:http");
const port = Number(process.argv[2]);

const server = http.createServer((request, response) => {
    if (request.url === "/close-idle") server.closeIdleConnections();
    if (request.url === "/close-all") server.closeAllConnections();
    response.end("alive");
});

server.listen(port);
