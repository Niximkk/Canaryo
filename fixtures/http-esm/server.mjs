import { createServer, maxHeaderSize, validateHeaderName, validateHeaderValue } from "node:http";
import { handleRequest } from "./handler.mjs";

const port = Number(process.argv[2] || 3003);
let ready = false;
let headerHelpersReady = maxHeaderSize === 16384;
try {
    validateHeaderName("x-canaryo");
    validateHeaderValue("x-canaryo", "ready");
    validateHeaderName("invalid header");
    headerHelpersReady = false;
} catch (error) {
    headerHelpersReady = headerHelpersReady && error.code === "ERR_INVALID_HTTP_TOKEN";
}
try {
    validateHeaderValue("x-canaryo", "invalid\nvalue");
    headerHelpersReady = false;
} catch (error) {
    headerHelpersReady = headerHelpersReady && error.code === "ERR_INVALID_CHAR";
}
const server = createServer((request, response) => handleRequest(request, response, ready));
await new Promise(resolve => server.listen(port, resolve));
ready = headerHelpersReady;
