const express = require("../../fixtures/express-basic/node_modules/express");

const port = Number(process.env.CANARYO_COMPAT_PORT);
const app = express();
let requests = 0;

app.use((_request, _response, next) => {
    requests += 1;
    next();
});
app.get("/users/:id", (request, response) => {
    response.json({ id: request.params.id, active: request.query.active });
});
app.post("/echo", express.json(), (request, response) => {
    response.status(201).json({ body: request.body });
});

const server = app.listen(port, "127.0.0.1", async () => {
    try {
        const routeResponse = await fetch(`http://127.0.0.1:${port}/users/42?active=true`);
        const route = await routeResponse.json();
        const bodyResponse = await fetch(`http://127.0.0.1:${port}/echo`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ runtime: "canaryo", compatible: true })
        });
        const body = await bodyResponse.json();

        server.close(() => {
            console.log(JSON.stringify({
                statuses: [routeResponse.status, bodyResponse.status],
                route,
                body,
                requests
            }));
        });
    } catch (error) {
        console.error(error);
        process.exitCode = 1;
        server.close();
    }
});
