const http = require("node:http");
const port = Number(process.argv[2] || 3006);

http.createServer(async (request, response) => {
    if (request.url === "/echo") {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({
            contentType: request.headers["content-type"],
            body: Buffer.concat(chunks).toString()
        }));
        return;
    }

    if (request.url === "/send") {
        const form = new FormData();
        form.append("runtime", "canaryo");
        form.append("tags", "rust");
        form.append("tags", "javascript");
        form.append("asset", new File(["binary-data"], "canaryo.txt", { type: "text/plain" }));
        const result = await fetch(`http://127.0.0.1:${port}/echo`, { method: "POST", body: form });
        response.setHeader("content-type", "application/json");
        response.end(await result.text());
        return;
    }

    response.end("ready");
}).listen(port, "127.0.0.1");
