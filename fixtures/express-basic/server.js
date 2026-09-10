const express = require("express");
const port = Number(process.argv[2] || 3001);

const app = express();

app.get("/", (_request, response) => {
    response.json({ runtime: "canaryo", status: "ok" });
});

app.listen(port, () => {
    console.log(`Express fixture listening on http://127.0.0.1:${port}`);
});
