const http = require("node:http");

const parserLimitResult = http.setMaxIdleHTTPParsers(25);
let parserLimitError;
try {
    http.setMaxIdleHTTPParsers(0);
} catch (error) {
    parserLimitError = { name: error.name, code: error.code };
}
const https = require("node:https");

const httpServer = http.createServer(() => {});
const httpsServer = https.createServer({}, () => {});

console.log(JSON.stringify({
    httpServer: httpServer instanceof http.Server,
    httpsServer: httpsServer instanceof https.Server,
    httpsIsHttpServer: httpsServer instanceof http.Server,
    responseIsOutgoing: http.ServerResponse.prototype instanceof http.OutgoingMessage,
    requestIsOutgoing: http.ClientRequest.prototype instanceof http.OutgoingMessage,
    parserLimit: {
        returnedUndefined: parserLimitResult === undefined,
        error: parserLimitError
    }
}));
