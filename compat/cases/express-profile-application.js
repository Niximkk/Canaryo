const http = require("node:http");
const app = require("../../fixtures/express-basic/profile-app");

const port = Number(process.env.CANARYO_COMPAT_PORT);

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
        const requestMeta = await request("/request-meta", {
            method: "POST",
            headers: {
                accept: "application/json",
                "accept-charset": "utf-8",
                "accept-encoding": "gzip, identity;q=0.5",
                "accept-language": "pt-BR, en;q=0.5",
                "content-type": "application/json",
                host: "api.example.com:4321",
                range: "bytes=0-9",
                "x-requested-with": "XMLHttpRequest"
            },
            body: "{}"
        });
        const responseHelpers = await request("/response-helpers");
        const sendStatus = await request("/send-status");
        const jsonp = await request("/jsonp?callback=handle");
        const asyncResponse = await request("/async");
        const asyncFailure = await request("/async-failure");
        const corsResponse = await request("/cors", {
            headers: { origin: "https://example.com" }
        });
        const cookies = await request("/cookies", {
            headers: { cookie: "theme=dark; count=2" }
        });
        const boundary = "----canaryo-profile-boundary";
        const multipart = [
            `--${boundary}\r\nContent-Disposition: form-data; name="note"\r\n\r\nprofile\r\n`,
            `--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="hello.txt"\r\n`,
            "Content-Type: text/plain\r\n\r\nhello upload\r\n",
            `--${boundary}--\r\n`
        ].join("");
        const upload = await request("/upload", {
            method: "POST",
            headers: { "content-type": `multipart/form-data; boundary=${boundary}` },
            body: multipart
        });
        const cookie = await request("/cookie");
        const redirect = await request("/redirect");
        const failure = await request("/failure");

        server.close(() => console.log(JSON.stringify({
            route: { status: route.status, profile: route.headers["x-canaryo-profile"], body: JSON.parse(route.body) },
            nested: JSON.parse(nested.body),
            urlencoded: JSON.parse(urlencoded.body),
            text: { contentType: text.headers["content-type"], body: text.body },
            raw: JSON.parse(raw.body),
            pipeline: JSON.parse(pipeline.body),
            requestMeta: JSON.parse(requestMeta.body),
            responseHelpers: {
                status: responseHelpers.status,
                trace: responseHelpers.headers["x-trace"],
                link: responseHelpers.headers.link,
                vary: responseHelpers.headers.vary,
                body: JSON.parse(responseHelpers.body)
            },
            sendStatus: { status: sendStatus.status, body: sendStatus.body },
            jsonp: { contentType: jsonp.headers["content-type"], body: jsonp.body },
            asyncResponse: JSON.parse(asyncResponse.body),
            asyncFailure: { status: asyncFailure.status, body: JSON.parse(asyncFailure.body) },
            cors: {
                origin: corsResponse.headers["access-control-allow-origin"],
                credentials: corsResponse.headers["access-control-allow-credentials"],
                body: JSON.parse(corsResponse.body)
            },
            cookies: JSON.parse(cookies.body),
            upload: JSON.parse(upload.body),
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
