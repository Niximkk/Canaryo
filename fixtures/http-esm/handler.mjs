import { suffix } from "./message.cjs";
import {
    AsyncLocalStorage,
    AsyncResource,
    createHook,
    executionAsyncId,
    executionAsyncResource,
    triggerAsyncId
} from "node:async_hooks";

const storage = new AsyncLocalStorage();
const asyncHooksReady = typeof AsyncResource === "function" &&
    typeof createHook === "function" &&
    executionAsyncId() === 1 &&
    triggerAsyncId() === 0 &&
    typeof executionAsyncResource() === "object";

export function handleRequest(_request, response, ready) {
    storage.run({ runtime: "canaryo" }, () => {
        response.setHeader("Content-Type", "application/json; charset=utf-8");
        response.end(JSON.stringify({
            runtime: storage.getStore().runtime,
            modules: `esm+cjs${suffix}`,
            ready: ready && asyncHooksReady
        }));
    });
}
