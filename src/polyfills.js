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
        static byteLength(value) { return Buffer.from(value).length; }
        static concat(list, totalLength) {
            const length = totalLength ?? list.reduce((sum, item) => sum + item.length, 0);
            const result = new Buffer(length);
            let offset = 0;
            for (const item of list) { result.set(item, offset); offset += item.length; }
            return result;
        }
        toString() { return decodeUtf8(this); }
    }

    class TextEncoder {
        encode(value) { return new Uint8Array(encodeUtf8(value)); }
    }

    class TextDecoder {
        decode(value = new Uint8Array()) { return decodeUtf8(value); }
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

    globalThis.Buffer = Buffer;
    globalThis.TextEncoder = TextEncoder;
    globalThis.TextDecoder = TextDecoder;
    globalThis.performance = performance;
    globalThis.setImmediate = (callback, ...args) => Promise.resolve().then(() => callback(...args));
    globalThis.clearImmediate = () => {};
    globalThis.setTimeout = () => ({ ref() { return this; }, unref() { return this; } });
    globalThis.clearTimeout = () => {};
    globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
    globalThis.__canaryoBuiltins = Object.freeze({
        assert,
        async_hooks: { AsyncLocalStorage },
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
        fs: { Stats: function Stats() {}, statSync() { throw new Error("fs.statSync ainda não implementado"); }, stat(_path, callback) { callback(new Error("fs.stat ainda não implementado")); }, createReadStream() { throw new Error("fs.createReadStream ainda não implementado"); } },
        net: { isIP: () => 0, isIPv4: () => false, isIPv6: () => false },
        os: { networkInterfaces: () => ({}) },
        path,
        perf_hooks: { performance },
        querystring: { parse(value) { return Object.fromEntries(String(value).split("&").filter(Boolean).map(item => item.split("=").map(decodeURIComponent))); }, stringify(value) { return Object.entries(value).map(([key, item]) => `${encodeURIComponent(key)}=${encodeURIComponent(item)}`).join("&"); }, escape: encodeURIComponent, unescape: decodeURIComponent },
        stream: Stream,
        tty: { isatty: () => false, ReadStream: function () {}, WriteStream: function () {} },
        url: { parse: parseUrl, format: value => value.href || value.path || String(value), resolve: (base, target) => target.startsWith("/") ? target : path.join(path.dirname(base), target) },
        util,
        zlib: { constants: {} }
    });
})();
