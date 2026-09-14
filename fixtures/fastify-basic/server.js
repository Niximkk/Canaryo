const fastify = require("fastify")({ logger: false });
const { ReadableStream } = require("node:stream/web");
const port = Number(process.argv[2] || 3002);
let completedResponses = 0;

fastify.get("/", async (_request, _reply) => {
    return { runtime: "canaryo", framework: "fastify", status: "ok" };
});

fastify.get("/users/:id", async request => {
    return { id: request.params.id, active: request.query.active };
});

fastify.post("/echo", async request => {
    return { body: request.body, contentType: request.headers["content-type"] };
});

fastify.get("/delayed", async () => {
    await new Promise(resolve => setTimeout(resolve, 20));
    return { delayed: true };
});

fastify.get("/web-stream", () => new ReadableStream({
    start(controller) {
        controller.enqueue(Buffer.from("can"));
        controller.enqueue(new TextEncoder().encode("aryo"));
        controller.close();
    }
}));

fastify.get("/hooks", {
    onRequest: async (_request, reply) => {
        reply.header("x-canaryo-hook", "on-request");
    },
    onResponse: async () => {
        completedResponses += 1;
    }
}, async () => ({ completedResponses }));

fastify.listen({ port, host: "127.0.0.1" });
