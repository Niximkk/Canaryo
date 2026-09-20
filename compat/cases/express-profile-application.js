const http = require("node:http");
const express = require("../../fixtures/express-basic/node_modules/express");

const port = Number(process.env.CANARYO_COMPAT_PORT);
const app = express();

app.use((_request, response, next) => {
    response.locals.steps = ["global"];
    response.set("x-profile", "express-5.2.1");
    next();
});
app.get("/users/:id", (request, response) => response.json({
    id: request.params.id,
    active: request.query.active,
    trace: request.get("x-trace")
}));

const router = express.Router();
router.get("/items/:slug", (request, response) => {
    response.json({ baseUrl: request.baseUrl, slug: request.params.slug });
});
app.use("/api", router);

app.post("/urlencoded", express.urlencoded({ extended: false }), (request, response) => {
    response.json(request.body);
});
app.post("/text", express.text({ type: "text/plain" }), (request, response) => {
    response.type("text").send(request.body);
});
app.post("/raw", express.raw({ type: "application/octet-stream" }), (request, response) => {
    response.json({ hex: request.body.toString("hex"), length: request.body.length });
});
app.get("/pipeline",
    (_request, response, next) => {
        response.locals.steps.push("route-1");
        next();
    },
    (_request, response) => {
        response.locals.steps.push("route-2");
        response.json({ steps: response.locals.steps });
    }
);
app.get("/cookie", (_request, response) => {
    response.cookie("token", "a b", { httpOnly: true, sameSite: "lax" }).send("cookie");
});
app.get("/redirect", (_request, response) => response.redirect(302, "/target"));
app.get("/failure", (_request, _response, next) => next(new Error("profile failure")));
app.use((error, _request, response, _next) => {
    response.status(500).json({ error: error.message });
});

function request(path, options = {}) {
    return new Promise((resolve, reject) => {
        const outgoing = http.request({
            host: "127.0.0.1",
            port,
            path,
            method: options.method || "GET",
            headers: options.headers || {},
            agent: false
        }, incoming => {
            const chunks = [];
            incoming.on("data", chunk => chunks.push(chunk));
            incoming.on("end", () => resolve({
                status: incoming.statusCode,
                headers: incoming.headers,
                body: Buffer.concat(chunks).toString()
            }));
        });
        outgoing.on("error", reject);
        if (options.body !== undefined) outgoing.write(options.body);
        outgoing.end();
    });
}

const server = app.listen(port, "127.0.0.1", async () => {
    try {
        const route = await request("/users/42?active=yes", { headers: { "x-trace": "profile" } });
        const nested = await request("/api/items/widget");
        const urlencoded = await request("/urlencoded", {
            method: "POST",
            headers: { "content-type": "application/x-www-form-urlencoded" },
            body: "name=canaryo&count=2"
        });
        const text = await request("/text", {
            method: "POST",
            headers: { "content-type": "text/plain" },
            body: "hello express"
        });
        const raw = await request("/raw", {
            method: "POST",
            headers: { "content-type": "application/octet-stream" },
            body: Buffer.from([0, 1, 254, 255])
        });
        const pipeline = await request("/pipeline");
        const cookie = await request("/cookie");
        const redirect = await request("/redirect");
        const failure = await request("/failure");

        server.close(() => console.log(JSON.stringify({
            route: { status: route.status, profile: route.headers["x-profile"], body: JSON.parse(route.body) },
            nested: JSON.parse(nested.body),
            urlencoded: JSON.parse(urlencoded.body),
            text: { contentType: text.headers["content-type"], body: text.body },
            raw: JSON.parse(raw.body),
            pipeline: JSON.parse(pipeline.body),
            cookie: cookie.headers["set-cookie"],
            redirect: { status: redirect.status, location: redirect.headers.location },
            failure: { status: failure.status, body: JSON.parse(failure.body) }
        })));
    } catch (error) {
        console.error(error && error.stack ? error.stack : error);
        process.exitCode = 1;
        server.close();
    }
});
