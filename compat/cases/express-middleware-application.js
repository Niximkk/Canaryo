const path = require("node:path");
const http = require("node:http");
const zlib = require("node:zlib");
const express = require("../../fixtures/express-basic/node_modules/express");
const compression = require("../../fixtures/express-basic/node_modules/compression");

const port = Number(process.env.CANARYO_COMPAT_PORT);
const app = express();
const payload = "canaryo-compression-".repeat(100);

app.use(compression());
app.use("/static", express.static(path.join(__dirname, "../../fixtures/express-static/public")));
app.get("/compressed", (_request, response) => response.json({ payload }));

function request(pathname, headers = {}) {
    return new Promise((resolve, reject) => {
        const request = http.request({
            host: "127.0.0.1",
            port,
            path: pathname,
            headers,
            agent: false
        }, response => {
            const chunks = [];
            response.on("data", chunk => chunks.push(chunk));
            response.on("end", () => resolve({
                status: response.statusCode,
                headers: response.headers,
                body: Buffer.concat(chunks)
            }));
        });
        request.on("error", reject);
        request.end();
    });
}

const server = app.listen(port, "127.0.0.1", async () => {
    try {
        const compressed = await request("/compressed", { "accept-encoding": "gzip" });
        const decoded = JSON.parse(zlib.gunzipSync(compressed.body).toString());
        const staticFile = await request("/static/hello.txt");

        server.close(() => {
            console.log(JSON.stringify({
                compression: {
                    status: compressed.status,
                    encoding: compressed.headers["content-encoding"],
                    payloadMatches: decoded.payload === payload
                },
                staticFile: {
                    status: staticFile.status,
                    contentType: staticFile.headers["content-type"],
                    body: staticFile.body.toString()
                }
            }));
        });
    } catch (error) {
        console.error(error);
        process.exitCode = 1;
        server.close();
    }
});
