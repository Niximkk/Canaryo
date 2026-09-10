const http = require("node:http");

const server = http.createServer((request, response) => {
  response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
  response.end("Olá do Canaryo!\n");
});

server.listen(3000, () => {
  console.log("Servidor em http://127.0.0.1:3000");
});