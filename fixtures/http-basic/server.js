const http = require("node:http");
const port = Number(process.argv[2] || 3000);

http.createServer((_request, response) => {
    response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Hello from Canaryo!");
}).listen(port);
