const http = require("node:http");
const port = Number(process.argv[2] || 3004);
const server = http.createServer((_request, response) => response.end("ok"));

server.listen(port);
setTimeout(() => server.close(() => console.log("closed")), 25);
