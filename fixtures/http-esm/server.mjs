import { createServer } from "node:http";
import { handleRequest } from "./handler.mjs";

const port = Number(process.argv[2] || 3003);
let ready = false;
const server = createServer((request, response) => handleRequest(request, response, ready));
await new Promise(resolve => server.listen(port, resolve));
ready = true;
