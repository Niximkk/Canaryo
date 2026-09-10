const fastify = require("fastify")({ logger: false });
const port = Number(process.argv[2] || 3002);

fastify.get("/", async (_request, _reply) => {
    return { runtime: "canaryo", framework: "fastify", status: "ok" };
});

fastify.listen({ port, host: "127.0.0.1" });
