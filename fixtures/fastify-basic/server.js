const fastify = require("fastify")({ logger: false });
const port = Number(process.argv[2] || 3002);
let completedResponses = 0;

fastify.get("/", async (_request, _reply) => {
    return { runtime: "canaryo", framework: "fastify", status: "ok" };
});

fastify.get("/users/:id", async request => {
    return { id: request.params.id, active: request.query.active };
});

fastify.get("/hooks", {
    onRequest: async (_request, reply) => {
        reply.header("x-canaryo-hook", "on-request");
    },
    onResponse: async () => {
        completedResponses += 1;
    }
}, async () => ({ completedResponses }));

fastify.listen({ port, host: "127.0.0.1" });
