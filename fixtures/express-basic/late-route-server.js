const express = require("express");
const port = Number(process.argv[2] || 3000);

const app = express();
app.get("/", (_request, response) => response.send("initial"));
app.listen(port, "127.0.0.1");

// Express permits the route stack to change after listen(). Canaryo must leave
// its startup route plan when that happens and dispatch through Express again.
app.get("/late", (_request, response) => response.send("late"));
