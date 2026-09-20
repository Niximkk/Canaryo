const express = require("express");
const cookieParser = require("cookie-parser");
const cors = require("cors");
const multer = require("multer");

const app = express();
const upload = multer({ storage: multer.memoryStorage(), limits: { fileSize: 1024 * 1024 } });

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
    (_request, response, next) => {
        response.locals.steps.push("route-1");
        next();
    },
    (_request, response) => {
        response.locals.steps.push("route-2");
        response.json({ steps: response.locals.steps });
    }
);

app.post("/request-meta", express.json(), (request, response) => {
    response.json({
        accepts: request.accepts(["html", "json"]),
        charset: request.acceptsCharsets("utf-8", "iso-8859-1"),
        encoding: request.acceptsEncodings("gzip", "identity"),
        language: request.acceptsLanguages("pt-BR", "en"),
        contentType: request.is("json"),
        host: request.host,
        hostname: request.hostname,
        path: request.path,
        protocol: request.protocol,
        secure: request.secure,
        xhr: request.xhr,
        range: request.range(100)
    });
});

app.get("/response-helpers", (_request, response) => {
    response
        .status(202)
        .append("x-trace", "one")
        .append("x-trace", "two")
        .links({ next: "/next", last: "/last" })
        .vary("Accept-Language")
        .type("json")
        .send({ ok: true });
});
app.get("/send-status", (_request, response) => response.sendStatus(418));
app.get("/jsonp", (_request, response) => response.jsonp({ ok: true }));

const corsMiddleware = cors({ origin: "https://example.com", credentials: true });
app.options("/cors", corsMiddleware);
app.get("/cors", corsMiddleware, (_request, response) => response.json({ cors: true }));
app.get("/cookies", cookieParser(), (request, response) => response.json(request.cookies));
app.post("/upload", upload.single("file"), (request, response) => {
    response.json({
        field: request.file.fieldname,
        name: request.file.originalname,
        type: request.file.mimetype,
        size: request.file.size,
        contents: request.file.buffer.toString(),
        note: request.body.note
    });
});

app.get("/async", async (_request, response) => {
    await Promise.resolve();
    response.json({ async: true });
});
app.get("/async-failure", async () => {
    await Promise.resolve();
    throw new Error("async profile failure");
});

app.get("/cookie", (_request, response) => {
    response.cookie("token", "a b", { httpOnly: true, sameSite: "lax" }).send("cookie");
});
app.get("/redirect", (_request, response) => response.redirect(302, "/target"));
app.get("/failure", (_request, _response, next) => next(new Error("profile failure")));
app.use((error, _request, response, _next) => {
    response.status(500).json({ error: error.message });
});

module.exports = app;
