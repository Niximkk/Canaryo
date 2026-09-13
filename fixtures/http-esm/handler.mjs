import { suffix } from "./message.cjs";
import {
    AsyncLocalStorage,
    AsyncResource,
    createHook,
    executionAsyncId,
    executionAsyncResource,
    triggerAsyncId
} from "node:async_hooks";
import consoleModule, { Console } from "node:console";
import processModule, { platform } from "node:process";
import { pipeline as pipelinePromise } from "node:stream/promises";
import { setTimeout as delay } from "node:timers/promises";

const storage = new AsyncLocalStorage();
const asyncHooksReady = typeof AsyncResource === "function" &&
    typeof createHook === "function" &&
    executionAsyncId() === 1 &&
    triggerAsyncId() === 0 &&
    typeof executionAsyncResource() === "object";
const builtinAliasesReady = consoleModule === console && typeof Console === "function" &&
    processModule === process && platform === process.platform &&
    typeof pipelinePromise === "function" && typeof delay === "function";

export function handleRequest(_request, response, ready) {
    storage.run({ runtime: "canaryo" }, () => {
        response.setHeader("Content-Type", "application/json; charset=utf-8");
        response.end(JSON.stringify({
            runtime: storage.getStore().runtime,
            modules: `esm+cjs${suffix}`,
            ready: ready && asyncHooksReady && builtinAliasesReady
        }));
    });
}
