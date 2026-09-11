import { suffix } from "./message.cjs";
import { AsyncLocalStorage } from "node:async_hooks";

const storage = new AsyncLocalStorage();

export function handleRequest(_request, response, ready) {
    storage.run({ runtime: "canaryo" }, () => {
        response.setHeader("Content-Type", "application/json; charset=utf-8");
        response.end(JSON.stringify({
            runtime: storage.getStore().runtime,
            modules: `esm+cjs${suffix}`,
            ready
        }));
    });
}
