"use strict";

const path = require("node:path");
const express = require("../express-basic/node_modules/express");

const app = express();
app.use(express.static(path.join(__dirname, "public")));
app.listen(Number(process.argv[2] || 3000));
