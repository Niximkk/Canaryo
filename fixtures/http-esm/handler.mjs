import { suffix } from "#message";
import strictAssert from "node:assert/strict";
import {
    AsyncLocalStorage,
    AsyncResource,
    createHook,
    executionAsyncId,
    executionAsyncResource,
    triggerAsyncId
} from "node:async_hooks";
import consoleModule, { Console } from "node:console";
import { lookup as lookupPromise } from "node:dns/promises";
import moduleModule, { isBuiltin } from "node:module";
import processModule, { platform } from "node:process";
import posixPath from "node:path/posix";
import { text as consumeText } from "node:stream/consumers";
import { pipeline as pipelinePromise } from "node:stream/promises";
import { CompressionStream, ReadableStream } from "node:stream/web";
import { setTimeout as delay } from "node:timers/promises";
import { isPromise } from "node:util/types";

const storage = new AsyncLocalStorage();
let strictAssertReady = true;
try { strictAssert.strictEqual(1, 1); }
catch { strictAssertReady = false; }
const asyncHooksReady = typeof AsyncResource === "function" &&
    typeof createHook === "function" &&
    executionAsyncId() === 1 &&
    triggerAsyncId() === 0 &&
    typeof executionAsyncResource() === "object";
const builtinAliasesReady = consoleModule === console && typeof Console === "function" &&
    processModule === process && platform === process.platform &&
    strictAssertReady &&
    posixPath.join("can", "aryo") === "can/aryo" && isPromise(Promise.resolve()) &&
    typeof consumeText === "function" && typeof pipelinePromise === "function" &&
    typeof delay === "function" && typeof lookupPromise === "function" &&
    typeof moduleModule.createRequire === "function" && isBuiltin("node:http") &&
    typeof ReadableStream === "function" && typeof CompressionStream === "function";

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
