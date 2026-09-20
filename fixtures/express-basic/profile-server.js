const app = require("./profile-app");
const port = Number(process.argv[2] || 3000);

app.listen(port, "127.0.0.1");
