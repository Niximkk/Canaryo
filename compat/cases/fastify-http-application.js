const fastify = require("../../fixtures/fastify-basic/node_modules/fastify")({ logger: false });

const port = Number(process.env.CANARYO_COMPAT_PORT);
let completedResponses = 0;

fastify.addHook("onRequest", async (_request, reply) => {
    reply.header("x-compat-hook", "active");
});
fastify.addHook("onResponse", async () => {
    completedResponses += 1;
});
fastify.get("/users/:id", async request => ({
    id: request.params.id,
    active: request.query.active
}));
fastify.post("/echo", async request => ({ body: request.body }));

(async () => {
    try {
        await fastify.listen({ port, host: "127.0.0.1" });
        const routeResponse = await fetch(`http://127.0.0.1:${port}/users/73?active=true`);
        const route = await routeResponse.json();
        const bodyResponse = await fetch(`http://127.0.0.1:${port}/echo`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ runtime: "canaryo", compatible: true })
        });
        const body = await bodyResponse.json();
        const hook = routeResponse.headers.get("x-compat-hook");
        await fastify.close();

        console.log(JSON.stringify({
            statuses: [routeResponse.status, bodyResponse.status],
            route,
            body,
            hook,
            completedResponses
        }));
    } catch (error) {
        console.error(error);
        process.exitCode = 1;
        await fastify.close();
    }
})();
