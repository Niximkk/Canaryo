import { createServer } from "node:http";
import { handleRequest } from "./handler.mjs";

const port = Number(process.argv[2] || 3003);
createServer(handleRequest).listen(port);
