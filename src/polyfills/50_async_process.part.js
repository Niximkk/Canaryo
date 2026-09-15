    const asyncLocalStorages = new Set();
    let asyncContextEnabled = false;
    function captureAsyncContext() {
        if (!asyncContextEnabled) return;
        let snapshot;
        for (const storage of asyncLocalStorages) {
            if (storage.enabled && storage.active) {
                (snapshot || (snapshot = [])).push([storage, storage.store]);
            }
        }
        return snapshot;
    }
    function enterAsyncContext(snapshot) {
        const previous = [];
        for (const storage of asyncLocalStorages) {
            previous.push([storage, storage.enabled, storage.active, storage.store]);
            storage.active = false;
            storage.store = storage.defaultValue;
        }
        for (const [storage, store] of snapshot) {
            storage.enabled = true;
            storage.active = true;
            storage.store = store;
        }
        return previous;
    }
    function restoreAsyncContext(snapshot) {
        for (const storage of asyncLocalStorages) {
            storage.active = false;
            storage.store = storage.defaultValue;
        }
        for (const [storage, enabled, active, store] of snapshot) {
            storage.enabled = enabled;
            storage.active = active;
            storage.store = store;
        }
    }
    function runInAsyncContext(snapshot, callback, thisArg, args) {
        if (!snapshot) return callback.apply(thisArg, args);
        const previous = enterAsyncContext(snapshot);
        try { return callback.apply(thisArg, args); }
        finally { restoreAsyncContext(previous); }
    }

    function AsyncLocalStorage(options = {}) {
        asyncContextEnabled = true;
        ensurePromiseContextPropagation();
        this.defaultValue = options.defaultValue;
        this.name = options.name;
        this.store = this.defaultValue;
        this.enabled = true;
        this.active = false;
        asyncLocalStorages.add(this);
    }
    AsyncLocalStorage.prototype.run = function (store, callback, ...args) {
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        const previous = [this.enabled, this.active, this.store];
        this.enabled = true;
        this.active = true;
        this.store = store;
        try { return callback(...args); }
        finally { [this.enabled, this.active, this.store] = previous; }
    };
    AsyncLocalStorage.prototype.getStore = function () {
        return this.enabled && this.active ? this.store : this.defaultValue;
    };
    AsyncLocalStorage.prototype.enterWith = function (store) {
        this.enabled = true;
        this.active = true;
        this.store = store;
    };
    AsyncLocalStorage.prototype.disable = function () {
        this.enabled = false;
        this.active = false;
        this.store = this.defaultValue;
    };
    AsyncLocalStorage.bind = function (callback) {
        const snapshot = captureAsyncContext();
        return function (...args) { return runInAsyncContext(snapshot, callback, this, args); };
    };
    AsyncLocalStorage.snapshot = function () {
        const snapshot = captureAsyncContext();
        return (callback, ...args) => runInAsyncContext(snapshot, callback, undefined, args);
    };

    const rootAsyncResource = {};
    const activeAsyncHooks = new Set();
    let nextAsyncId = 2;
    let currentAsyncId = 1;
    let currentTriggerAsyncId = 0;
    let currentAsyncResource = rootAsyncResource;

    function emitAsyncHook(name, ...args) {
        for (const hook of [...activeAsyncHooks]) {
            const callback = hook.callbacks[name];
            if (typeof callback === "function") callback.apply(hook.callbacks, args);
        }
    }

    function createHook(callbacks) {
        if (!callbacks || typeof callbacks !== "object") throw new TypeError("callbacks must be an object");
        for (const name of ["init", "before", "after", "destroy", "promiseResolve"]) {
            if (callbacks[name] !== undefined && typeof callbacks[name] !== "function") {
                throw new TypeError(`hook.${name} must be a function`);
            }
        }
        return {
            callbacks,
            enable() {
                activeAsyncHooks.add(this);
                return this;
            },
            disable() {
                activeAsyncHooks.delete(this);
                return this;
            }
        };
    }

    function executionAsyncId() { return currentAsyncId; }
    function triggerAsyncId() { return currentTriggerAsyncId; }
    function executionAsyncResource() { return currentAsyncResource; }

    function AsyncResource(type, options = {}) {
        if (typeof type !== "string") throw new TypeError("type must be a string");
        const configuredTrigger = typeof options === "number" ? options : options?.triggerAsyncId;
        this.type = type;
        this._asyncId = nextAsyncId++;
        this._triggerAsyncId = configuredTrigger === undefined ? currentAsyncId : Number(configuredTrigger);
        this._destroyed = false;
        if (asyncContextEnabled) this.context = captureAsyncContext();
        emitAsyncHook("init", this._asyncId, this.type, this._triggerAsyncId, this);
    }
    AsyncResource.prototype.runInAsyncScope = function (fn, thisArg, ...args) {
        if (typeof fn !== "function") throw new TypeError("fn must be a function");
        const previousId = currentAsyncId;
        const previousTriggerId = currentTriggerAsyncId;
        const previousResource = currentAsyncResource;
        const previousContext = this.context ? enterAsyncContext(this.context) : undefined;
        currentAsyncId = this._asyncId;
        currentTriggerAsyncId = this._triggerAsyncId;
        currentAsyncResource = this;
        try {
            emitAsyncHook("before", this._asyncId);
            return fn.apply(thisArg, args);
        } finally {
            emitAsyncHook("after", this._asyncId);
            currentAsyncId = previousId;
            currentTriggerAsyncId = previousTriggerId;
            currentAsyncResource = previousResource;
            if (previousContext) restoreAsyncContext(previousContext);
        }
    };
    AsyncResource.prototype.bind = function (fn, thisArg) {
        const resource = this;
        const bound = function (...args) {
            return resource.runInAsyncScope(fn, thisArg === undefined ? this : thisArg, ...args);
        };
        Object.defineProperty(bound, "asyncResource", { value: resource });
        return bound;
    };
    AsyncResource.prototype.emitDestroy = function () {
        if (this._destroyed) return this;
        this._destroyed = true;
        const asyncId = this._asyncId;
        setImmediate(() => emitAsyncHook("destroy", asyncId));
        return this;
    };
    AsyncResource.prototype.asyncId = function () { return this._asyncId; };
    AsyncResource.prototype.triggerAsyncId = function () { return this._triggerAsyncId; };
    AsyncResource.bind = function (fn, type, thisArg) {
        return new AsyncResource(type || fn.name || "bound-anonymous-fn").bind(fn, thisArg);
    };

    function EventEmitterAsyncResource(options = {}) {
        if (typeof options === "string") options = { name: options };
        if (!options || typeof options.name !== "string") {
            throw new TypeError("options.name must be a string");
        }
        EventEmitter.call(this, options);
        Object.defineProperty(this, "asyncResource", {
            value: new AsyncResource(options.name, options),
            enumerable: true
        });
    }
    EventEmitterAsyncResource.prototype = Object.create(EventEmitter.prototype, {
        constructor: { value: EventEmitterAsyncResource, writable: true, configurable: true }
    });
    EventEmitterAsyncResource.prototype.emit = function (name, ...args) {
        return this.asyncResource.runInAsyncScope(
            EventEmitter.prototype.emit,
            this,
            name,
            ...args
        );
    };
    EventEmitterAsyncResource.prototype.emitDestroy = function () {
        this.asyncResource.emitDestroy();
    };
    Object.defineProperties(EventEmitterAsyncResource.prototype, {
        asyncId: { get() { return this.asyncResource.asyncId(); } },
        triggerAsyncId: { get() { return this.asyncResource.triggerAsyncId(); } }
    });
    EventEmitter.EventEmitterAsyncResource = EventEmitterAsyncResource;

    const originalPromiseThen = Promise.prototype.then;
    let promiseContextPatched = false;
    function ensurePromiseContextPropagation() {
        if (promiseContextPatched) return;
        promiseContextPatched = true;
        Promise.prototype.then = function (onFulfilled, onRejected) {
            const snapshot = captureAsyncContext();
            if (!snapshot) return originalPromiseThen.call(this, onFulfilled, onRejected);
            const wrap = callback => typeof callback === "function"
                ? function (...args) { return runInAsyncContext(snapshot, callback, this, args); }
                : callback;
            return originalPromiseThen.call(this, wrap(onFulfilled), wrap(onRejected));
        };
    }

    function createDiagnosticsChannel(name) {
        const subscribers = new Set();
        const stores = new Map();
        return {
            name: String(name),
            get hasSubscribers() { return subscribers.size > 0 || stores.size > 0; },
            publish(message) {
                for (const subscriber of [...subscribers]) subscriber(message, this.name);
            },
            subscribe(subscriber) {
                if (typeof subscriber !== "function") throw new TypeError("subscriber must be a function");
                subscribers.add(subscriber);
            },
            unsubscribe(subscriber) { return subscribers.delete(subscriber); },
            bindStore(store, transform = message => message) {
                if (!store || typeof store.run !== "function") throw new TypeError("store must be an AsyncLocalStorage instance");
                if (typeof transform !== "function") throw new TypeError("transform must be a function");
                stores.set(store, transform);
            },
            unbindStore(store) { return stores.delete(store); },
            runStores(message, callback, thisArg, ...args) {
                if (typeof callback !== "function") throw new TypeError("callback must be a function");
                const entries = [...stores.entries()];
                const run = index => index === entries.length
                    ? callback.apply(thisArg, args)
                    : entries[index][0].run(entries[index][1](message), () => run(index + 1));
                return run(0);
            }
        };
    }
    const diagnosticsChannels = new Map();
    const tracingEvents = ["start", "end", "asyncStart", "asyncEnd", "error"];

    function createTracingChannel(nameOrChannels, channelForName) {
        const channels = typeof nameOrChannels === "object" && nameOrChannels !== null
            ? nameOrChannels
            : Object.fromEntries(tracingEvents.map(event => [
                event,
                channelForName(`tracing:${String(nameOrChannels)}:${event}`)
            ]));
        const tracing = {};
        for (const event of tracingEvents) {
            if (!channels[event] || typeof channels[event].publish !== "function") {
                throw new TypeError(`channels.${event} must be a diagnostics channel`);
            }
            Object.defineProperty(tracing, event, { value: channels[event], enumerable: true });
        }
        Object.defineProperties(tracing, {
            hasSubscribers: {
                get() { return tracingEvents.some(event => tracing[event].hasSubscribers); }
            },
            subscribe: {
                value(subscribers) {
                    if (!subscribers || typeof subscribers !== "object") {
                        throw new TypeError("subscribers must be an object");
                    }
                    for (const event of tracingEvents) {
                        if (typeof subscribers[event] === "function") tracing[event].subscribe(subscribers[event]);
                    }
                }
            },
            unsubscribe: {
                value(subscribers) {
                    if (!subscribers || typeof subscribers !== "object") return false;
                    let removed = false;
                    for (const event of tracingEvents) {
                        if (typeof subscribers[event] === "function") {
                            removed = tracing[event].unsubscribe(subscribers[event]) || removed;
                        }
                    }
                    return removed;
                }
            },
            traceSync: {
                value(fn, context = {}, thisArg, ...args) {
                    if (typeof fn !== "function") throw new TypeError("fn must be a function");
                    if (!this.hasSubscribers) return fn.apply(thisArg, args);
                    return this.start.runStores(context, () => {
                        this.start.publish(context);
                        try {
                            const result = fn.apply(thisArg, args);
                            context.result = result;
                            this.end.publish(context);
                            return result;
                        } catch (error) {
                            context.error = error;
                            this.error.publish(context);
                            this.end.publish(context);
                            throw error;
                        }
                    });
                }
            },
            tracePromise: {
                value(fn, context = {}, thisArg, ...args) {
                    if (typeof fn !== "function") throw new TypeError("fn must be a function");
                    if (!this.hasSubscribers) return fn.apply(thisArg, args);
                    let result;
                    this.start.runStores(context, () => {
                        this.start.publish(context);
                        try {
                            result = fn.apply(thisArg, args);
                            this.end.publish(context);
                        } catch (error) {
                            context.error = error;
                            this.error.publish(context);
                            this.end.publish(context);
                            throw error;
                        }
                    });
                    if (!result || typeof result.then !== "function") return result;
                    return result.then(
                        value => {
                            context.result = value;
                            return this.asyncStart.runStores(context, () => {
                                this.asyncStart.publish(context);
                                try { return value; }
                                finally { this.asyncEnd.publish(context); }
                            });
                        },
                        error => {
                            context.error = error;
                            return this.asyncStart.runStores(context, () => {
                                this.asyncStart.publish(context);
                                this.error.publish(context);
                                try { throw error; }
                                finally { this.asyncEnd.publish(context); }
                            });
                        }
                    );
                }
            },
            traceCallback: {
                value(fn, position, context = {}, thisArg, ...args) {
                    if (typeof fn !== "function") throw new TypeError("fn must be a function");
                    if (!this.hasSubscribers) return fn.apply(thisArg, args);
                    const callbackIndex = position === undefined ? args.length - 1 : Number(position);
                    const callback = args[callbackIndex];
                    if (typeof callback !== "function") throw new TypeError("callback must be a function");
                    const tracingChannel = this;
                    args[callbackIndex] = function (...callbackArgs) {
                        const callbackThis = this;
                        const error = callbackArgs[0];
                        if (error !== null && error !== undefined) context.error = error;
                        else context.result = callbackArgs[1];
                        return tracingChannel.asyncStart.runStores(context, () => {
                            tracingChannel.asyncStart.publish(context);
                            if (context.error !== undefined) tracingChannel.error.publish(context);
                            try { return callback.apply(callbackThis, callbackArgs); }
                            catch (callbackError) {
                                context.error = callbackError;
                                tracingChannel.error.publish(context);
                                throw callbackError;
                            } finally {
                                tracingChannel.asyncEnd.publish(context);
                            }
                        });
                    };
                    return this.start.runStores(context, () => {
                        this.start.publish(context);
                        try {
                            const result = fn.apply(thisArg, args);
                            this.end.publish(context);
                            return result;
                        } catch (error) {
                            context.error = error;
                            this.error.publish(context);
                            this.end.publish(context);
                            throw error;
                        }
                    });
                }
            }
        });
        return tracing;
    }

    const diagnosticsChannel = {
        channel(name) {
            if (!diagnosticsChannels.has(name)) diagnosticsChannels.set(name, createDiagnosticsChannel(name));
            return diagnosticsChannels.get(name);
        },
        hasSubscribers(name) { return this.channel(name).hasSubscribers; },
        subscribe(name, subscriber) { this.channel(name).subscribe(subscriber); },
        unsubscribe(name, subscriber) { return this.channel(name).unsubscribe(subscriber); },
        tracingChannel(nameOrChannels) {
            return createTracingChannel(nameOrChannels, name => this.channel(name));
        }
    };

    function depd() {
        function deprecate() {}
        deprecate.function = function (fn) { return fn; };
        deprecate.property = function () {};
        return deprecate;
    }

    class AssertionError extends Error {
        constructor(options = {}) {
            super(options.message || "Assertion failed");
            this.name = "AssertionError";
            this.code = "ERR_ASSERTION";
            this.actual = options.actual;
            this.expected = options.expected;
            this.operator = options.operator;
            this.generatedMessage = options.message === undefined;
        }
    }
    function assertionError(message, actual, expected, operator) {
        return new AssertionError({ message, actual, expected, operator });
    }
    function deepEquals(actual, expected, strict, seen = new Map()) {
        if (strict ? Object.is(actual, expected) : actual == expected) return true;
        if (actual === null || expected === null || typeof actual !== "object" || typeof expected !== "object") return false;
        if (seen.get(actual) === expected) return true;
        seen.set(actual, expected);
        if (strict && Object.getPrototypeOf(actual) !== Object.getPrototypeOf(expected)) return false;
        if (actual instanceof Date || expected instanceof Date) return actual instanceof Date && expected instanceof Date && actual.getTime() === expected.getTime();
        if (actual instanceof RegExp || expected instanceof RegExp) return actual instanceof RegExp && expected instanceof RegExp && actual.source === expected.source && actual.flags === expected.flags;
        if (ArrayBuffer.isView(actual) || ArrayBuffer.isView(expected)) {
            if (!ArrayBuffer.isView(actual) || !ArrayBuffer.isView(expected) || actual.byteLength !== expected.byteLength) return false;
            const left = new Uint8Array(actual.buffer, actual.byteOffset, actual.byteLength);
            const right = new Uint8Array(expected.buffer, expected.byteOffset, expected.byteLength);
            return left.every((value, index) => value === right[index]);
        }
        if (actual instanceof Map || expected instanceof Map) {
            if (!(actual instanceof Map) || !(expected instanceof Map) || actual.size !== expected.size) return false;
            return [...actual].every(([key, value]) => [...expected].some(([otherKey, otherValue]) =>
                deepEquals(key, otherKey, strict, seen) && deepEquals(value, otherValue, strict, seen)));
        }
        if (actual instanceof Set || expected instanceof Set) {
            if (!(actual instanceof Set) || !(expected instanceof Set) || actual.size !== expected.size) return false;
            return [...actual].every(value => [...expected].some(other => deepEquals(value, other, strict, seen)));
        }
        const actualKeys = Object.keys(actual);
        const expectedKeys = Object.keys(expected);
        if (actualKeys.length !== expectedKeys.length || actualKeys.some(key => !Object.prototype.hasOwnProperty.call(expected, key))) return false;
        return actualKeys.every(key => deepEquals(actual[key], expected[key], strict, seen));
    }
    function matchesException(error, expected) {
        if (expected === undefined) return true;
        if (expected instanceof RegExp) return expected.test(String(error?.message || error));
        if (typeof expected === "function") {
            if (expected.prototype instanceof Error || expected === Error) return error instanceof expected;
            return expected(error) === true;
        }
        if (expected && typeof expected === "object") {
            return Object.keys(expected).every(key => expected[key] instanceof RegExp
                ? expected[key].test(String(error?.[key]))
                : deepEquals(error?.[key], expected[key], true));
        }
        throw new TypeError("expected must be a RegExp, function, or object");
    }
    function assert(value, message) {
        if (!value) throw assertionError(message, value, true, "==");
    }
    assert.ok = assert;
    assert.equal = (actual, expected, message) => { if (actual != expected) throw assertionError(message, actual, expected, "=="); };
    assert.notEqual = (actual, expected, message) => { if (actual == expected) throw assertionError(message, actual, expected, "!="); };
    assert.strictEqual = (actual, expected, message) => { if (!Object.is(actual, expected)) throw assertionError(message, actual, expected, "strictEqual"); };
    assert.notStrictEqual = (actual, expected, message) => { if (Object.is(actual, expected)) throw assertionError(message, actual, expected, "notStrictEqual"); };
    assert.deepEqual = (actual, expected, message) => { if (!deepEquals(actual, expected, false)) throw assertionError(message, actual, expected, "deepEqual"); };
    assert.notDeepEqual = (actual, expected, message) => { if (deepEquals(actual, expected, false)) throw assertionError(message, actual, expected, "notDeepEqual"); };
    assert.deepStrictEqual = (actual, expected, message) => { if (!deepEquals(actual, expected, true)) throw assertionError(message, actual, expected, "deepStrictEqual"); };
    assert.notDeepStrictEqual = (actual, expected, message) => { if (deepEquals(actual, expected, true)) throw assertionError(message, actual, expected, "notDeepStrictEqual"); };
    assert.fail = message => { throw assertionError(message); };
    assert.throws = (block, expected, message) => {
        if (typeof block !== "function") throw new TypeError("block must be a function");
        try { block(); }
        catch (error) {
            if (matchesException(error, expected)) return error;
            throw assertionError(message, error, expected, "throws");
        }
        throw assertionError(message || "Missing expected exception", undefined, expected, "throws");
    };
    assert.doesNotThrow = (block, expected, message) => {
        try { block(); }
        catch (error) {
            if (expected === undefined || matchesException(error, expected)) throw assertionError(message || "Got unwanted exception", error, expected, "doesNotThrow");
            throw error;
        }
    };
    assert.rejects = async (block, expected, message) => {
        try { await (typeof block === "function" ? block() : block); }
        catch (error) {
            if (matchesException(error, expected)) return;
            throw assertionError(message, error, expected, "rejects");
        }
        throw assertionError(message || "Missing expected rejection", undefined, expected, "rejects");
    };
    assert.doesNotReject = async (block, expected, message) => {
        try { await (typeof block === "function" ? block() : block); }
        catch (error) {
            if (expected === undefined || matchesException(error, expected)) throw assertionError(message || "Got unwanted rejection", error, expected, "doesNotReject");
            throw error;
        }
    };
    assert.match = (value, regexp, message) => { if (!(regexp instanceof RegExp) || !regexp.test(String(value))) throw assertionError(message, value, regexp, "match"); };
    assert.doesNotMatch = (value, regexp, message) => { if (!(regexp instanceof RegExp) || regexp.test(String(value))) throw assertionError(message, value, regexp, "doesNotMatch"); };
    assert.ifError = value => { if (value !== null && value !== undefined) throw assertionError(`ifError got unwanted exception: ${value.message || value}`, value, null, "ifError"); };
    assert.AssertionError = AssertionError;
    function strictAssert(value, message) { return assert(value, message); }
    Object.assign(strictAssert, assert, {
        equal: assert.strictEqual,
        notEqual: assert.notStrictEqual,
        deepEqual: assert.deepStrictEqual,
        notDeepEqual: assert.notDeepStrictEqual
    });
    strictAssert.strict = strictAssert;
    assert.strict = strictAssert;

    const processStartedAt = __canaryoPerformanceNow();
    EventEmitter.call(process);
    Object.setPrototypeOf(process, EventEmitter.prototype);
    process.cwd = () => __canaryoCwd();
    process.platform = __canaryoOsInfo.platform;
    process.arch = __canaryoOsInfo.arch;
    process.version = "v22.0.0-canaryo";
    process.versions = { node: "22.0.0", canaryo: "0.1.0" };
    process.release = { name: "canaryo", sourceUrl: "", headersUrl: "" };
    process.argv0 = "canaryo";
    process.execArgv = [];
    process.title = "canaryo";
    process.config = { variables: {} };
    process.moduleLoadList = [];
    process.exitCode = undefined;
    let processExitRequested = false;
    let processExitEmitted = false;
    function normalizedExitCode(value) {
        if (value === undefined || value === null || value === "") return 0;
        const number = Number(value);
        if (!Number.isInteger(number)) throw new TypeError("exit code must be an integer");
        return number & 255;
    }
    process.exit = code => {
        if (code !== undefined) process.exitCode = normalizedExitCode(code);
        else process.exitCode = normalizedExitCode(process.exitCode);
        processExitRequested = true;
        if (globalThis.__canaryoActiveServer) {
            globalThis.__canaryoActiveServer.__canaryoCloseRequested = true;
            globalThis.__canaryoActiveServer.__canaryoCloseAllConnectionsRequested = true;
        }
        if (!processExitEmitted) {
            processExitEmitted = true;
            process.emit("exit", process.exitCode);
        }
    };
    process.reallyExit = process.exit;
    process.abort = () => process.exit(134);
    globalThis.__canaryoProcessShouldExit = () => processExitRequested;
    globalThis.__canaryoProcessExitCode = () => normalizedExitCode(process.exitCode);
    process.nextTick = (callback, ...args) => {
        if (!asyncContextEnabled) {
            Promise.resolve().then(() => callback(...args));
            return;
        }
        const snapshot = captureAsyncContext();
        originalPromiseThen.call(
            Promise.resolve(),
            snapshot
                ? () => runInAsyncContext(snapshot, callback, undefined, args)
                : () => callback(...args)
        );
    };
    process.uptime = () => (__canaryoPerformanceNow() - processStartedAt) / 1000;
    process.hrtime = previous => {
        const nanoseconds = BigInt(Math.floor((__canaryoPerformanceNow() - processStartedAt) * 1000000));
        let seconds = Number(nanoseconds / 1000000000n);
        let remainder = Number(nanoseconds % 1000000000n);
        if (previous) {
            seconds -= Number(previous[0]);
            remainder -= Number(previous[1]);
            if (remainder < 0) { seconds -= 1; remainder += 1000000000; }
        }
        return [seconds, remainder];
    };
    process.hrtime.bigint = () => BigInt(Math.floor((__canaryoPerformanceNow() - processStartedAt) * 1000000));
    process.memoryUsage = () => ({ rss: 0, heapTotal: 0, heapUsed: 0, external: 0, arrayBuffers: 0 });
    process.memoryUsage.rss = () => 0;
    process.cpuUsage = () => ({ user: 0, system: 0 });
    process.resourceUsage = () => ({ userCPUTime: 0, systemCPUTime: 0, maxRSS: 0 });
    process.emitWarning = (warning, options) => {
        const value = warning instanceof Error ? warning : new Error(String(warning));
        value.name = typeof options === "string" ? options : options && options.type || "Warning";
        if (!process.emit("warning", value)) console.error(`${value.name}: ${value.message}`);
    };
    process.getBuiltinModule = name => {
        const normalized = String(name).replace(/^node:/, "");
        if (normalized === "http") return globalThis.__canaryoHttpModule;
        return globalThis.__canaryoBuiltins[normalized];
    };
    function terminalStatus(fd) {
        if (Number(fd) === 0) return Boolean(__canaryoOsInfo.stdinIsTerminal);
        if (Number(fd) === 1) return Boolean(__canaryoOsInfo.stdoutIsTerminal);
        if (Number(fd) === 2) return Boolean(__canaryoOsInfo.stderrIsTerminal);
        return false;
    }
    function TTYReadStream(fd = 0, options = {}) {
        if (!(this instanceof TTYReadStream)) return new TTYReadStream(fd, options);
        Readable.call(this, options);
        this.fd = Number(fd);
        this.isTTY = terminalStatus(this.fd);
        this.isRaw = false;
    }
    util.inherits(TTYReadStream, Readable);
    TTYReadStream.prototype.setRawMode = function (mode) { this.isRaw = Boolean(mode); return this; };
    TTYReadStream.prototype.ref = function () { this._referenced = true; return this; };
    TTYReadStream.prototype.unref = function () { this._referenced = false; return this; };
    TTYReadStream.prototype.hasRef = function () { return this._referenced !== false; };
    function TTYWriteStream(fd = 1) {
        if (!(this instanceof TTYWriteStream)) return new TTYWriteStream(fd);
        EventEmitter.call(this);
        this.fd = Number(fd);
        this.isTTY = terminalStatus(this.fd);
        this.writable = true;
        this.columns = this.isTTY ? 80 : undefined;
        this.rows = this.isTTY ? 24 : undefined;
    }
    util.inherits(TTYWriteStream, EventEmitter);
    TTYWriteStream.prototype.write = function (value, encoding, callback) {
        if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        const output = typeof value === "string" ? value : Buffer.from(value).toString(encoding);
        if (this.fd === 2) __canaryoWriteError(output);
        else __canaryoWrite(output);
        if (typeof callback === "function") process.nextTick(callback, null);
        return true;
    };
    TTYWriteStream.prototype.getColorDepth = function (environment = process.env) {
        if (environment && (environment.FORCE_COLOR === "0" || environment.NO_COLOR !== undefined)) return 1;
        if (!this.isTTY && !(environment && environment.FORCE_COLOR)) return 1;
        if (environment && environment.COLORTERM === "truecolor") return 24;
        return 8;
    };
    TTYWriteStream.prototype.hasColors = function (count = 16, environment) {
        return 2 ** this.getColorDepth(environment) >= Number(count);
    };
    TTYWriteStream.prototype.getWindowSize = function () { return [this.columns || 80, this.rows || 24]; };
    for (const method of ["clearLine", "clearScreenDown", "cursorTo", "moveCursor"]) {
        TTYWriteStream.prototype[method] = function (...args) {
            const callback = typeof args[args.length - 1] === "function" ? args.pop() : undefined;
            if (callback) process.nextTick(callback, null);
            return true;
        };
    }
    TTYWriteStream.prototype.ref = function () { this._referenced = true; return this; };
    TTYWriteStream.prototype.unref = function () { this._referenced = false; return this; };
    TTYWriteStream.prototype.hasRef = function () { return this._referenced !== false; };
    process.stdin = new TTYReadStream(0);
    process.stdout = new TTYWriteStream(1);
    process.stderr = new TTYWriteStream(2);
