(() => {
    function EventEmitter() { this._events = Object.create(null); }
    EventEmitter.prototype.on = EventEmitter.prototype.addListener = function (name, listener) {
        if (typeof listener !== "function") throw new TypeError("listener must be a function");
        const events = this._events || (this._events = Object.create(null));
        if (name !== "newListener" && events.newListener) this.emit("newListener", name, listener);
        const listeners = events[name] || (events[name] = []);
        listeners.push(listener);
        return this;
    };
    EventEmitter.prototype.prependListener = function (name, listener) {
        if (typeof listener !== "function") throw new TypeError("listener must be a function");
        const events = this._events || (this._events = Object.create(null));
        if (name !== "newListener" && events.newListener) this.emit("newListener", name, listener);
        const listeners = events[name] || (events[name] = []);
        listeners.unshift(listener);
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
    EventEmitter.prototype.prependOnceListener = function (name, listener) {
        const emitter = this;
        function onceListener(...args) {
            emitter.removeListener(name, onceListener);
            listener.apply(emitter, args);
        }
        onceListener.listener = listener;
        return this.prependListener(name, onceListener);
    };
    EventEmitter.prototype.emit = function (name, ...args) {
        const listeners = this._events && this._events[name];
        if (!listeners || listeners.length === 0) {
            if (name === "error") {
                throw args[0] instanceof Error ? args[0] : new Error(`Unhandled error: ${args[0]}`);
            }
            return false;
        }
        for (const listener of [...listeners]) listener.apply(this, args);
        return true;
    };
    EventEmitter.prototype.removeListener = function (name, listener) {
        const listeners = this._events && this._events[name];
        if (!listeners) return this;
        const retained = listeners.filter(item => item !== listener && item.listener !== listener);
        if (retained.length === listeners.length) return this;
        if (retained.length) this._events[name] = retained;
        else delete this._events[name];
        if (name !== "removeListener" && this._events.removeListener) {
            this.emit("removeListener", name, listener);
        }
        return this;
    };
    EventEmitter.prototype.off = EventEmitter.prototype.removeListener;
    EventEmitter.prototype.removeAllListeners = function (name) {
        if (name === undefined) this._events = Object.create(null);
        else if (this._events) delete this._events[name];
        return this;
    };
    EventEmitter.prototype.listeners = function (name) {
        return (this._events && this._events[name] || []).map(listener => listener.listener || listener);
    };
    EventEmitter.prototype.rawListeners = function (name) {
        return [...(this._events && this._events[name] || [])];
    };
    EventEmitter.prototype.listenerCount = function (name) { return this.listeners(name).length; };
    EventEmitter.prototype.eventNames = function () { return Reflect.ownKeys(this._events || {}); };
    EventEmitter.prototype.setMaxListeners = function (value) { this._maxListeners = Number(value); return this; };
    EventEmitter.prototype.getMaxListeners = function () { return this._maxListeners ?? 10; };
    EventEmitter.once = function (emitter, name) {
        return new Promise((resolve, reject) => {
            function cleanup() {
                emitter.removeListener(name, onEvent);
                if (name !== "error") emitter.removeListener("error", onError);
            }
            function onEvent(...args) { cleanup(); resolve(args); }
            function onError(error) { cleanup(); reject(error); }
            emitter.once(name, onEvent);
            if (name !== "error") emitter.once("error", onError);
        });
    };
    EventEmitter.getEventListeners = (emitter, name) => emitter.listeners(name);
    EventEmitter.listenerCount = (emitter, name) => emitter.listenerCount(name);
    EventEmitter.setMaxListeners = (value, ...emitters) => {
        for (const emitter of emitters) emitter.setMaxListeners(value);
    };

    function encodeUtf8(value) {
        const bytes = [];
        for (const character of String(value)) {
            let code = character.codePointAt(0);
            if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd;
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
    function normalizeEncoding(encoding = "utf8") {
        const value = String(encoding).toLowerCase().replace(/[-_]/g, "");
        if (value === "utf8" || value === "utf") return "utf8";
        if (value === "utf16le" || value === "ucs2" || value === "ucs2le") return "utf16le";
        if (value === "latin1" || value === "binary") return "latin1";
        if (value === "ascii" || value === "hex" || value === "base64" || value === "base64url") return value;
        throw new TypeError(`Unknown encoding: ${encoding}`);
    }
    function decodeBase64(value) {
        const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        const input = String(value).replace(/-/g, "+").replace(/_/g, "/").replace(/[^A-Za-z0-9+/]/g, "");
        const output = [];
        let accumulator = 0;
        let bits = 0;
        for (const character of input) {
            const digit = alphabet.indexOf(character);
            if (digit < 0) continue;
            accumulator = accumulator << 6 | digit;
            bits += 6;
            if (bits >= 8) {
                bits -= 8;
                output.push(accumulator >> bits & 0xff);
            }
        }
        return output;
    }
    function encodeBase64(bytes, urlSafe = false) {
        const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let output = "";
        for (let index = 0; index < bytes.length; index += 3) {
            const remaining = bytes.length - index;
            const value = bytes[index] << 16 | (bytes[index + 1] || 0) << 8 | (bytes[index + 2] || 0);
            output += alphabet[value >> 18 & 63] + alphabet[value >> 12 & 63];
            output += remaining > 1 ? alphabet[value >> 6 & 63] : "=";
            output += remaining > 2 ? alphabet[value & 63] : "=";
        }
        return urlSafe ? output.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "") : output;
    }
    function encodeString(value, encoding) {
        const normalized = normalizeEncoding(encoding);
        if (normalized === "utf8") return encodeUtf8(value);
        if (normalized === "hex") {
            const input = String(value).match(/^[0-9a-fA-F]*/)[0];
            const bytes = [];
            for (let index = 0; index + 1 < input.length; index += 2) bytes.push(parseInt(input.slice(index, index + 2), 16));
            return bytes;
        }
        if (normalized === "base64" || normalized === "base64url") return decodeBase64(value);
        if (normalized === "utf16le") {
            const bytes = [];
            const input = String(value);
            for (let index = 0; index < input.length; index++) {
                const code = input.charCodeAt(index);
                bytes.push(code & 0xff, code >> 8);
            }
            return bytes;
        }
        const input = String(value);
        const bytes = [];
        for (let index = 0; index < input.length; index++) bytes.push(input.charCodeAt(index) & (normalized === "ascii" ? 0x7f : 0xff));
        return bytes;
    }
    function decodeBytes(bytes, encoding) {
        const normalized = normalizeEncoding(encoding);
        if (normalized === "utf8") return decodeUtf8(bytes);
        if (normalized === "hex") return [...bytes].map(byte => byte.toString(16).padStart(2, "0")).join("");
        if (normalized === "base64" || normalized === "base64url") return encodeBase64(bytes, normalized === "base64url");
        if (normalized === "utf16le") {
            let output = "";
            for (let index = 0; index + 1 < bytes.length; index += 2) output += String.fromCharCode(bytes[index] | bytes[index + 1] << 8);
            return output;
        }
        return [...bytes].map(byte => String.fromCharCode(normalized === "ascii" ? byte & 0x7f : byte)).join("");
    }
    class Buffer extends Uint8Array {
        static from(value, encodingOrOffset, length) {
            if (typeof value === "string") return new Buffer(encodeString(value, encodingOrOffset));
            if (ArrayBuffer.isView(value)) return new Buffer(value);
            if (value instanceof ArrayBuffer) return new Buffer(value, encodingOrOffset, length);
            if (value && value.type === "Buffer" && Array.isArray(value.data)) return new Buffer(value.data);
            return new Buffer(value);
        }
        static alloc(size, fill = 0, encoding) {
            const buffer = new Buffer(size);
            if (typeof fill === "string") {
                const pattern = Buffer.from(fill, encoding);
                for (let index = 0; index < buffer.length; index++) buffer[index] = pattern[index % pattern.length];
            } else buffer.fill(fill);
            return buffer;
        }
        static allocUnsafe(size) { return new Buffer(size); }
        static isBuffer(value) { return value instanceof Buffer; }
        static isEncoding(value) { try { normalizeEncoding(value); return true; } catch { return false; } }
        static byteLength(value, encoding) {
            if (typeof value === "string") return !encoding || normalizeEncoding(encoding) === "utf8" ? __canaryoByteLength(value) : encodeString(value, encoding).length;
            if (ArrayBuffer.isView(value)) return value.byteLength;
            if (value instanceof ArrayBuffer) return value.byteLength;
            return Buffer.from(value).length;
        }
        static compare(left, right) { return Buffer.from(left).compare(right); }
        static concat(list, totalLength) {
            const length = totalLength ?? list.reduce((sum, item) => sum + item.length, 0);
            const result = new Buffer(length);
            let offset = 0;
            for (const item of list) {
                const count = Math.min(item.length, length - offset);
                if (count <= 0) break;
                result.set(item.subarray(0, count), offset);
                offset += count;
            }
            return result;
        }
        toString(encoding = "utf8", start = 0, end = this.length) { return decodeBytes(this.subarray(start, end), encoding); }
        equals(other) { return this.compare(other) === 0; }
        compare(other) {
            const right = Buffer.from(other);
            const length = Math.min(this.length, right.length);
            for (let index = 0; index < length; index++) if (this[index] !== right[index]) return this[index] < right[index] ? -1 : 1;
            return this.length === right.length ? 0 : this.length < right.length ? -1 : 1;
        }
        copy(target, targetStart = 0, sourceStart = 0, sourceEnd = this.length) {
            const source = this.subarray(sourceStart, sourceEnd);
            const count = Math.min(source.length, target.length - targetStart);
            if (count > 0) target.set(source.subarray(0, count), targetStart);
            return Math.max(0, count);
        }
        slice(start, end) { return this.subarray(start, end); }
        indexOf(value, byteOffset = 0, encoding) {
            const needle = typeof value === "number" ? Buffer.from([value & 0xff]) : Buffer.from(value, encoding);
            let start = Number(byteOffset) || 0;
            if (start < 0) start = Math.max(0, this.length + start);
            for (let index = start; index <= this.length - needle.length; index++) {
                let matches = true;
                for (let offset = 0; offset < needle.length; offset++) {
                    if (this[index + offset] !== needle[offset]) { matches = false; break; }
                }
                if (matches) return index;
            }
            return -1;
        }
        includes(value, byteOffset, encoding) { return this.indexOf(value, byteOffset, encoding) !== -1; }
        write(value, offset = 0, length = this.length - offset, encoding = "utf8") {
            if (typeof length === "string") { encoding = length; length = this.length - offset; }
            const source = Buffer.from(value, encoding);
            const count = Math.min(source.length, length, this.length - offset);
            this.set(source.subarray(0, count), offset);
            return count;
        }
        readUInt8(offset = 0) { return this[offset]; }
        readUInt16LE(offset = 0) { return this[offset] | this[offset + 1] << 8; }
        readUInt16BE(offset = 0) { return this[offset] << 8 | this[offset + 1]; }
        readUInt32LE(offset = 0) { return (this[offset] | this[offset + 1] << 8 | this[offset + 2] << 16 | this[offset + 3] << 24) >>> 0; }
        readUInt32BE(offset = 0) { return (this[offset] << 24 | this[offset + 1] << 16 | this[offset + 2] << 8 | this[offset + 3]) >>> 0; }
        writeUInt8(value, offset = 0) { this[offset] = value; return offset + 1; }
        writeUInt16LE(value, offset = 0) { this[offset] = value; this[offset + 1] = value >> 8; return offset + 2; }
        writeUInt16BE(value, offset = 0) { this[offset] = value >> 8; this[offset + 1] = value; return offset + 2; }
        writeUInt32LE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> index * 8; return offset + 4; }
        writeUInt32BE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> (3 - index) * 8; return offset + 4; }
        toJSON() { return { type: "Buffer", data: [...this] }; }
    }
    for (const method of ["from", "alloc", "allocUnsafe", "isBuffer", "isEncoding", "byteLength", "compare", "concat"]) {
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
    class URLSearchParams {
        constructor(value = "", onChange) {
            this._entries = [];
            this._onChange = onChange;
            if (typeof value === "string") {
                const input = value.startsWith("?") ? value.slice(1) : value;
                for (const item of input.split("&")) {
                    if (!item) continue;
                    const [name, entry = ""] = item.split("=");
                    this._entries.push([
                        decodeURIComponent(name.replace(/\+/g, " ")),
                        decodeURIComponent(entry.replace(/\+/g, " "))
                    ]);
                }
            } else if (value && typeof value[Symbol.iterator] === "function") {
                for (const [name, entry] of value) this._entries.push([String(name), String(entry)]);
            } else if (value) {
                for (const [name, entry] of Object.entries(value)) this._entries.push([name, String(entry)]);
            }
        }
        append(name, value) { this._entries.push([String(name), String(value)]); this._changed(); }
        delete(name, value) {
            name = String(name);
            this._entries = this._entries.filter(entry => entry[0] !== name || value !== undefined && entry[1] !== String(value));
            this._changed();
        }
        get(name) { const entry = this._entries.find(entry => entry[0] === String(name)); return entry ? entry[1] : null; }
        getAll(name) { return this._entries.filter(entry => entry[0] === String(name)).map(entry => entry[1]); }
        has(name, value) { return this._entries.some(entry => entry[0] === String(name) && (value === undefined || entry[1] === String(value))); }
        set(name, value) {
            name = String(name); value = String(value);
            const index = this._entries.findIndex(entry => entry[0] === name);
            this._entries = this._entries.filter(entry => entry[0] !== name);
            this._entries.splice(index < 0 ? this._entries.length : index, 0, [name, value]);
            this._changed();
        }
        sort() { this._entries.sort((left, right) => left[0].localeCompare(right[0])); this._changed(); }
        entries() { return this._entries[Symbol.iterator](); }
        keys() { return this._entries.map(entry => entry[0])[Symbol.iterator](); }
        values() { return this._entries.map(entry => entry[1])[Symbol.iterator](); }
        forEach(callback, thisArg) { for (const [name, value] of this._entries) callback.call(thisArg, value, name, this); }
        toString() { return this._entries.map(([name, value]) => `${encodeURIComponent(name).replace(/%20/g, "+")}=${encodeURIComponent(value).replace(/%20/g, "+")}`).join("&"); }
        _changed() { if (this._onChange) this._onChange(); }
        [Symbol.iterator]() { return this.entries(); }
        get size() { return this._entries.length; }
    }

    function normalizeUrlPathname(value) {
        const trailingSlash = value.endsWith("/");
        const output = [];
        for (const part of value.split("/")) {
            if (!part || part === ".") continue;
            if (part === "..") output.pop();
            else output.push(part);
        }
        return `/${output.join("/")}${trailingSlash && output.length ? "/" : ""}`;
    }

    class URL {
        constructor(input, base) {
            input = String(input);
            if (!/^[A-Za-z][A-Za-z\d+.-]*:/.test(input)) {
                if (base === undefined) throw new TypeError("Invalid URL");
                const parent = base instanceof URL ? base : new URL(base);
                if (input.startsWith("/")) input = parent.origin + input;
                else input = parent.origin + parent.pathname.replace(/[^/]*$/, "") + input;
            }
            const match = input.match(/^([A-Za-z][A-Za-z\d+.-]*:)(?:\/\/([^/?#]*))?([^?#]*)(\?[^#]*)?(#.*)?$/);
            if (!match) throw new TypeError("Invalid URL");
            this.protocol = match[1].toLowerCase();
            const authority = match[2] || "";
            const at = authority.lastIndexOf("@");
            const credentials = at >= 0 ? authority.slice(0, at) : "";
            const host = at >= 0 ? authority.slice(at + 1) : authority;
            const colon = credentials.indexOf(":");
            this.username = decodeURIComponent(colon < 0 ? credentials : credentials.slice(0, colon));
            this.password = decodeURIComponent(colon < 0 ? "" : credentials.slice(colon + 1));
            const portSeparator = host.startsWith("[") ? host.indexOf("]") + 1 : host.lastIndexOf(":");
            this.hostname = portSeparator > 0 && host[portSeparator] === ":" ? host.slice(0, portSeparator) : host;
            this.port = portSeparator > 0 && host[portSeparator] === ":" ? host.slice(portSeparator + 1) : "";
            this.pathname = authority || this.protocol === "file:"
                ? normalizeUrlPathname(match[3] || "/")
                : match[3];
            this.hash = match[5] || "";
            this.searchParams = new URLSearchParams(match[4] || "");
        }
        get host() { return this.hostname + (this.port ? `:${this.port}` : ""); }
        set host(value) { const parsed = new URL(`${this.protocol}//${value}${this.pathname}`); this.hostname = parsed.hostname; this.port = parsed.port; }
        get origin() { return this.protocol === "file:" ? "null" : `${this.protocol}//${this.host}`; }
        get search() { const value = this.searchParams.toString(); return value ? `?${value}` : ""; }
        set search(value) { this.searchParams = new URLSearchParams(value); }
        get href() {
            const credentials = this.username ? `${encodeURIComponent(this.username)}${this.password ? `:${encodeURIComponent(this.password)}` : ""}@` : "";
            const authority = this.host || this.protocol === "file:" ? `//${credentials}${this.host}` : "";
            return `${this.protocol}${authority}${this.pathname}${this.search}${this.hash}`;
        }
        set href(value) { const parsed = new URL(value); Object.assign(this, parsed); }
        toString() { return this.href; }
        toJSON() { return this.href; }
    }
    function fileURLToPath(value) {
        const url = value instanceof URL ? value : new URL(value);
        if (url.protocol !== "file:") throw new TypeError("URL must use file: protocol");
        let pathname = decodeURIComponent(url.pathname);
        if (process.platform === "win32" && /^\/[A-Za-z]:/.test(pathname)) pathname = pathname.slice(1).replace(/\//g, "\\");
        return pathname;
    }
    function pathToFileURL(value) {
        let pathname = path.resolve(String(value)).replace(/\\/g, "/");
        if (!pathname.startsWith("/")) pathname = `/${pathname}`;
        return new URL(`file://${encodeURI(pathname)}`);
    }
    function Stream() { EventEmitter.call(this); this.destroyed = false; }
    util.inherits(Stream, EventEmitter);
    Stream.prototype.pipe = function (destination) {
        this.on("data", chunk => destination.write(chunk));
        this.on("end", () => destination.end());
        this.on("error", error => destination.destroy(error));
        destination.emit("pipe", this);
        return destination;
    };
    Stream.prototype.destroy = function (error) {
        if (this.destroyed) return this;
        this.destroyed = true;
        if (error) this.emit("error", error);
        this.emit("close");
        return this;
    };

    function Readable(options = {}) {
        Stream.call(this);
        this.readable = true;
        this.readableEnded = false;
        this._readableQueue = [];
        if (typeof options.read === "function") this._read = options.read;
    }
    util.inherits(Readable, Stream);
    Readable.prototype._read = function () {};
    Readable.prototype.push = function (chunk) {
        if (chunk === null) {
            this.readable = false;
            this.readableEnded = true;
            this.emit("end");
            return false;
        }
        const value = typeof chunk === "string" ? Buffer.from(chunk) : chunk;
        this._readableQueue.push(value);
        this.emit("data", value);
        this.emit("readable");
        return true;
    };
    Readable.prototype.read = function () { return this._readableQueue.shift() ?? null; };
    Readable.prototype.pause = function () { this._paused = true; return this; };
    Readable.prototype.resume = function () { this._paused = false; return this; };
    Readable.from = function (iterable) {
        const readable = new Readable();
        setImmediate(() => {
            for (const chunk of iterable) readable.push(chunk);
            readable.push(null);
        });
        return readable;
    };

    function Writable(options = {}) {
        Stream.call(this);
        this.writable = true;
        this.writableEnded = false;
        this.writableFinished = false;
        if (typeof options.write === "function") this._write = options.write;
        if (typeof options.final === "function") this._final = options.final;
    }
    util.inherits(Writable, Stream);
    Writable.prototype._write = function (_chunk, _encoding, callback) { callback(); };
    Writable.prototype.write = function (chunk, encoding, callback) {
        if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        const done = error => {
            if (error) this.emit("error", error);
            if (callback) callback(error);
        };
        this._write(chunk, encoding || "utf8", done);
        return true;
    };
    Writable.prototype.end = function (chunk, encoding, callback) {
        if (typeof chunk === "function") { callback = chunk; chunk = undefined; }
        else if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        if (chunk !== undefined) this.write(chunk, encoding);
        const finish = error => {
            if (error) { this.emit("error", error); if (callback) callback(error); return; }
            this.writable = false;
            this.writableEnded = true;
            this.writableFinished = true;
            this.emit("finish");
            if (callback) callback();
        };
        if (this._final) this._final(finish); else finish();
        return this;
    };

    function Duplex(options = {}) {
        Readable.call(this, options);
        this.writable = true;
        this.writableEnded = false;
        this.writableFinished = false;
        if (typeof options.write === "function") this._write = options.write;
        if (typeof options.final === "function") this._final = options.final;
    }
    util.inherits(Duplex, Readable);
    for (const name of ["_write", "write", "end"]) Duplex.prototype[name] = Writable.prototype[name];

    function Transform(options = {}) {
        Duplex.call(this, options);
        if (typeof options.transform === "function") this._transform = options.transform;
    }
    util.inherits(Transform, Duplex);
    Transform.prototype._transform = function (chunk, _encoding, callback) { callback(null, chunk); };
    Transform.prototype._write = function (chunk, encoding, callback) {
        this._transform(chunk, encoding, (error, output) => {
            if (!error && output !== undefined && output !== null) this.push(output);
            callback(error);
        });
    };
    Transform.prototype.end = function (chunk, encoding, callback) {
        if (typeof chunk === "function") { callback = chunk; chunk = undefined; }
        else if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        return Writable.prototype.end.call(this, chunk, encoding, error => {
            if (!error) this.push(null);
            if (callback) callback(error);
        });
    };

    function PassThrough(options) { Transform.call(this, options); }
    util.inherits(PassThrough, Transform);
    PassThrough.prototype._transform = function (chunk, _encoding, callback) { callback(null, chunk); };

    function finished(stream, callback) {
        const event = stream.writable ? "finish" : "end";
        stream.once(event, () => callback());
        stream.once("error", callback);
        return () => stream.removeListener(event, callback);
    }
    function pipeline(...streams) {
        const callback = typeof streams[streams.length - 1] === "function" ? streams.pop() : () => {};
        for (let index = 0; index + 1 < streams.length; index++) streams[index].pipe(streams[index + 1]);
        finished(streams[streams.length - 1], callback);
        return streams[streams.length - 1];
    }
    Object.assign(Stream, { Stream, Readable, Writable, Duplex, Transform, PassThrough, finished, pipeline });

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

    const processStartedAt = Date.now();
    EventEmitter.call(process);
    Object.setPrototypeOf(process, EventEmitter.prototype);
    process.cwd = () => __canaryoCwd();
    process.platform = "win32";
    process.arch = "x64";
    process.version = "v22.0.0-canaryo";
    process.versions = { node: "22.0.0", canaryo: "0.1.0" };
    process.release = { name: "canaryo", sourceUrl: "", headersUrl: "" };
    process.argv0 = "canaryo";
    process.execArgv = [];
    process.title = "canaryo";
    process.config = { variables: {} };
    process.moduleLoadList = [];
    process.exitCode = undefined;
    process.nextTick = (callback, ...args) => Promise.resolve().then(() => callback(...args));
    process.uptime = () => (Date.now() - processStartedAt) / 1000;
    process.hrtime = previous => {
        const nanoseconds = BigInt(Date.now() - processStartedAt) * 1000000n;
        let seconds = Number(nanoseconds / 1000000000n);
        let remainder = Number(nanoseconds % 1000000000n);
        if (previous) {
            seconds -= Number(previous[0]);
            remainder -= Number(previous[1]);
            if (remainder < 0) { seconds -= 1; remainder += 1000000000; }
        }
        return [seconds, remainder];
    };
    process.hrtime.bigint = () => BigInt(Date.now() - processStartedAt) * 1000000n;
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
    globalThis.URL = URL;
    globalThis.URLSearchParams = URLSearchParams;
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
        url: {
            URL,
            URLSearchParams,
            parse: parseUrl,
            format: value => value.href || value.path || String(value),
            resolve: (base, target) => target.startsWith("/") ? target : path.join(path.dirname(base), target),
            fileURLToPath,
            pathToFileURL,
            domainToASCII: value => String(value),
            domainToUnicode: value => String(value),
            urlToHttpOptions(value) {
                const url = value instanceof URL ? value : new URL(value);
                return { protocol: url.protocol, hostname: url.hostname, port: url.port, path: url.pathname + url.search, href: url.href };
            }
        },
        util,
        zlib: { constants: {} }
    });
})();
