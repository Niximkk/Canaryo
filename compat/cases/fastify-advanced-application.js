const createFastify = require("../../fixtures/fastify-basic/node_modules/fastify");
const pino = require("../../fixtures/fastify-basic/node_modules/pino");
const { ReadableStream } = require("node:stream/web");

const port = Number(process.env.CANARYO_COMPAT_PORT);
const logs = [];
const logger = pino({ base: null, timestamp: false }, {
    write(line) {
        logs.push(JSON.parse(line));
    }
});
const fastify = createFastify({ loggerInstance: logger });

fastify.get("/users/:id", async request => ({
    id: request.params.id,
    active: request.query.active
}));
fastify.get("/delayed", async request => {
    request.log.info("compatibility request");
    await new Promise(resolve => setTimeout(resolve, 10));
    return { delayed: true, logger: typeof request.log.info };
});
fastify.get("/web-stream", () => new ReadableStream({
    start(controller) {
        controller.enqueue(Buffer.from("can"));
        controller.enqueue(new TextEncoder().encode("aryo"));
        controller.close();
    }
}));
fastify.get("/web-response", () => new Response("web-response", {
    status: 201,
    headers: { "x-compat-web": "response" }
}));
fastify.get("/fetch-response", () => fetch(
    `http://127.0.0.1:${port}/users/73?active=fetch`
));
fastify.get("/redirect-source", (_request, reply) => reply.redirect("/users/91?active=redirect"));
fastify.get("/fetch-redirect", () => fetch(`http://127.0.0.1:${port}/redirect-source`));

(async () => {
    try {
        await fastify.listen({ port, host: "127.0.0.1" });
        const webStream = await fetch(`http://127.0.0.1:${port}/web-stream`);
        const webResponse = await fetch(`http://127.0.0.1:${port}/web-response`);
        const fetchResponse = await fetch(`http://127.0.0.1:${port}/fetch-response`);
        const redirect = await fetch(`http://127.0.0.1:${port}/fetch-redirect`);
        const delayed = await fetch(`http://127.0.0.1:${port}/delayed`);

        const result = {
            webStream: { status: webStream.status, body: await webStream.text() },
            webResponse: {
                status: webResponse.status,
                header: webResponse.headers.get("x-compat-web"),
                body: await webResponse.text()
            },
            fetchResponse: await fetchResponse.json(),
            redirect: await redirect.json(),
            delayed: await delayed.json(),
            logger: (() => {
                const record = logs.find(entry => entry.compat === true);
                return { level: record.level, message: record.msg, compat: record.compat };
            })()
        };
        await fastify.close();
        console.log(JSON.stringify(result));
    } catch (error) {
        console.error(error);
        process.exitCode = 1;
        await fastify.close();
    }
})();
