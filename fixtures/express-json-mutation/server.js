const express = require("../express-basic/node_modules/express");
const port = Number(process.argv[2] || 3001);

const app = express();

app.post("/", express.json(), (request, response) => {
    request.body.data = `${request.body.data}-changed`;
    Object.defineProperty(request.body, "toJSON", {
        value: () => ({ data: request.body.data, customized: true })
    });
    response.json({ body: request.body });
});

app.listen(port);
