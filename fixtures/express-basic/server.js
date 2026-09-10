const express = require("express");

const app = express();

app.get("/", (_request, response) => {
    response.json({ runtime: "canaryo", status: "ok" });
});

app.listen(3001, () => {
    console.log("Express fixture listening on http://127.0.0.1:3001");
});
