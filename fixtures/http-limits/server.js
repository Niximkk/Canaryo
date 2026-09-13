const http = require("node:http");
const port = Number(process.argv[2]);
let drops = 0;

const server = http.createServer({ maxRequestSize: 8 }, (request, response) => {
    request.resume();
    request.on("end", () => response.end(request.url === "/drops" ? String(drops) : "accepted"));
});

server.maxConnections = 1;
server.on("drop", () => drops += 1);
server.listen(port);
