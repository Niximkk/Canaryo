const compression = require("compression");
const express = require("express");

const app = express();
app.use(compression({ threshold: 0 }));
app.get("/", (_request, response) => {
    response.json({ payload: "canaryo-compression-".repeat(100) });
});
app.listen(Number(process.argv[2]), "127.0.0.1");
