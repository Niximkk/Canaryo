const http = require("node:http");
const port = Number(process.argv[2]);

const server = http.createServer((_request, response) => {
    server.close(() => process.stdout.write("closed"));
    setTimeout(() => response.end("finished"), 100);
});

server.listen(port);
