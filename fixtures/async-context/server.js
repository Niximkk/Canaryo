const http = require("node:http");
const { AsyncLocalStorage } = require("node:async_hooks");
const port = Number(process.argv[2]);
const storage = new AsyncLocalStorage();

http.createServer((request, response) => {
    storage.run({ path: request.url }, async () => {
        const before = storage.getStore()?.path;
        await new Promise(resolve => setTimeout(resolve, request.url === "/slow" ? 40 : 10));
        const afterTimer = storage.getStore()?.path;
        process.nextTick(() => {
            const insideTick = storage.getStore()?.path;
            Promise.resolve().then(() => {
                const insidePromise = storage.getStore()?.path;
                response.end([before, afterTimer, insideTick, insidePromise].join(":"));
            });
        });
    });
}).listen(port);
