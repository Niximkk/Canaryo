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
    const promisifyCustom = Symbol.for("nodejs.util.promisify.custom");
    function promisify(original) {
        if (typeof original !== "function") throw new TypeError("original must be a function");
        if (typeof original[promisifyCustom] === "function") return original[promisifyCustom];
        function promisified(...args) {
            return new Promise((resolve, reject) => {
                original.call(this, ...args, (error, ...values) => {
                    if (error) reject(error);
                    else resolve(values.length > 1 ? values : values[0]);
                });
            });
        }
        Object.setPrototypeOf(promisified, Object.getPrototypeOf(original));
        return promisified;
    }
    promisify.custom = promisifyCustom;
    function callbackify(original) {
        if (typeof original !== "function") throw new TypeError("original must be a function");
        return function callbackified(...args) {
            const callback = args.pop();
            if (typeof callback !== "function") throw new TypeError("callback must be a function");
            Promise.resolve(original.apply(this, args)).then(
                value => process.nextTick(callback, null, value),
                error => process.nextTick(callback, error || new Error("Promise was rejected with a falsy value"))
            );
        };
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
        promisify,
        callbackify,
        stripVTControlCharacters(value) { return String(value).replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, ""); },
        TextEncoder,
        TextDecoder,
        types: { isDate: value => value instanceof Date, isRegExp: value => value instanceof RegExp, isNativeError: value => value instanceof Error }
    };

    function createPathApi(separator, delimiter, windows) {
        const toSlashes = value => windows ? String(value).replace(/\\/g, "/") : String(value);
        function rootOf(value) {
            const source = toSlashes(value);
            if (windows) {
                const unc = source.match(/^\/\/([^/]+)\/([^/]+)/);
                if (unc) return { root: unc[0], rest: source.slice(unc[0].length), absolute: true };
                const drive = source.match(/^([A-Za-z]:)(\/)?/);
                if (drive) return { root: drive[1], rest: source.slice(drive[0].length), absolute: Boolean(drive[2]) };
            }
            return { root: source.startsWith("/") ? "/" : "", rest: source.replace(/^\/+/, ""), absolute: source.startsWith("/") };
        }
        function normalize(value) {
            const source = toSlashes(value);
            if (!source) return ".";
            const root = rootOf(source);
            const output = [];
            for (const part of root.rest.split("/")) {
                if (!part || part === ".") continue;
                if (part === "..") {
                    if (output.length && output[output.length - 1] !== "..") output.pop();
                    else if (!root.absolute) output.push(part);
                } else output.push(part);
            }
            let result;
            if (windows && root.root.startsWith("//")) result = `${root.root}/${output.join("/")}`;
            else if (windows && root.root) result = `${root.root}${root.absolute ? "/" : ""}${output.join("/")}`;
            else result = `${root.absolute ? "/" : ""}${output.join("/")}`;
            if (!result) result = root.absolute ? "/" : ".";
            if (source.endsWith("/") && result !== "/" && !result.endsWith("/")) result += "/";
            return separator === "/" ? result : result.replace(/\//g, separator);
        }
        function isAbsolute(value) {
            const source = toSlashes(value);
            return windows ? /^[A-Za-z]:\/|^\/\//.test(source) : source.startsWith("/");
        }
        function resolve(...parts) {
            let result = process.cwd();
            for (const part of parts.map(String)) result = isAbsolute(part) ? part : `${result}${separator}${part}`;
            return normalize(result);
        }
        function basename(value, suffix = "") {
            const normalized = toSlashes(normalize(value)).replace(/\/+$/, "");
            let name = normalized.slice(normalized.lastIndexOf("/") + 1);
            if (suffix && name.endsWith(suffix)) name = name.slice(0, -String(suffix).length);
            return name;
        }
        function extname(value) {
            const name = basename(value);
            const index = name.lastIndexOf(".");
            return index <= 0 ? "" : name.slice(index);
        }
        function dirname(value) {
            const normalized = toSlashes(normalize(value)).replace(/\/+$/, "");
            const root = rootOf(normalized);
            const index = normalized.lastIndexOf("/");
            if (index < 0) return root.root || ".";
            if (index === 0) return separator;
            const result = normalized.slice(0, index);
            if (windows && root.absolute && result.toLowerCase() === root.root.toLowerCase()) return `${result}${separator}`;
            return separator === "/" ? result : result.replace(/\//g, separator);
        }
        const api = {
            sep: separator,
            delimiter,
            normalize,
            join(...parts) { return normalize(parts.filter(part => String(part).length).join(separator)); },
            resolve,
            dirname,
            basename,
            extname,
            isAbsolute,
            relative(from, to) {
                const leftValue = toSlashes(resolve(from));
                const rightValue = toSlashes(resolve(to));
                const leftRoot = rootOf(leftValue).root;
                const rightRoot = rootOf(rightValue).root;
                if (windows && leftRoot.toLowerCase() !== rightRoot.toLowerCase()) return normalize(rightValue);
                const left = rootOf(leftValue).rest.split("/").filter(Boolean);
                const right = rootOf(rightValue).rest.split("/").filter(Boolean);
                while (left.length && right.length && (windows ? left[0].toLowerCase() === right[0].toLowerCase() : left[0] === right[0])) { left.shift(); right.shift(); }
                return [...left.map(() => ".."), ...right].join(separator);
            },
            parse(value) {
                const normalized = normalize(value);
                const root = rootOf(normalized).root;
                const dir = dirname(normalized);
                const base = basename(normalized);
                const ext = extname(base);
                const parsedRoot = root && rootOf(normalized).absolute ? `${root}/` : root;
                return { root: separator === "/" ? parsedRoot : parsedRoot.replace(/\//g, separator), dir, base, ext, name: base.slice(0, base.length - ext.length) };
            },
            format(value) {
                const base = value.base || `${value.name || ""}${value.ext || ""}`;
                return value.dir ? `${value.dir}${value.dir.endsWith(separator) ? "" : separator}${base}` : `${value.root || ""}${base}`;
            },
            toNamespacedPath(value) { return String(value); }
        };
        return api;
    }
    const win32Path = createPathApi("\\", ";", true);
    const posixPath = createPathApi("/", ":", false);
    win32Path.win32 = win32Path;
    win32Path.posix = posixPath;
    posixPath.win32 = win32Path;
    posixPath.posix = posixPath;
    const path = __canaryoOsInfo.platform === "win32" ? win32Path : posixPath;

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
    Stream.prototype.pipe = function (destination, options = {}) {
        const source = this;
        const onData = chunk => {
            if (destination.write(chunk) === false && typeof source.pause === "function") source.pause();
        };
        const onDrain = () => {
            if (typeof source.resume === "function") source.resume();
        };
        const onEnd = () => {
            if (options.end !== false) destination.end();
        };
        this.on("data", onData);
        this.on("end", onEnd);
        destination.on("drain", onDrain);
        this.on("error", error => {
            if (typeof destination.destroy === "function") destination.destroy(error);
        });
        destination.once("close", () => {
            source.removeListener("data", onData);
            source.removeListener("end", onEnd);
            destination.removeListener("drain", onDrain);
        });
        destination.emit("pipe", this);
        if (typeof this.resume === "function") this.resume();
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
        this.readableLength = 0;
        this.readableHighWaterMark = Math.max(1, Number(options.highWaterMark) || (options.objectMode ? 16 : 16 * 1024));
        this._readableObjectMode = Boolean(options.objectMode);
        this._paused = true;
        this._flowing = false;
        this._readableEndPending = false;
        if (typeof options.read === "function") this._read = options.read;
    }
    util.inherits(Readable, Stream);
    Readable.prototype._read = function () {};
    Readable.prototype._chunkLength = function (chunk) {
        if (this._readableObjectMode) return 1;
        if (typeof chunk === "string") return Buffer.byteLength(chunk, this._readableEncoding || "utf8");
        return chunk && typeof chunk.length === "number" ? chunk.length : 0;
    };
    Readable.prototype._finishReadable = function () {
        if (!this._readableEndPending || this._readableQueue.length || this.readableEnded) return;
        this._readableEndPending = false;
        this.readable = false;
        this.readableEnded = true;
        this.emit("end");
    };
    Readable.prototype._flow = function () {
        while (this._flowing && !this._paused && this._readableQueue.length) {
            const value = this._readableQueue.shift();
            this.readableLength -= this._chunkLength(value);
            this.emit("data", value);
        }
        this._finishReadable();
    };
    Readable.prototype.push = function (chunk) {
        if (chunk === null) {
            this._readableEndPending = true;
            this._finishReadable();
            return false;
        }
        const value = typeof chunk === "string" && !this._readableEncoding ? Buffer.from(chunk) : chunk;
        this._readableQueue.push(value);
        this.readableLength += this._chunkLength(value);
        this.emit("readable");
        this._flow();
        return this.readableLength < this.readableHighWaterMark;
    };
    Readable.prototype.read = function () {
        const value = this._readableQueue.shift() ?? null;
        if (value !== null) this.readableLength -= this._chunkLength(value);
        this._finishReadable();
        return value;
    };
    Readable.prototype.pause = function () { this._paused = true; this._flowing = false; return this; };
    Readable.prototype.isPaused = function () { return this._paused; };
    Readable.prototype.resume = function () {
        this._paused = false;
        this._flowing = true;
        this._flow();
        return this;
    };
    Readable.prototype.setEncoding = function (encoding) {
        this._readableEncoding = encoding;
        this._readableQueue = this._readableQueue.map(chunk => Buffer.isBuffer(chunk) ? chunk.toString(encoding) : chunk);
        return this;
    };
    Readable.prototype.on = Readable.prototype.addListener = function (name, listener) {
        EventEmitter.prototype.on.call(this, name, listener);
        if (name === "data") this.resume();
        return this;
    };
    Readable.from = function (iterable) {
        const readable = new Readable();
        setImmediate(() => {
            for (const chunk of iterable) readable.push(chunk);
            readable.push(null);
        });
        return readable;
    };

    function initializeWritable(stream, options) {
        stream.writable = true;
        stream.writableEnded = false;
        stream.writableFinished = false;
        stream.writableLength = 0;
        stream.writableHighWaterMark = Math.max(1, Number(options.highWaterMark) || (options.objectMode ? 16 : 16 * 1024));
        stream.writableNeedDrain = false;
        stream._writableObjectMode = Boolean(options.objectMode);
        stream._writableQueue = [];
        stream._writing = false;
        stream._ending = false;
        stream._finalizing = false;
        stream._corked = 0;
        if (typeof options.write === "function") stream._write = options.write;
        if (typeof options.final === "function") stream._final = options.final;
    }
    function Writable(options = {}) {
        Stream.call(this);
        initializeWritable(this, options);
    }
    util.inherits(Writable, Stream);
    Writable.prototype._write = function (_chunk, _encoding, callback) { callback(); };
    Writable.prototype._chunkLength = function (chunk, encoding) {
        if (this._writableObjectMode) return 1;
        if (typeof chunk === "string") return Buffer.byteLength(chunk, encoding);
        return chunk && typeof chunk.length === "number" ? chunk.length : 0;
    };
    Writable.prototype._finishWritable = function () {
        if (!this._ending || this._writing || this._writableQueue.length || this._finalizing || this.writableFinished) return;
        this._finalizing = true;
        const finish = error => {
            this._finalizing = false;
            if (error) { this.emit("error", error); return; }
            this.writable = false;
            this.writableFinished = true;
            this.emit("finish");
        };
        if (this._final) this._final(finish); else finish();
    };
    Writable.prototype._processWritable = function () {
        if (this._writing || this._corked || !this._writableQueue.length) {
            this._finishWritable();
            return;
        }
        const entry = this._writableQueue.shift();
        this._writing = true;
        let called = false;
        const done = error => {
            if (called) return;
            called = true;
            this._writing = false;
            this.writableLength -= entry.length;
            if (error) this.emit("error", error);
            if (entry.callback) entry.callback(error);
            if (this.writableNeedDrain && this.writableLength < this.writableHighWaterMark) {
                this.writableNeedDrain = false;
                this.emit("drain");
            }
            if (!error) this._processWritable();
        };
        try { this._write(entry.chunk, entry.encoding, done); }
        catch (error) { done(error); }
    };
    Writable.prototype.write = function (chunk, encoding, callback) {
        if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        if (this._ending) throw new Error("write after end");
        const normalizedEncoding = encoding || "utf8";
        const length = this._chunkLength(chunk, normalizedEncoding);
        this.writableLength += length;
        this._writableQueue.push({ chunk, encoding: normalizedEncoding, callback, length });
        const belowHighWaterMark = this.writableLength < this.writableHighWaterMark;
        if (!belowHighWaterMark) this.writableNeedDrain = true;
        this._processWritable();
        return belowHighWaterMark;
    };
    Writable.prototype.end = function (chunk, encoding, callback) {
        if (typeof chunk === "function") { callback = chunk; chunk = undefined; }
        else if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
        if (chunk !== undefined) this.write(chunk, encoding);
        if (callback) this.once("finish", callback);
        this._ending = true;
        this.writableEnded = true;
        this._processWritable();
        return this;
    };
    Writable.prototype.cork = function () { this._corked++; };
    Writable.prototype.uncork = function () {
        if (this._corked) this._corked--;
        this._processWritable();
    };

    function Duplex(options = {}) {
        Readable.call(this, options);
        initializeWritable(this, options);
    }
    util.inherits(Duplex, Readable);
    for (const name of ["_write", "_chunkLength", "_finishWritable", "_processWritable", "write", "end", "cork", "uncork"]) {
        Duplex.prototype[name] = Writable.prototype[name];
    }

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
    function normalizeFsPath(filename) {
        return filename instanceof URL ? fileURLToPath(filename) : String(filename);
    }
    function readFileSync(filename, options) {
        const buffer = Buffer.from(__canaryoFsRead(normalizeFsPath(filename)));
        return encodingFrom(options) ? buffer.toString(encodingFrom(options)) : buffer;
    }
    function writeFileSync(filename, value, _options) {
        __canaryoFsWrite(normalizeFsPath(filename), [...Buffer.from(value)], false);
    }
    function appendFileSync(filename, value, _options) {
        __canaryoFsWrite(normalizeFsPath(filename), [...Buffer.from(value)], true);
    }
    function statSync(filename) { return new Stats(__canaryoFsStat(normalizeFsPath(filename))); }
    function existsSync(filename) { return __canaryoFsExists(normalizeFsPath(filename)); }
    function accessSync(filename) {
        if (!existsSync(filename)) throw new Error(`ENOENT: no such file or directory, access '${filename}'`);
    }
    function mkdirSync(filename, options) {
        const recursive = options === true || Boolean(options && options.recursive);
        __canaryoFsMkdir(normalizeFsPath(filename), recursive);
    }
    function readdirSync(filename, _options) { return [...__canaryoFsReaddir(normalizeFsPath(filename))]; }
    function callbackOperation(callback, operation) {
        queueMicrotask(() => {
            try { callback(null, operation()); }
            catch (error) { callback(error); }
        });
    }
    let nextFileDescriptor = 10;
    function ReadStream(filename, options = {}) {
        if (!(this instanceof ReadStream)) return new ReadStream(filename, options);
        if (typeof options === "string") options = { encoding: options };
        Readable.call(this, options);
        this.path = normalizeFsPath(filename);
        this.fd = options.fd ?? null;
        this.flags = options.flags || "r";
        this.mode = options.mode ?? 0o666;
        this.start = Math.max(0, Number(options.start) || 0);
        this.end = options.end === undefined ? Infinity : Math.max(this.start, Number(options.end));
        this.pos = this.start;
        this.bytesRead = 0;
        this.pending = this.fd === null;
        this.closed = false;
        this.autoClose = options.autoClose !== false;
        this.emitClose = options.emitClose !== false;
        this._highWaterMark = Math.max(1, Number(options.highWaterMark) || 64 * 1024);
        this._readableEncoding = options.encoding;
        setImmediate(() => {
            if (this.destroyed) return;
            try {
                const contents = readFileSync(this.path);
                if (this.fd === null) this.fd = nextFileDescriptor++;
                this.pending = false;
                this.emit("open", this.fd);
                this.emit("ready");
                const last = Math.min(contents.length, Number.isFinite(this.end) ? this.end + 1 : contents.length);
                for (let offset = this.start; offset < last; offset += this._highWaterMark) {
                    const chunk = contents.slice(offset, Math.min(last, offset + this._highWaterMark));
                    this.pos = offset + chunk.length;
                    this.bytesRead += chunk.length;
                    this.push(this._readableEncoding ? chunk.toString(this._readableEncoding) : chunk);
                }
                this.push(null);
                if (this.autoClose) this.close();
            } catch (error) {
                this.pending = false;
                this.emit("error", error);
                if (this.autoClose) this.close();
            }
        });
    }
    util.inherits(ReadStream, Readable);
    ReadStream.prototype.close = function (callback) {
        if (typeof callback === "function") this.once("close", callback);
        if (this.closed) return this;
        this.closed = true;
        this.fd = null;
        this.readable = false;
        if (this.emitClose) this.emit("close");
        return this;
    };
    ReadStream.prototype.destroy = function (error) {
        if (this.destroyed) return this;
        this.destroyed = true;
        if (error) this.emit("error", error);
        return this.close();
    };

    function WriteStream(filename, options = {}) {
        if (!(this instanceof WriteStream)) return new WriteStream(filename, options);
        if (typeof options === "string") options = { encoding: options };
        Writable.call(this, options);
        this.path = normalizeFsPath(filename);
        this.fd = options.fd ?? nextFileDescriptor++;
        this.flags = options.flags || "w";
        this.mode = options.mode ?? 0o666;
        this.start = Math.max(0, Number(options.start) || 0);
        this.pos = this.start;
        this.bytesWritten = 0;
        this.pending = false;
        this.closed = false;
        this.autoClose = options.autoClose !== false;
        this.emitClose = options.emitClose !== false;
        this._streamError = null;
        try {
            if (!this.flags.startsWith("a")) writeFileSync(this.path, Buffer.alloc(0));
        } catch (error) {
            this._streamError = error;
        }
        setImmediate(() => {
            if (this._streamError) this.emit("error", this._streamError);
            else { this.emit("open", this.fd); this.emit("ready"); }
        });
    }
    util.inherits(WriteStream, Writable);
    WriteStream.prototype._write = function (chunk, encoding, callback) {
        if (this._streamError) { callback(this._streamError); return; }
        try {
            const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, encoding);
            appendFileSync(this.path, buffer);
            this.pos += buffer.length;
            this.bytesWritten += buffer.length;
            callback();
        } catch (error) { callback(error); }
    };
    WriteStream.prototype._final = function (callback) {
        callback(this._streamError);
        if (this.autoClose) setImmediate(() => this.close());
    };
    WriteStream.prototype.close = function (callback) {
        if (typeof callback === "function") this.once("close", callback);
        if (this.closed) return this;
        this.closed = true;
        this.fd = null;
        if (this.emitClose) this.emit("close");
        return this;
    };
    WriteStream.prototype.destroy = function (error) {
        if (this.destroyed) return this;
        this.destroyed = true;
        if (error) this.emit("error", error);
        return this.close();
    };
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
        ReadStream,
        WriteStream,
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
        createReadStream(filename, options) { return new ReadStream(filename, options); },
        createWriteStream(filename, options) { return new WriteStream(filename, options); }
    };

    const dnsModule = (() => {
        let defaultResultOrder = "verbatim";
        function lookup(hostname, options, callback) {
            if (typeof options === "function") { callback = options; options = {}; }
            if (typeof options === "number") options = { family: options };
            options ||= {};
            if (typeof callback !== "function") throw new TypeError("callback must be a function");
            const family = options.family === "IPv4" ? 4 : options.family === "IPv6" ? 6 : Number(options.family) || 0;
            process.nextTick(() => {
                try {
                    let addresses = __canaryoDnsLookup(String(hostname), family);
                    if (options.order === "ipv4first" || (!options.order && defaultResultOrder === "ipv4first")) {
                        addresses = addresses.slice().sort((left, right) => left.family - right.family);
                    } else if (options.order === "ipv6first" || (!options.order && defaultResultOrder === "ipv6first")) {
                        addresses = addresses.slice().sort((left, right) => right.family - left.family);
                    }
                    if (options.all) callback(null, addresses);
                    else callback(null, addresses[0].address, addresses[0].family);
                } catch (error) {
                    error.code ||= "ENOTFOUND";
                    error.hostname ||= String(hostname);
                    callback(error);
                }
            });
        }
        function resolve(hostname, recordType, callback) {
            if (typeof recordType === "function") { callback = recordType; recordType = "A"; }
            const family = String(recordType || "A").toUpperCase() === "AAAA" ? 6 : 4;
            lookup(hostname, { family, all: true }, (error, addresses) => {
                callback(error, error ? undefined : addresses.map(item => item.address));
            });
        }
        const promises = {
            lookup(hostname, options) {
                return new Promise((resolvePromise, reject) => lookup(hostname, options || {}, (error, address, family) => {
                    if (error) reject(error);
                    else resolvePromise(options && options.all ? address : { address, family });
                }));
            },
            resolve(hostname, recordType) {
                return new Promise((resolvePromise, reject) => resolve(hostname, recordType || "A", (error, addresses) => error ? reject(error) : resolvePromise(addresses)));
            }
        };
        return {
            lookup,
            resolve,
            resolve4: (hostname, callback) => resolve(hostname, "A", callback),
            resolve6: (hostname, callback) => resolve(hostname, "AAAA", callback),
            promises,
            getDefaultResultOrder: () => defaultResultOrder,
            setDefaultResultOrder(value) {
                if (!["verbatim", "ipv4first", "ipv6first"].includes(value)) throw new TypeError("invalid DNS result order");
                defaultResultOrder = value;
            }
        };
    })();

    const workerEnvironment = new Map();
    function MessagePort() {
        EventEmitter.call(this);
        this._peer = null;
        this._messageQueue = [];
        this._closed = false;
    }
    util.inherits(MessagePort, EventEmitter);
    MessagePort.prototype.postMessage = function (value) {
        if (this._closed || !this._peer || this._peer._closed) return;
        const peer = this._peer;
        peer._messageQueue.push(value);
        setImmediate(() => {
            const index = peer._messageQueue.indexOf(value);
            if (index !== -1) peer._messageQueue.splice(index, 1);
            if (!peer._closed) peer.emit("message", value);
        });
    };
    MessagePort.prototype.start = function () {};
    MessagePort.prototype.close = function () {
        if (this._closed) return;
        this._closed = true;
        this.emit("close");
    };
    MessagePort.prototype.ref = function () { return this; };
    MessagePort.prototype.unref = function () { return this; };
    MessagePort.prototype.hasRef = function () { return false; };
    function MessageChannel() {
        this.port1 = new MessagePort();
        this.port2 = new MessagePort();
        this.port1._peer = this.port2;
        this.port2._peer = this.port1;
    }
    function Worker() {
        const error = new Error("Canaryo does not support isolated worker threads yet");
        error.code = "ERR_WORKER_UNSUPPORTED_OPERATION";
        throw error;
    }
    util.inherits(Worker, EventEmitter);
    const workerThreads = {
        isMainThread: true,
        threadId: 0,
        threadName: "",
        workerData: null,
        parentPort: null,
        resourceLimits: {},
        SHARE_ENV: Symbol.for("nodejs.worker_threads.SHARE_ENV"),
        Worker,
        MessageChannel,
        MessagePort,
        BroadcastChannel: function BroadcastChannel() {
            const error = new Error("Canaryo does not support BroadcastChannel yet");
            error.code = "ERR_WORKER_UNSUPPORTED_OPERATION";
            throw error;
        },
        getEnvironmentData(key) { return workerEnvironment.get(key); },
        setEnvironmentData(key, value) { workerEnvironment.set(key, value); },
        receiveMessageOnPort(port) {
            if (!(port instanceof MessagePort)) throw new TypeError("port must be a MessagePort");
            return port._messageQueue.length ? { message: port._messageQueue.shift() } : undefined;
        },
        markAsUntransferable() {},
        markAsUncloneable() {},
        isMarkedAsUntransferable: () => false,
        moveMessagePortToContext(port) { return port; },
        postMessageToThread() {
            return Promise.reject(Object.assign(new Error("Canaryo does not support isolated worker threads yet"), {
                code: "ERR_WORKER_UNSUPPORTED_OPERATION"
            }));
        }
    };

    const netSockets = new Map();
    function normalizeNetConnect(argumentsList) {
        const values = Array.from(argumentsList);
        const callback = typeof values[values.length - 1] === "function" ? values.pop() : null;
        let options;
        if (values[0] && typeof values[0] === "object") options = Object.assign({}, values[0]);
        else options = { port: Number(values[0]), host: typeof values[1] === "string" ? values[1] : "localhost" };
        if (!options.port) throw new TypeError("TCP port is required");
        options.host ||= "localhost";
        return { options, callback };
    }
    function NetSocket(options = {}) {
        Duplex.call(this, options);
        this.allowHalfOpen = Boolean(options.allowHalfOpen);
        this.connecting = false;
        this.pending = false;
        this.bytesRead = 0;
        this.bytesWritten = 0;
        this.remoteAddress = undefined;
        this.remoteFamily = undefined;
        this.remotePort = undefined;
        this.localAddress = undefined;
        this.localPort = undefined;
        this.readyState = "closed";
        this._timeout = 0;
    }
    util.inherits(NetSocket, Duplex);
    NetSocket.prototype.connect = function (...args) {
        const { options, callback } = normalizeNetConnect(args);
        if (callback) this.once("connect", callback);
        this.connecting = true;
        this.pending = true;
        this.destroyed = false;
        this.readyState = "opening";
        this._canaryoNetId = __canaryoNetConnect(String(options.host), Number(options.port));
        netSockets.set(this._canaryoNetId, this);
        return this;
    };
    NetSocket.prototype._resetTimeout = function () {
        if (this._timeoutHandle) clearTimeout(this._timeoutHandle);
        if (this._timeout > 0 && !this.destroyed) {
            this._timeoutHandle = setTimeout(() => this.emit("timeout"), this._timeout);
        }
    };
    NetSocket.prototype._write = function (chunk, encoding, callback) {
        try {
            const body = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, encoding);
            if (this._canaryoServerSocket) this.__canaryoServerOutgoing.push(body.toString("base64"));
            else __canaryoNetWrite(this._canaryoNetId, body.toString("base64"));
            this.bytesWritten += body.length;
            this._resetTimeout();
            callback();
        } catch (error) { callback(error); }
    };
    NetSocket.prototype._final = function (callback) {
        try {
            if (this._canaryoServerSocket) this.__canaryoServerEnd = true;
            else __canaryoNetEnd(this._canaryoNetId);
            callback();
        }
        catch (error) { callback(error); }
    };
    NetSocket.prototype.destroy = function (error) {
        if (this.destroyed) return this;
        this.readyState = "closed";
        if (this._timeoutHandle) clearTimeout(this._timeoutHandle);
        if (this._canaryoServerSocket) this.__canaryoServerDestroy = true;
        else if (this._canaryoNetId !== undefined) __canaryoNetDestroy(this._canaryoNetId);
        if (error) process.nextTick(() => this.emit("error", error));
        return this;
    };
    NetSocket.prototype.setTimeout = function (timeout, callback) {
        this._timeout = Number(timeout) || 0;
        if (typeof callback === "function") this.once("timeout", callback);
        this._resetTimeout();
        return this;
    };
    NetSocket.prototype.setNoDelay = function (value = true) { this.noDelay = Boolean(value); return this; };
    NetSocket.prototype.setKeepAlive = function (value = false, delay = 0) {
        this.keepAlive = Boolean(value);
        this.keepAliveInitialDelay = Number(delay) || 0;
        return this;
    };
    NetSocket.prototype.address = function () {
        return this.localAddress
            ? { address: this.localAddress, family: this.localFamily, port: this.localPort }
            : {};
    };
    NetSocket.prototype.ref = function () { return this; };
    NetSocket.prototype.unref = function () { return this; };
    NetSocket.prototype.__canaryoNetReceiveBytes = function (bytes) {
        const body = Buffer.from(bytes);
        this.bytesRead += body.length;
        this._resetTimeout();
        this.push(body);
    };
    NetSocket.prototype.__canaryoNetReceiveEnd = function () {
        this.push(null);
        if (!this.allowHalfOpen && !this.writableEnded) this.end();
    };
    NetSocket.prototype.__canaryoNetError = function (message, syscall) {
        this._hadError = true;
        this.emit("error", Object.assign(new Error(message), syscall ? { syscall } : {}));
    };
    NetSocket.prototype.__canaryoNetClose = function () {
        if (this._closeEmitted) return;
        this._closeEmitted = true;
        this.destroyed = true;
        this.connecting = false;
        this.pending = false;
        this.readyState = "closed";
        if (this._timeoutHandle) clearTimeout(this._timeoutHandle);
        this.emit("close", Boolean(this._hadError));
    };
    function connect(...args) { return new NetSocket().connect(...args); }
    globalThis.__canaryoPollNetSockets = function () {
        if (netSockets.size === 0) return 0;
        for (let count = 0; count < 64; count++) {
            const raw = __canaryoNetPoll();
            if (raw === undefined || raw === null) break;
            const event = JSON.parse(raw);
            const socket = netSockets.get(event.id);
            if (!socket) continue;
            if (event.type === "connected") {
                socket.connecting = false;
                socket.pending = false;
                socket.readyState = "open";
                socket.localAddress = event.localAddress;
                socket.localFamily = event.family;
                socket.localPort = event.localPort;
                socket.remoteAddress = event.remoteAddress;
                socket.remoteFamily = event.family;
                socket.remotePort = event.remotePort;
                socket._resetTimeout();
                socket.emit("connect");
                socket.emit("ready");
            } else if (event.type === "data") {
                socket.__canaryoNetReceiveBytes(Buffer.from(event.body, "base64"));
            } else if (event.type === "end") {
                socket.__canaryoNetReceiveEnd();
            } else if (event.type === "error") {
                socket.__canaryoNetError(event.message, "connect");
            } else if (event.type === "close") {
                netSockets.delete(event.id);
                socket.__canaryoNetClose();
            }
        }
        return netSockets.size;
    };
    function NetServer(options = {}, listener) {
        EventEmitter.call(this);
        if (typeof options === "function") { listener = options; options = {}; }
        this.allowHalfOpen = Boolean(options.allowHalfOpen);
        this.pauseOnConnect = Boolean(options.pauseOnConnect);
        this.listening = false;
        this.maxConnections = undefined;
        this._connections = new Set();
        this._closeCallbacks = [];
        if (typeof listener === "function") this.on("connection", listener);
    }
    util.inherits(NetServer, EventEmitter);
    NetServer.prototype.listen = function (port, host, callback) {
        const options = port && typeof port === "object" ? port : null;
        const listenPort = Number(options ? options.port : port);
        const listenHost = String(options && options.host || (typeof host === "string" ? host : "127.0.0.1"));
        const onListen = typeof host === "function" ? host : typeof callback === "function" ? callback : null;
        this._address = { address: listenHost, family: listenHost.includes(":") ? "IPv6" : "IPv4", port: listenPort };
        this.__canaryoCloseRequested = false;
        const server = this;
        globalThis.__canaryoActiveServer = server;
        globalThis.__canaryoPendingServerStart = () => {
            __canaryoNetListen(
                listenPort,
                listenHost,
                socket => {
                    server._connections.add(socket);
                    socket.once("close", () => server._connections.delete(socket));
                    if (server.pauseOnConnect) socket.pause();
                    server.emit("connection", socket);
                },
                (boundAddress, boundFamily, boundPort) => {
                    server._address = { address: boundAddress, family: boundFamily, port: boundPort };
                    server.listening = true;
                    server.emit("listening");
                    if (onListen) onListen();
                },
                (remoteAddress, remoteFamily, remotePort, localAddress, localPort) => {
                    const socket = new NetSocket({ allowHalfOpen: server.allowHalfOpen });
                    socket._canaryoServerSocket = true;
                    socket.__canaryoServerOutgoing = [];
                    socket.__canaryoServerEnd = false;
                    socket.__canaryoServerEnding = false;
                    socket.__canaryoServerDestroy = false;
                    socket.connecting = false;
                    socket.pending = false;
                    socket.readyState = "open";
                    socket.remoteAddress = remoteAddress;
                    socket.remoteFamily = remoteFamily;
                    socket.remotePort = remotePort;
                    socket.localAddress = localAddress;
                    socket.localFamily = localAddress.includes(":") ? "IPv6" : "IPv4";
                    socket.localPort = localPort;
                    if (server.pauseOnConnect) socket.pause();
                    return socket;
                }
            );
            server.listening = false;
            const callbacks = server._closeCallbacks.splice(0);
            for (const closeCallback of callbacks) closeCallback();
            server.emit("close");
        };
        return this;
    };
    NetServer.prototype.address = function () { return this.listening ? this._address : null; };
    NetServer.prototype.getConnections = function (callback) {
        process.nextTick(() => callback(null, this._connections.size));
    };
    NetServer.prototype.close = function (callback) {
        if (typeof callback === "function") this._closeCallbacks.push(callback);
        this.__canaryoCloseRequested = true;
        return this;
    };
    NetServer.prototype.closeAllConnections = function () {
        for (const socket of this._connections) socket.destroy();
    };
    NetServer.prototype.ref = function () { return this; };
    NetServer.prototype.unref = function () { return this; };
    function createServer(options, listener) { return new NetServer(options, listener); }
    const netModule = {
        Server: NetServer,
        Socket: NetSocket,
        Stream: NetSocket,
        connect,
        createConnection: connect,
        isIP: value => __canaryoIsIp(String(value)),
        isIPv4: value => __canaryoIsIp(String(value)) === 4,
        isIPv6: value => __canaryoIsIp(String(value)) === 6,
        createServer
    };

    const builtinModules = [
        "assert", "async_hooks", "buffer", "crypto", "diagnostics_channel", "dns",
        "dns/promises", "events", "fs", "fs/promises", "http", "https", "module", "net", "os",
        "path", "perf_hooks", "querystring", "stream", "string_decoder", "timers", "tty",
        "url", "util", "worker_threads", "zlib"
    ];
    function isBuiltin(name) {
        return builtinModules.includes(String(name).replace(/^node:/, ""));
    }
    function NodeModule(id = "", parent = null) {
        this.id = id;
        this.path = path.dirname(id || ".");
        this.exports = {};
        this.filename = null;
        this.loaded = false;
        this.parent = parent;
        this.children = [];
        this.paths = [];
    }
    NodeModule.builtinModules = builtinModules;
    NodeModule.isBuiltin = isBuiltin;
    NodeModule.createRequire = filename => {
        const value = filename instanceof URL ? fileURLToPath(filename) : String(filename);
        return __canaryoCreateRequire(value);
    };
    NodeModule.createRequireFromPath = NodeModule.createRequire;
    NodeModule.syncBuiltinESMExports = () => {};
    NodeModule._cache = globalThis.__canaryoModuleCache || Object.create(null);
    const moduleModule = Object.assign(NodeModule, {
        Module: NodeModule,
        builtinModules,
        isBuiltin,
        createRequire: NodeModule.createRequire,
        syncBuiltinESMExports: NodeModule.syncBuiltinESMExports,
        _cache: NodeModule._cache
    });

    function cryptoInput(value, encoding) {
        return Buffer.isBuffer(value) ? Buffer.from(value) : Buffer.from(value, encoding);
    }
    function cryptoOutput(base64, encoding) {
        const output = Buffer.from(base64, "base64");
        return encoding === undefined ? output : output.toString(encoding);
    }
    function createDigest(algorithm, key) {
        const chunks = [];
        let finalized = false;
        return {
            update(value, encoding) {
                if (finalized) throw new Error("Digest already called");
                chunks.push(cryptoInput(value, encoding));
                return this;
            },
            digest(encoding) {
                if (finalized) throw new Error("Digest already called");
                finalized = true;
                const contents = Buffer.concat(chunks).toString("base64");
                const result = key === undefined
                    ? __canaryoHash(algorithm, contents)
                    : __canaryoHmac(algorithm, key.toString("base64"), contents);
                return cryptoOutput(result, encoding);
            }
        };
    }
    function randomBytes(size, callback) {
        const length = Number(size);
        if (!Number.isInteger(length) || length < 0) throw new RangeError("size must be a non-negative integer");
        const output = Buffer.from(__canaryoRandomBytes(length), "base64");
        if (typeof callback === "function") {
            process.nextTick(callback, null, output);
            return undefined;
        }
        return output;
    }
    function randomFillSync(buffer, offset = 0, size) {
        if (!ArrayBuffer.isView(buffer)) throw new TypeError("buffer must be an ArrayBuffer view");
        const bytes = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
        const start = Number(offset);
        const length = size === undefined ? bytes.length - start : Number(size);
        if (!Number.isInteger(start) || !Number.isInteger(length) || start < 0 || length < 0 || start + length > bytes.length) {
            throw new RangeError("offset and size are outside the buffer");
        }
        bytes.set(Buffer.from(__canaryoRandomBytes(length), "base64"), start);
        return buffer;
    }
    function randomFill(buffer, offset, size, callback) {
        if (typeof offset === "function") { callback = offset; offset = 0; size = undefined; }
        else if (typeof size === "function") { callback = size; size = undefined; }
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        try {
            randomFillSync(buffer, offset || 0, size);
            process.nextTick(callback, null, buffer);
        } catch (error) {
            process.nextTick(callback, error);
        }
    }
    function randomUUID() {
        const bytes = randomBytes(16);
        bytes[6] = bytes[6] & 0x0f | 0x40;
        bytes[8] = bytes[8] & 0x3f | 0x80;
        const hex = bytes.toString("hex");
        return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    }
    const webcrypto = {
        getRandomValues(view) {
            if (!ArrayBuffer.isView(view)) throw new TypeError("value must be an integer ArrayBuffer view");
            if (view.byteLength > 65536) throw new RangeError("requested too many random bytes");
            randomFillSync(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
            return view;
        },
        randomUUID,
        subtle: Object.freeze({})
    };
    const cryptoModule = {
        createHash(algorithm) { return createDigest(algorithm); },
        createHmac(algorithm, key, encoding) { return createDigest(algorithm, cryptoInput(key, encoding)); },
        randomBytes,
        randomFill,
        randomFillSync,
        randomUUID,
        timingSafeEqual(left, right) {
            return __canaryoTimingSafeEqual(
                cryptoInput(left).toString("base64"),
                cryptoInput(right).toString("base64")
            );
        },
        getHashes() { return ["sha1", "sha256", "sha384", "sha512"]; },
        webcrypto,
        constants: {}
    };
    if (typeof globalThis.crypto === "undefined") globalThis.crypto = webcrypto;

    function zlibSync(operation, input) {
        const contents = Buffer.from(input).toString("base64");
        return Buffer.from(__canaryoZlibTransform(operation, contents), "base64");
    }
    function zlibAsync(operation, input, options, callback) {
        if (typeof options === "function") { callback = options; options = undefined; }
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        process.nextTick(() => {
            try { callback(null, zlibSync(operation, input)); }
            catch (error) { callback(error); }
        });
    }
    function zlibConstructor(operation) {
        function Codec(_options = {}) {
            const chunks = [];
            Transform.call(this, {
                transform(chunk, encoding, callback) {
                    chunks.push(Buffer.isBuffer(chunk) ? Buffer.from(chunk) : Buffer.from(chunk, encoding));
                    callback();
                },
                final(callback) {
                    try { this.push(zlibSync(operation, Buffer.concat(chunks))); callback(); }
                    catch (error) { callback(error); }
                }
            });
        }
        util.inherits(Codec, Transform);
        return Codec;
    }
    const Gzip = zlibConstructor("gzip");
    const Gunzip = zlibConstructor("gunzip");
    const Deflate = zlibConstructor("deflate");
    const Inflate = zlibConstructor("inflate");
    const DeflateRaw = zlibConstructor("deflateRaw");
    const InflateRaw = zlibConstructor("inflateRaw");
    const Unzip = zlibConstructor("unzip");
    const BrotliCompress = zlibConstructor("brotliCompress");
    const BrotliDecompress = zlibConstructor("brotliDecompress");
    const zlibModule = {
        gzipSync: input => zlibSync("gzip", input),
        gunzipSync: input => zlibSync("gunzip", input),
        deflateSync: input => zlibSync("deflate", input),
        inflateSync: input => zlibSync("inflate", input),
        deflateRawSync: input => zlibSync("deflateRaw", input),
        inflateRawSync: input => zlibSync("inflateRaw", input),
        unzipSync: input => zlibSync("unzip", input),
        brotliCompressSync: input => zlibSync("brotliCompress", input),
        brotliDecompressSync: input => zlibSync("brotliDecompress", input),
        gzip(input, options, callback) { zlibAsync("gzip", input, options, callback); },
        gunzip(input, options, callback) { zlibAsync("gunzip", input, options, callback); },
        deflate(input, options, callback) { zlibAsync("deflate", input, options, callback); },
        inflate(input, options, callback) { zlibAsync("inflate", input, options, callback); },
        deflateRaw(input, options, callback) { zlibAsync("deflateRaw", input, options, callback); },
        inflateRaw(input, options, callback) { zlibAsync("inflateRaw", input, options, callback); },
        unzip(input, options, callback) { zlibAsync("unzip", input, options, callback); },
        brotliCompress(input, options, callback) { zlibAsync("brotliCompress", input, options, callback); },
        brotliDecompress(input, options, callback) { zlibAsync("brotliDecompress", input, options, callback); },
        createGzip: options => new Gzip(options),
        createGunzip: options => new Gunzip(options),
        createDeflate: options => new Deflate(options),
        createInflate: options => new Inflate(options),
        createDeflateRaw: options => new DeflateRaw(options),
        createInflateRaw: options => new InflateRaw(options),
        createUnzip: options => new Unzip(options),
        createBrotliCompress: options => new BrotliCompress(options),
        createBrotliDecompress: options => new BrotliDecompress(options),
        Gzip,
        Gunzip,
        Deflate,
        Inflate,
        DeflateRaw,
        InflateRaw,
        Unzip,
        BrotliCompress,
        BrotliDecompress,
        constants: {
            Z_NO_FLUSH: 0,
            Z_SYNC_FLUSH: 2,
            Z_FULL_FLUSH: 3,
            Z_FINISH: 4,
            Z_DEFAULT_COMPRESSION: -1,
            Z_BEST_SPEED: 1,
            Z_BEST_COMPRESSION: 9,
            Z_DEFAULT_STRATEGY: 0,
            BROTLI_OPERATION_PROCESS: 0,
            BROTLI_OPERATION_FLUSH: 1,
            BROTLI_OPERATION_FINISH: 2
        }
    };

    globalThis.__canaryoBuiltins = Object.freeze({
        assert,
        async_hooks: { AsyncLocalStorage, AsyncResource, executionAsyncId: () => 0, triggerAsyncId: () => 0 },
        buffer: {
            Buffer,
            SlowBuffer: Buffer,
            INSPECT_MAX_BYTES: 50,
            kMaxLength: 0x7fffffff,
            constants: { MAX_LENGTH: 0x7fffffff, MAX_STRING_LENGTH: 0x1fffffe8 }
        },
        crypto: cryptoModule,
        depd,
        diagnostics_channel: diagnosticsChannel,
        dns: dnsModule,
        "dns/promises": dnsModule.promises,
        events: Object.assign(EventEmitter, { EventEmitter }),
        fs: fsModule,
        "fs/promises": fsPromises,
        https: globalThis.__canaryoHttpsModule,
        module: moduleModule,
        net: netModule,
        os: {
            EOL: process.platform === "win32" ? "\r\n" : "\n",
            constants: { signals: {}, errno: {}, priority: {}, dlopen: {} },
            arch: () => __canaryoOsInfo.arch,
            platform: () => __canaryoOsInfo.platform,
            type: () => __canaryoOsInfo.type,
            endianness: () => __canaryoOsInfo.endianness,
            homedir: () => __canaryoOsInfo.homeDir,
            tmpdir: () => __canaryoOsInfo.tempDir,
            hostname: () => __canaryoOsInfo.hostname,
            availableParallelism: () => __canaryoOsInfo.parallelism,
            cpus: () => Array.from({ length: __canaryoOsInfo.parallelism }, () => ({
                model: "Canaryo virtual CPU", speed: 0,
                times: { user: 0, nice: 0, sys: 0, idle: 0, irq: 0 }
            })),
            freemem: () => 0,
            totalmem: () => 0,
            uptime: () => process.uptime(),
            release: () => "",
            version: () => "",
            machine: () => __canaryoOsInfo.arch,
            userInfo: () => ({
                uid: -1, gid: -1,
                username: process.env.USERNAME || process.env.USER || "",
                homedir: __canaryoOsInfo.homeDir,
                shell: null
            }),
            networkInterfaces: () => ({})
        },
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
        worker_threads: workerThreads,
        zlib: zlibModule
    });
})();
