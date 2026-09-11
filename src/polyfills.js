(() => {
    function EventEmitter() { this._events = Object.create(null); }
    EventEmitter.prototype.on = EventEmitter.prototype.addListener = function (name, listener) {
        const events = this._events || (this._events = Object.create(null));
        const listeners = events[name] || (events[name] = []);
        listeners.push(listener);
        return this;
    };
    EventEmitter.prototype.once = function (name, listener) {
        const emitter = this;
        function onceListener(...args) {
            emitter.removeListener(name, onceListener);
            listener.apply(emitter, args);
        }
        onceListener.listener = listener;
        return this.on(name, onceListener);
    };
    EventEmitter.prototype.emit = function (name, ...args) {
        const listeners = this._events && this._events[name];
        if (!listeners) return false;
        for (const listener of [...listeners]) listener.apply(this, args);
        return true;
    };
    EventEmitter.prototype.removeListener = function (name, listener) {
        const listeners = this._events && this._events[name];
        if (!listeners) return this;
        this._events[name] = listeners.filter(item => item !== listener && item.listener !== listener);
        return this;
    };
    EventEmitter.prototype.off = EventEmitter.prototype.removeListener;
    EventEmitter.prototype.removeAllListeners = function (name) {
        if (name === undefined) this._events = Object.create(null);
        else if (this._events) delete this._events[name];
        return this;
    };
    EventEmitter.prototype.listeners = function (name) { return [...(this._events && this._events[name] || [])]; };
    EventEmitter.prototype.rawListeners = EventEmitter.prototype.listeners;
    EventEmitter.prototype.listenerCount = function (name) { return this.listeners(name).length; };
    EventEmitter.prototype.setMaxListeners = function (value) { this._maxListeners = Number(value); return this; };
    EventEmitter.prototype.getMaxListeners = function () { return this._maxListeners ?? 10; };

    function encodeUtf8(value) {
        const bytes = [];
        for (const character of String(value)) {
            const code = character.codePointAt(0);
            if (code <= 0x7f) bytes.push(code);
            else if (code <= 0x7ff) bytes.push(0xc0 | code >> 6, 0x80 | code & 0x3f);
            else if (code <= 0xffff) bytes.push(0xe0 | code >> 12, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
            else bytes.push(0xf0 | code >> 18, 0x80 | code >> 12 & 0x3f, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
        }
        return bytes;
    }
    function decodeUtf8(bytes) {
        let result = "";
        for (let index = 0; index < bytes.length;) {
            const first = bytes[index++];
            if (first < 0x80) result += String.fromCodePoint(first);
            else if (first < 0xe0) result += String.fromCodePoint((first & 0x1f) << 6 | bytes[index++] & 0x3f);
            else if (first < 0xf0) result += String.fromCodePoint((first & 0x0f) << 12 | (bytes[index++] & 0x3f) << 6 | bytes[index++] & 0x3f);
            else result += String.fromCodePoint((first & 0x07) << 18 | (bytes[index++] & 0x3f) << 12 | (bytes[index++] & 0x3f) << 6 | bytes[index++] & 0x3f);
        }
        return result;
    }
    class Buffer extends Uint8Array {
        static from(value) {
            if (typeof value === "string") return new Buffer(encodeUtf8(value));
            if (ArrayBuffer.isView(value)) return new Buffer(value);
            if (value instanceof ArrayBuffer) return new Buffer(new Uint8Array(value));
            return new Buffer(value);
        }
        static alloc(size, fill = 0) { const buffer = new Buffer(size); buffer.fill(fill); return buffer; }
        static allocUnsafe(size) { return new Buffer(size); }
        static isBuffer(value) { return value instanceof Buffer; }
        static byteLength(value) {
            if (typeof value === "string") return __canaryoByteLength(value);
            if (ArrayBuffer.isView(value)) return value.byteLength;
            if (value instanceof ArrayBuffer) return value.byteLength;
            return Buffer.from(value).length;
        }
        static concat(list, totalLength) {
            const length = totalLength ?? list.reduce((sum, item) => sum + item.length, 0);
            const result = new Buffer(length);
            let offset = 0;
            for (const item of list) { result.set(item, offset); offset += item.length; }
            return result;
        }
        toString() { return decodeUtf8(this); }
    }
    for (const method of ["from", "alloc", "allocUnsafe", "isBuffer", "byteLength", "concat"]) {
        Object.defineProperty(Buffer, method, { enumerable: true });
    }

    class TextEncoder {
        encode(value) { return new Uint8Array(encodeUtf8(value)); }
    }

    class TextDecoder {
        decode(value = new Uint8Array()) { return decodeUtf8(value); }
    }

    class StringDecoder {
        constructor(encoding = "utf8") {
            const normalized = String(encoding).toLowerCase().replace(/[-_]/g, "");
            if (normalized !== "utf8") throw new Error(`encoding not supported: ${encoding}`);
            this.encoding = "utf8";
        }
        write(value) { return Buffer.from(value).toString(); }
        end(value) { return value === undefined ? "" : this.write(value); }
        text(value, offset = 0) { return this.write(value.subarray(offset)); }
    }

    function inspect(value) {
        if (typeof value === "string") return value;
        try { return JSON.stringify(value); } catch { return String(value); }
    }
    function format(first, ...values) {
        if (typeof first !== "string") return [first, ...values].map(inspect).join(" ");
        let index = 0;
        const formatted = first.replace(/%[sdijoO%]/g, token => {
            if (token === "%%") return "%";
            if (index >= values.length) return token;
            const value = values[index++];
            if (token === "%d" || token === "%i") return String(Number(value));
            if (token === "%j") { try { return JSON.stringify(value); } catch { return "[Circular]"; } }
            return token === "%s" ? String(value) : inspect(value);
        });
        return [formatted, ...values.slice(index).map(inspect)].join(" ");
    }
    const util = {
        inherits(constructor, parent) {
            constructor.super_ = parent;
            constructor.prototype = Object.create(parent.prototype, { constructor: { value: constructor, writable: true, configurable: true } });
        },
        deprecate(fn) { return fn; },
        debuglog() { const logger = () => {}; logger.enabled = false; return logger; },
        format,
        formatWithOptions(_options, ...args) { return format(...args); },
        inspect,
        types: { isDate: value => value instanceof Date, isRegExp: value => value instanceof RegExp, isNativeError: value => value instanceof Error }
    };

    function normalizePath(value) {
        const absolute = /^[A-Za-z]:[\\/]|^[\\/]/.test(value);
        const prefix = /^[A-Za-z]:/.test(value) ? value.slice(0, 2) : "";
        const parts = value.replace(/\\/g, "/").replace(/^[A-Za-z]:/, "").split("/");
        const output = [];
        for (const part of parts) {
            if (!part || part === ".") continue;
            if (part === "..") output.pop();
            else output.push(part);
        }
        return prefix + (absolute ? "/" : "") + output.join("/") || ".";
    }
    const path = {
        sep: "/",
        delimiter: ";",
        normalize: normalizePath,
        join(...parts) { return normalizePath(parts.filter(Boolean).join("/")); },
        resolve(...parts) {
            let result = process.cwd();
            for (const part of parts) result = /^[A-Za-z]:[\\/]|^[\\/]/.test(part) ? part : result + "/" + part;
            return normalizePath(result);
        },
        dirname(value) { const normalized = normalizePath(value); const index = normalized.lastIndexOf("/"); return index <= 0 ? "." : normalized.slice(0, index); },
        basename(value, suffix = "") { const name = normalizePath(value).split("/").pop(); return suffix && name.endsWith(suffix) ? name.slice(0, -suffix.length) : name; },
        extname(value) { const name = path.basename(value); const index = name.lastIndexOf("."); return index <= 0 ? "" : name.slice(index); },
        isAbsolute(value) { return /^[A-Za-z]:[\\/]|^[\\/]/.test(value); },
        relative(from, to) {
            const left = normalizePath(from).split("/");
            const right = normalizePath(to).split("/");
            while (left.length && right.length && left[0].toLowerCase() === right[0].toLowerCase()) { left.shift(); right.shift(); }
            return [...left.map(() => ".."), ...right].join("/");
        }
    };
    path.win32 = path;
    path.posix = path;

    function parseUrl(value) {
        const hashIndex = value.indexOf("#");
        const href = hashIndex < 0 ? value : value.slice(0, hashIndex);
        const queryIndex = href.indexOf("?");
        const pathname = queryIndex < 0 ? href : href.slice(0, queryIndex);
        const search = queryIndex < 0 ? null : href.slice(queryIndex);
        return { href: value, path: href, pathname, search, query: search ? search.slice(1) : null };
    }
    function Stream() { EventEmitter.call(this); }
    util.inherits(Stream, EventEmitter);
    Stream.Readable = Stream.Writable = Stream.Duplex = Stream.Transform = Stream.PassThrough = Stream;
    Stream.prototype.pipe = function (destination) { this.on("data", chunk => destination.write(chunk)); this.on("end", () => destination.end()); return destination; };

    function AsyncLocalStorage() { this.store = undefined; }
    AsyncLocalStorage.prototype.run = function (store, callback, ...args) { const previous = this.store; this.store = store; try { return callback(...args); } finally { this.store = previous; } };
    AsyncLocalStorage.prototype.getStore = function () { return this.store; };
    AsyncLocalStorage.prototype.enterWith = function (store) { this.store = store; };
    AsyncLocalStorage.prototype.disable = function () { this.store = undefined; };

    function AsyncResource(type) { this.type = String(type); }
    AsyncResource.prototype.runInAsyncScope = function (fn, thisArg, ...args) {
        return fn.apply(thisArg, args);
    };
    AsyncResource.prototype.bind = function (fn, thisArg) {
        const resource = this;
        return function (...args) {
            return resource.runInAsyncScope(fn, thisArg === undefined ? this : thisArg, ...args);
        };
    };
    AsyncResource.prototype.emitDestroy = function () { return this; };
    AsyncResource.prototype.asyncId = function () { return 0; };
    AsyncResource.prototype.triggerAsyncId = function () { return 0; };
    AsyncResource.bind = function (fn, type, thisArg) {
        return new AsyncResource(type || fn.name || "bound-anonymous-fn").bind(fn, thisArg);
    };

    function createDiagnosticsChannel(name) {
        return {
            name,
            hasSubscribers: false,
            publish() {},
            subscribe() {},
            unsubscribe() { return false; },
            bindStore() {},
            unbindStore() {},
            runStores(_message, callback, thisArg, ...args) { return callback.apply(thisArg, args); }
        };
    }
    const diagnosticsChannels = new Map();
    const diagnosticsChannel = {
        channel(name) {
            if (!diagnosticsChannels.has(name)) diagnosticsChannels.set(name, createDiagnosticsChannel(name));
            return diagnosticsChannels.get(name);
        },
        hasSubscribers() { return false; },
        subscribe() {},
        unsubscribe() { return false; },
        tracingChannel(name) {
            const channels = {
                start: this.channel(`tracing:${name}:start`),
                end: this.channel(`tracing:${name}:end`),
                asyncStart: this.channel(`tracing:${name}:asyncStart`),
                asyncEnd: this.channel(`tracing:${name}:asyncEnd`),
                error: this.channel(`tracing:${name}:error`)
            };
            Object.defineProperty(channels, "hasSubscribers", {
                get() {
                    return channels.start.hasSubscribers || channels.end.hasSubscribers ||
                        channels.asyncStart.hasSubscribers || channels.asyncEnd.hasSubscribers ||
                        channels.error.hasSubscribers;
                }
            });
            return channels;
        }
    };

    function depd() {
        function deprecate() {}
        deprecate.function = function (fn) { return fn; };
        deprecate.property = function () {};
        return deprecate;
    }

    function assertionError(message) {
        const error = new Error(message || "Assertion failed");
        error.name = "AssertionError";
        error.code = "ERR_ASSERTION";
        return error;
    }
    function assert(value, message) {
        if (!value) throw assertionError(message);
    }
    assert.ok = assert;
    assert.equal = (actual, expected, message) => { if (actual != expected) throw assertionError(message); };
    assert.strictEqual = (actual, expected, message) => { if (actual !== expected) throw assertionError(message); };
    assert.notStrictEqual = (actual, expected, message) => { if (actual === expected) throw assertionError(message); };
    assert.fail = message => { throw assertionError(message); };
    assert.AssertionError = function AssertionError(options = {}) { return assertionError(options.message); };

    process.cwd = () => __canaryoCwd();
    process.platform = "win32";
    process.version = "v22.0.0-canaryo";
    process.versions = { node: "22.0.0", canaryo: "0.1.0" };
    process.nextTick = (callback, ...args) => Promise.resolve().then(() => callback(...args));
    process.stdout = { isTTY: false, write(value) { __canaryoWrite(String(value)); return true; } };
    process.stderr = { isTTY: false, write(value) { __canaryoWriteError(String(value)); return true; } };
    const performance = { now: () => Date.now(), timeOrigin: Date.now() };
    const scheduledTimers = new Map();
    let nextTimerId = 1;

    function scheduleTimer(callback, delay, repeat, args) {
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        const timeout = Math.max(0, Number(delay) || 0);
        const id = nextTimerId++;
        const timer = {
            id,
            callback,
            args,
            delay: timeout,
            repeat,
            due: Date.now() + timeout,
            referenced: true
        };
        const handle = {
            id,
            ref() { timer.referenced = true; return this; },
            unref() { timer.referenced = false; return this; },
            hasRef() { return timer.referenced; },
            refresh() { timer.due = Date.now() + timer.delay; scheduledTimers.set(id, timer); return this; },
            [Symbol.toPrimitive]() { return id; }
        };
        timer.handle = handle;
        scheduledTimers.set(id, timer);
        return handle;
    }

    function clearTimer(handle) {
        const id = handle && typeof handle === "object" ? handle.id : Number(handle);
        scheduledTimers.delete(id);
    }

    function setTimeout(callback, delay, ...args) {
        return scheduleTimer(callback, delay, false, args);
    }
    function setInterval(callback, delay, ...args) {
        return scheduleTimer(callback, delay, true, args);
    }
    function setImmediate(callback, ...args) {
        return scheduleTimer(callback, 0, false, args);
    }

    globalThis.__canaryoRunTimers = () => {
        const now = Date.now();
        for (const timer of [...scheduledTimers.values()]) {
            if (timer.due > now || !scheduledTimers.has(timer.id)) continue;
            if (timer.repeat) timer.due = now + timer.delay;
            else scheduledTimers.delete(timer.id);
            timer.callback(...timer.args);
        }

        let nextDelay;
        const updatedNow = Date.now();
        for (const timer of scheduledTimers.values()) {
            const delay = Math.max(0, timer.due - updatedNow);
            if (nextDelay === undefined || delay < nextDelay) nextDelay = delay;
        }
        return nextDelay;
    };

    globalThis.Buffer = Buffer;
    globalThis.TextEncoder = TextEncoder;
    globalThis.TextDecoder = TextDecoder;
    globalThis.performance = performance;
    globalThis.setImmediate = setImmediate;
    globalThis.clearImmediate = clearTimer;
    globalThis.setTimeout = setTimeout;
    globalThis.clearTimeout = clearTimer;
    globalThis.setInterval = setInterval;
    globalThis.clearInterval = clearTimer;
    globalThis.queueMicrotask = callback => Promise.resolve().then(callback);

    function Stats(values) {
        Object.assign(this, values);
        this.mtime = new Date(this.mtimeMs);
    }
    Stats.prototype.isFile = function () { return this.file; };
    Stats.prototype.isDirectory = function () { return this.directory; };
    Stats.prototype.isSymbolicLink = function () { return this.symlink; };
    Stats.prototype.isBlockDevice = Stats.prototype.isCharacterDevice =
        Stats.prototype.isFIFO = Stats.prototype.isSocket = function () { return false; };

    function encodingFrom(options) {
        return typeof options === "string" ? options : options && options.encoding;
    }
    function readFileSync(filename, options) {
        const buffer = Buffer.from(__canaryoFsRead(String(filename)));
        return encodingFrom(options) ? buffer.toString(encodingFrom(options)) : buffer;
    }
    function writeFileSync(filename, value, _options) {
        __canaryoFsWrite(String(filename), [...Buffer.from(value)], false);
    }
    function appendFileSync(filename, value, _options) {
        __canaryoFsWrite(String(filename), [...Buffer.from(value)], true);
    }
    function statSync(filename) { return new Stats(__canaryoFsStat(String(filename))); }
    function existsSync(filename) { return __canaryoFsExists(String(filename)); }
    function accessSync(filename) {
        if (!existsSync(filename)) throw new Error(`ENOENT: no such file or directory, access '${filename}'`);
    }
    function mkdirSync(filename, options) {
        const recursive = options === true || Boolean(options && options.recursive);
        __canaryoFsMkdir(String(filename), recursive);
    }
    function readdirSync(filename, _options) { return [...__canaryoFsReaddir(String(filename))]; }
    function callbackOperation(callback, operation) {
        queueMicrotask(() => {
            try { callback(null, operation()); }
            catch (error) { callback(error); }
        });
    }
    const fsPromises = {
        readFile(filename, options) { return Promise.resolve().then(() => readFileSync(filename, options)); },
        writeFile(filename, value, options) { return Promise.resolve().then(() => writeFileSync(filename, value, options)); },
        appendFile(filename, value, options) { return Promise.resolve().then(() => appendFileSync(filename, value, options)); },
        stat(filename) { return Promise.resolve().then(() => statSync(filename)); },
        access(filename) { return Promise.resolve().then(() => accessSync(filename)); },
        mkdir(filename, options) { return Promise.resolve().then(() => mkdirSync(filename, options)); },
        readdir(filename, options) { return Promise.resolve().then(() => readdirSync(filename, options)); }
    };
    const fsModule = {
        Stats,
        constants: { F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1 },
        promises: fsPromises,
        readFileSync,
        writeFileSync,
        appendFileSync,
        statSync,
        existsSync,
        accessSync,
        mkdirSync,
        readdirSync,
        readFile(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => readFileSync(filename, options));
        },
        writeFile(filename, value, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => writeFileSync(filename, value, options));
        },
        appendFile(filename, value, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => appendFileSync(filename, value, options));
        },
        stat(filename, callback) { callbackOperation(callback, () => statSync(filename)); },
        mkdir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => mkdirSync(filename, options));
        },
        readdir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => readdirSync(filename, options));
        },
        createReadStream() { throw new Error("fs.createReadStream ainda não implementado"); }
    };

    globalThis.__canaryoBuiltins = Object.freeze({
        assert,
        async_hooks: { AsyncLocalStorage, AsyncResource, executionAsyncId: () => 0, triggerAsyncId: () => 0 },
        buffer: { Buffer, SlowBuffer: Buffer, INSPECT_MAX_BYTES: 50, kMaxLength: 0x7fffffff },
        crypto: {
            createHash(algorithm) {
                let contents = "";
                return {
                    update(value) { contents += String(value); return this; },
                    digest(encoding = "hex") { return __canaryoHash(algorithm, contents, encoding); }
                };
            }
        },
        depd,
        diagnostics_channel: diagnosticsChannel,
        dns: {
            lookup(hostname, options, callback) {
                if (typeof options === "function") callback = options;
                callback(null, [{ address: hostname === "localhost" ? "127.0.0.1" : hostname, family: 4 }]);
            }
        },
        events: Object.assign(EventEmitter, { EventEmitter }),
        fs: fsModule,
        "fs/promises": fsPromises,
        net: { isIP: () => 0, isIPv4: () => false, isIPv6: () => false },
        os: { networkInterfaces: () => ({}) },
        path,
        perf_hooks: { performance },
        querystring: { parse(value) { return Object.fromEntries(String(value).split("&").filter(Boolean).map(item => item.split("=").map(decodeURIComponent))); }, stringify(value) { return Object.entries(value).map(([key, item]) => `${encodeURIComponent(key)}=${encodeURIComponent(item)}`).join("&"); }, escape: encodeURIComponent, unescape: decodeURIComponent },
        stream: Stream,
        string_decoder: { StringDecoder },
        timers: { setImmediate, clearImmediate: clearTimer, setTimeout, clearTimeout: clearTimer, setInterval, clearInterval: clearTimer },
        tty: { isatty: () => false, ReadStream: function () {}, WriteStream: function () {} },
        url: { parse: parseUrl, format: value => value.href || value.path || String(value), resolve: (base, target) => target.startsWith("/") ? target : path.join(path.dirname(base), target) },
        util,
        zlib: { constants: {} }
    });
})();
