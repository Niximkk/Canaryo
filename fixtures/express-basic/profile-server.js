const express = require("express");

const app = express();
const port = Number(process.argv[2] || 3000);

app.use((request, response, next) => {
    response.locals.steps = ["global"];
    response.set("x-canaryo-profile", "express-5.2.1");
    next();
});

app.get("/users/:id", (request, response) => {
    response.status(200).json({
        id: request.params.id,
        active: request.query.active,
        trace: request.get("x-trace")
    });
});

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
    (request, response, next) => {
        response.locals.steps.push("route-1");
        next();
    },
    (request, response) => {
        response.locals.steps.push("route-2");
        response.json({ steps: response.locals.steps });
    }
);

app.get("/cookie", (request, response) => {
    response.cookie("token", "a b", { httpOnly: true, sameSite: "lax" }).send("cookie");
});
app.get("/redirect", (_request, response) => response.redirect(302, "/target"));
app.get("/failure", (_request, _response, next) => next(new Error("profile failure")));
app.use((error, _request, response, _next) => {
    response.status(500).json({ error: error.message });
});

app.listen(port, "127.0.0.1");
