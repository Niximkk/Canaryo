const http = require("node:http");
const https = require("node:https");

const httpServer = http.createServer(() => {});
const httpsServer = https.createServer({}, () => {});

console.log(JSON.stringify({
    httpServer: httpServer instanceof http.Server,
    httpsServer: httpsServer instanceof https.Server,
    httpsIsHttpServer: httpsServer instanceof http.Server,
    responseIsOutgoing: http.ServerResponse.prototype instanceof http.OutgoingMessage,
    requestIsOutgoing: http.ClientRequest.prototype instanceof http.OutgoingMessage
}));
