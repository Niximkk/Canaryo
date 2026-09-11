"use strict";

const https = require("node:https");
const target = process.argv[2] || "https://example.com/";

https.get(target, response => {
    let bytes = 0;
    response.on("data", chunk => {
        bytes += chunk.length;
    });
    response.on("end", () => {
        console.log(JSON.stringify({
            statusCode: response.statusCode,
            contentType: response.headers["content-type"],
            bytes
        }));
    });
}).on("error", error => {
    throw error;
});
