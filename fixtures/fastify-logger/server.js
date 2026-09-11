"use strict";

const fastify = require("../fastify-basic/node_modules/fastify")({ logger: true });
const port = Number(process.argv[2] || 3003);

fastify.get("/", async request => {
    request.log.info({ runtime: "canaryo" }, "logger fixture handled request");
    return { runtime: "canaryo", logger: "pino", status: "ok" };
});

fastify.listen({ port, host: "127.0.0.1" });
