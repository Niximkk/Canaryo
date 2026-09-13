const http = require("node:http");
const port = Number(process.argv[2]);

http.createServer((request, response) => {
    let received = 0;
    let queuedWhilePaused = null;
    request.on("data", chunk => {
        received += chunk.length;
        if (received !== chunk.length) return;
        request.pause();
        if (request.url === "/inspect") {
            setTimeout(() => {
                queuedWhilePaused = request.__canaryoChunkQueue?.length || 0;
                request.resume();
            }, 100);
        } else {
            setTimeout(() => request.resume(), 50);
        }
    });
    request.on("end", () => response.end(String(queuedWhilePaused ?? received)));
}).listen(port);
