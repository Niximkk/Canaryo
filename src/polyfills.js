(() => {
    const disposeSymbol = Symbol.dispose || Symbol.for("nodejs.dispose");

    function DOMException(message = "", name = "Error") {
        const error = new Error(String(message));
        Object.setPrototypeOf(error, DOMException.prototype);
        error.name = String(name);
        error.code = error.name === "AbortError" ? 20 : 0;
        return error;
    }
    DOMException.prototype = Object.create(Error.prototype, {
        constructor: { value: DOMException, writable: true, configurable: true }
    });

    function Event(type, options = {}) {
        if (arguments.length === 0) throw new TypeError("event type is required");
        this.type = String(type);
        this.bubbles = Boolean(options.bubbles);
        this.cancelable = Boolean(options.cancelable);
        this.composed = Boolean(options.composed);
        this.defaultPrevented = false;
        this.target = null;
        this.currentTarget = null;
        this.eventPhase = 0;
        this.timeStamp = Date.now();
        this._stopped = false;
        this._immediateStopped = false;
    }
    Event.NONE = 0;
    Event.CAPTURING_PHASE = 1;
    Event.AT_TARGET = 2;
    Event.BUBBLING_PHASE = 3;
    Event.prototype.preventDefault = function () {
        if (this.cancelable) this.defaultPrevented = true;
    };
    Event.prototype.stopPropagation = function () { this._stopped = true; };
    Event.prototype.stopImmediatePropagation = function () {
        this._stopped = true;
        this._immediateStopped = true;
    };
    Event.prototype.composedPath = function () { return this.target ? [this.target] : []; };

    function MessageEvent(type, options = {}) {
        Event.call(this, type, options);
        this.data = options.data ?? null;
        this.origin = String(options.origin || "");
        this.lastEventId = String(options.lastEventId || "");
        this.source = options.source ?? null;
        this.ports = Array.isArray(options.ports) ? options.ports : [];
    }
    MessageEvent.prototype = Object.create(Event.prototype, {
        constructor: { value: MessageEvent, writable: true, configurable: true }
    });

    function EventTarget() { this._eventTargetListeners = new Map(); }
    EventTarget.prototype.addEventListener = function (type, callback, options = {}) {
        if (callback === null || callback === undefined) return;
        if (typeof callback !== "function" && typeof callback.handleEvent !== "function") {
            throw new TypeError("callback must be a function or EventListener object");
        }
        const name = String(type);
        const capture = typeof options === "boolean" ? options : Boolean(options.capture);
        const once = typeof options === "object" && Boolean(options.once);
        const signal = typeof options === "object" ? options.signal : undefined;
        if (signal?.aborted) return;
        const listeners = this._eventTargetListeners.get(name) || [];
        if (listeners.some(listener => listener.callback === callback && listener.capture === capture)) return;
        const entry = { callback, capture, once, signal, abortDisposable: undefined };
        listeners.push(entry);
        this._eventTargetListeners.set(name, listeners);
        if (signal) {
            entry.abortDisposable = addAbortListener(signal, () => {
                this.removeEventListener(name, callback, capture);
            });
        }
    };
    EventTarget.prototype.removeEventListener = function (type, callback, options = {}) {
        const name = String(type);
        const capture = typeof options === "boolean" ? options : Boolean(options.capture);
        const listeners = this._eventTargetListeners.get(name);
        if (!listeners) return;
        const index = listeners.findIndex(listener => listener.callback === callback && listener.capture === capture);
        if (index < 0) return;
        const [entry] = listeners.splice(index, 1);
        entry.abortDisposable?.[disposeSymbol]();
        if (listeners.length === 0) this._eventTargetListeners.delete(name);
    };
    EventTarget.prototype.dispatchEvent = function (event) {
        if (!(event instanceof Event)) throw new TypeError("event must be an Event");
        event.target = this;
        event.currentTarget = this;
        event.eventPhase = Event.AT_TARGET;
        const listeners = [...(this._eventTargetListeners.get(event.type) || [])];
        for (const entry of listeners) {
            if (event._immediateStopped) break;
            if (entry.once) this.removeEventListener(event.type, entry.callback, entry.capture);
            if (typeof entry.callback === "function") entry.callback.call(this, event);
            else entry.callback.handleEvent(event);
        }
        if (!event._immediateStopped) {
            const handler = this[`on${event.type}`];
            if (typeof handler === "function") handler.call(this, event);
        }
        event.currentTarget = null;
        event.eventPhase = Event.NONE;
        return !event.defaultPrevented;
    };
    EventTarget.prototype.setMaxListeners = function (value) {
        this._maxListeners = Number(value);
        return this;
    };
    EventTarget.prototype.getMaxListeners = function () { return this._maxListeners ?? 10; };
    EventTarget.prototype.listeners = function (type) {
        return (this._eventTargetListeners.get(String(type)) || []).map(entry => entry.callback);
    };
    EventTarget.prototype.listenerCount = function (type) { return this.listeners(type).length; };

    const abortSignalToken = {};
    function AbortSignal(token) {
        if (token !== abortSignalToken) throw new TypeError("Illegal constructor");
        EventTarget.call(this);
        this.aborted = false;
        this.reason = undefined;
        this.onabort = null;
        this._safeAbortListeners = new Set();
    }
    AbortSignal.prototype = Object.create(EventTarget.prototype, {
        constructor: { value: AbortSignal, writable: true, configurable: true }
    });
    AbortSignal.prototype.throwIfAborted = function () {
        if (this.aborted) throw this.reason;
    };
    AbortSignal.prototype._abort = function (reason) {
        if (this.aborted) return;
        this.aborted = true;
        this.reason = reason === undefined
            ? new DOMException("This operation was aborted", "AbortError")
            : reason;
        try { this.dispatchEvent(new Event("abort")); }
        finally {
            for (const listener of [...this._safeAbortListeners]) listener();
            this._safeAbortListeners.clear();
        }
    };
    AbortSignal.abort = function (reason) {
        const signal = new AbortSignal(abortSignalToken);
        signal._abort(reason);
        return signal;
    };
    AbortSignal.timeout = function (delay) {
        const signal = new AbortSignal(abortSignalToken);
        const handle = setTimeout(() => signal._abort(
            new DOMException("The operation was aborted due to timeout", "TimeoutError")
        ), Math.max(0, Number(delay)));
        handle.unref();
        return signal;
    };
    AbortSignal.any = function (signals) {
        const signal = new AbortSignal(abortSignalToken);
        const disposables = [];
        const abort = source => {
            if (signal.aborted) return;
            signal._abort(source.reason);
            for (const disposable of disposables) disposable[disposeSymbol]();
        };
        for (const source of signals) {
            if (!source || typeof source.addEventListener !== "function") {
                throw new TypeError("signals must contain AbortSignal instances");
            }
            if (source.aborted) {
                abort(source);
                break;
            }
            disposables.push(addAbortListener(source, () => abort(source)));
        }
        return signal;
    };

    function AbortController() {
        this.signal = new AbortSignal(abortSignalToken);
    }
    AbortController.prototype.abort = function (reason) { this.signal._abort(reason); };

    function abortApiError(reason) {
        const error = new Error("The operation was aborted");
        error.name = "AbortError";
        error.code = "ABORT_ERR";
        error.cause = reason;
        return error;
    }

    function addAbortListener(signal, listener) {
        if (!signal || typeof signal.addEventListener !== "function") {
            throw new TypeError("signal must be an AbortSignal");
        }
        if (typeof listener !== "function") throw new TypeError("listener must be a function");
        let active = true;
        const wrapped = () => {
            if (!active) return;
            active = false;
            listener();
        };
        if (signal.aborted) queueMicrotask(wrapped);
        else if (signal._safeAbortListeners) signal._safeAbortListeners.add(wrapped);
        else signal.addEventListener("abort", wrapped, { once: true });
        const dispose = () => {
            if (!active) return;
            active = false;
            if (signal._safeAbortListeners) signal._safeAbortListeners.delete(wrapped);
            else signal.removeEventListener("abort", wrapped);
        };
        return { [disposeSymbol]: dispose };
    }

    const captureRejectionSymbol = Symbol.for("nodejs.rejection");
    const errorMonitor = Symbol("events.errorMonitor");
    function EventEmitter(options) {
        this._events = Object.create(null);
        this._captureRejections = options && Object.prototype.hasOwnProperty.call(options, "captureRejections")
            ? Boolean(options.captureRejections)
            : EventEmitter.captureRejections;
    }
    EventEmitter.captureRejections = false;
    EventEmitter.captureRejectionSymbol = captureRejectionSymbol;
    EventEmitter.errorMonitor = errorMonitor;
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
            return listener.apply(emitter, args);
        }
        onceListener.listener = listener;
        return this.on(name, onceListener);
    };
    EventEmitter.prototype.prependOnceListener = function (name, listener) {
        const emitter = this;
        function onceListener(...args) {
            emitter.removeListener(name, onceListener);
            return listener.apply(emitter, args);
        }
        onceListener.listener = listener;
        return this.prependListener(name, onceListener);
    };
    EventEmitter.prototype.emit = function (name, ...args) {
        const listeners = this._events && this._events[name];
        if (name === "error") {
            const monitors = this._events && this._events[errorMonitor];
            if (monitors) {
                for (const monitor of [...monitors]) monitor.apply(this, args);
            }
        }
        if (!listeners || listeners.length === 0) {
            if (name === "error") {
                throw args[0] instanceof Error ? args[0] : new Error(`Unhandled error: ${args[0]}`);
            }
            return false;
        }
        for (const listener of [...listeners]) {
            const result = listener.apply(this, args);
            if (this._captureRejections && result && typeof result.then === "function") {
                Promise.resolve(result).catch(error => {
                    const handler = this[captureRejectionSymbol];
                    if (typeof handler === "function") handler.call(this, error, name, ...args);
                    else this.emit("error", error);
                });
            }
        }
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
    EventEmitter.once = function (emitter, name, options = {}) {
        return new Promise((resolve, reject) => {
            let abortDisposable;
            const eventTarget = typeof emitter.once !== "function" &&
                typeof emitter.addEventListener === "function";
            function cleanup() {
                if (eventTarget) emitter.removeEventListener(name, onEvent);
                else {
                    emitter.removeListener(name, onEvent);
                    if (name !== "error") emitter.removeListener("error", onError);
                }
                abortDisposable?.[disposeSymbol]();
            }
            function onEvent(...args) { cleanup(); resolve(args); }
            function onError(error) { cleanup(); reject(error); }
            function onAbort() { cleanup(); reject(abortApiError(options.signal.reason)); }
            if (options.signal?.aborted) {
                onAbort();
                return;
            }
            if (eventTarget) emitter.addEventListener(name, onEvent, { once: true });
            else {
                emitter.once(name, onEvent);
                if (name !== "error") emitter.once("error", onError);
            }
            if (options.signal) abortDisposable = addAbortListener(options.signal, onAbort);
        });
    };
    EventEmitter.getEventListeners = (emitter, name) => emitter.listeners(name);
    EventEmitter.listenerCount = (emitter, name) => emitter.listenerCount(name);
    EventEmitter.setMaxListeners = (value, ...emitters) => {
        for (const emitter of emitters) emitter.setMaxListeners(value);
    };
    EventEmitter.getMaxListeners = emitter => emitter.getMaxListeners();
    EventEmitter.addAbortListener = addAbortListener;
    EventEmitter.on = function (emitter, name, options = {}) {
        const values = [];
        const waiting = [];
        let stopped = false;
        let failure;
        let abortDisposable;
        const eventTarget = typeof emitter.on !== "function" &&
            typeof emitter.addEventListener === "function";
        const add = (event, listener) => eventTarget
            ? emitter.addEventListener(event, listener)
            : emitter.on(event, listener);
        const remove = (event, listener) => eventTarget
            ? emitter.removeEventListener(event, listener)
            : emitter.removeListener(event, listener);
        const cleanup = () => {
            remove(name, onEvent);
            if (!eventTarget && name !== "error") remove("error", onError);
            for (const closeEvent of options.close || []) remove(closeEvent, onClose);
            abortDisposable?.[disposeSymbol]();
        };
        const finish = error => {
            if (stopped) return;
            stopped = true;
            failure = error;
            cleanup();
            while (waiting.length > 0) {
                const waiter = waiting.shift();
                if (error) waiter.reject(error);
                else waiter.resolve({ value: undefined, done: true });
            }
        };
        const onEvent = (...args) => {
            const waiter = waiting.shift();
            if (waiter) waiter.resolve({ value: args, done: false });
            else values.push(args);
        };
        const onError = error => finish(error);
        const onClose = () => finish();
        add(name, onEvent);
        if (!eventTarget && name !== "error") add("error", onError);
        for (const closeEvent of options.close || []) add(closeEvent, onClose);
        if (options.signal?.aborted) finish(abortApiError(options.signal.reason));
        else if (options.signal) {
            abortDisposable = addAbortListener(options.signal, () => {
                finish(abortApiError(options.signal.reason));
            });
        }
        return {
            next() {
                if (values.length > 0) return Promise.resolve({ value: values.shift(), done: false });
                if (failure) return Promise.reject(failure);
                if (stopped) return Promise.resolve({ value: undefined, done: true });
                return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
            },
            return() {
                finish();
                return Promise.resolve({ value: undefined, done: true });
            },
            throw(error) {
                finish(error);
                return Promise.reject(error);
            },
            [Symbol.asyncIterator]() { return this; }
        };
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
        if (normalized === "utf8") return __canaryoEncodeUtf8(String(value));
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
        if (normalized === "utf8") return __canaryoDecodeUtf8(bytes);
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
        constructor(...args) {
            super(0);
            const bytes = new Uint8Array(...args);
            Object.setPrototypeOf(bytes, new.target.prototype);
            return bytes;
        }
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
                if (pattern.length) {
                    for (let index = 0; index < buffer.length; index++) buffer[index] = pattern[index % pattern.length];
                }
            } else buffer.fill(fill);
            return buffer;
        }
        static allocUnsafe(size) { return new Buffer(size); }
        static allocUnsafeSlow(size) { return new Buffer(size); }
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
            if (!Array.isArray(list)) throw new TypeError("list must be an Array of Uint8Array instances");
            const length = totalLength === undefined
                ? list.reduce((sum, item) => sum + item.length, 0)
                : Number(totalLength);
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
        lastIndexOf(value, byteOffset = this.length - 1, encoding) {
            const needle = typeof value === "number" ? Buffer.from([value & 0xff]) : Buffer.from(value, encoding);
            let start = Number(byteOffset);
            if (!Number.isFinite(start)) start = this.length - 1;
            if (start < 0) start = this.length + start;
            start = Math.min(Math.trunc(start), this.length - needle.length);
            for (let index = start; index >= 0; index--) {
                let matches = true;
                for (let offset = 0; offset < needle.length; offset++) {
                    if (this[index + offset] !== needle[offset]) { matches = false; break; }
                }
                if (matches) return index;
            }
            return -1;
        }
        write(value, offset = 0, length = this.length - offset, encoding = "utf8") {
            if (typeof length === "string") { encoding = length; length = this.length - offset; }
            const source = Buffer.from(value, encoding);
            const count = Math.min(source.length, length, this.length - offset);
            this.set(source.subarray(0, count), offset);
            return count;
        }
        readUInt8(offset = 0) { return this[offset]; }
        readInt8(offset = 0) { const value = this.readUInt8(offset); return value & 0x80 ? value - 0x100 : value; }
        readUInt16LE(offset = 0) { return this[offset] | this[offset + 1] << 8; }
        readUInt16BE(offset = 0) { return this[offset] << 8 | this[offset + 1]; }
        readInt16LE(offset = 0) { const value = this.readUInt16LE(offset); return value & 0x8000 ? value - 0x10000 : value; }
        readInt16BE(offset = 0) { const value = this.readUInt16BE(offset); return value & 0x8000 ? value - 0x10000 : value; }
        readUInt32LE(offset = 0) { return (this[offset] | this[offset + 1] << 8 | this[offset + 2] << 16 | this[offset + 3] << 24) >>> 0; }
        readUInt32BE(offset = 0) { return (this[offset] << 24 | this[offset + 1] << 16 | this[offset + 2] << 8 | this[offset + 3]) >>> 0; }
        readInt32LE(offset = 0) { return this.readUInt32LE(offset) | 0; }
        readInt32BE(offset = 0) { return this.readUInt32BE(offset) | 0; }
        readUIntLE(offset, byteLength) {
            let value = 0;
            let multiplier = 1;
            for (let index = 0; index < byteLength; index++) { value += this[offset + index] * multiplier; multiplier *= 256; }
            return value;
        }
        readUIntBE(offset, byteLength) {
            let value = 0;
            for (let index = 0; index < byteLength; index++) value = value * 256 + this[offset + index];
            return value;
        }
        readIntLE(offset, byteLength) { const value = this.readUIntLE(offset, byteLength); const limit = 2 ** (byteLength * 8 - 1); return value >= limit ? value - 2 ** (byteLength * 8) : value; }
        readIntBE(offset, byteLength) { const value = this.readUIntBE(offset, byteLength); const limit = 2 ** (byteLength * 8 - 1); return value >= limit ? value - 2 ** (byteLength * 8) : value; }
        _dataView() { return new DataView(this.buffer, this.byteOffset, this.byteLength); }
        readFloatLE(offset = 0) { return this._dataView().getFloat32(offset, true); }
        readFloatBE(offset = 0) { return this._dataView().getFloat32(offset, false); }
        readDoubleLE(offset = 0) { return this._dataView().getFloat64(offset, true); }
        readDoubleBE(offset = 0) { return this._dataView().getFloat64(offset, false); }
        readBigUInt64LE(offset = 0) { return this._dataView().getBigUint64(offset, true); }
        readBigUInt64BE(offset = 0) { return this._dataView().getBigUint64(offset, false); }
        readBigInt64LE(offset = 0) { return this._dataView().getBigInt64(offset, true); }
        readBigInt64BE(offset = 0) { return this._dataView().getBigInt64(offset, false); }
        writeUInt8(value, offset = 0) { this[offset] = value; return offset + 1; }
        writeInt8(value, offset = 0) { return this.writeUInt8(value, offset); }
        writeUInt16LE(value, offset = 0) { this[offset] = value; this[offset + 1] = value >> 8; return offset + 2; }
        writeUInt16BE(value, offset = 0) { this[offset] = value >> 8; this[offset + 1] = value; return offset + 2; }
        writeInt16LE(value, offset = 0) { return this.writeUInt16LE(value, offset); }
        writeInt16BE(value, offset = 0) { return this.writeUInt16BE(value, offset); }
        writeUInt32LE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> index * 8; return offset + 4; }
        writeUInt32BE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> (3 - index) * 8; return offset + 4; }
        writeInt32LE(value, offset = 0) { return this.writeUInt32LE(value, offset); }
        writeInt32BE(value, offset = 0) { return this.writeUInt32BE(value, offset); }
        writeUIntLE(value, offset, byteLength) {
            let remaining = Number(value);
            for (let index = 0; index < byteLength; index++) { this[offset + index] = remaining % 256; remaining = Math.floor(remaining / 256); }
            return offset + byteLength;
        }
        writeUIntBE(value, offset, byteLength) {
            let remaining = Number(value);
            for (let index = byteLength - 1; index >= 0; index--) { this[offset + index] = remaining % 256; remaining = Math.floor(remaining / 256); }
            return offset + byteLength;
        }
        writeIntLE(value, offset, byteLength) { return this.writeUIntLE(value < 0 ? value + 2 ** (byteLength * 8) : value, offset, byteLength); }
        writeIntBE(value, offset, byteLength) { return this.writeUIntBE(value < 0 ? value + 2 ** (byteLength * 8) : value, offset, byteLength); }
        writeFloatLE(value, offset = 0) { this._dataView().setFloat32(offset, Number(value), true); return offset + 4; }
        writeFloatBE(value, offset = 0) { this._dataView().setFloat32(offset, Number(value), false); return offset + 4; }
        writeDoubleLE(value, offset = 0) { this._dataView().setFloat64(offset, Number(value), true); return offset + 8; }
        writeDoubleBE(value, offset = 0) { this._dataView().setFloat64(offset, Number(value), false); return offset + 8; }
        writeBigUInt64LE(value, offset = 0) { this._dataView().setBigUint64(offset, BigInt(value), true); return offset + 8; }
        writeBigUInt64BE(value, offset = 0) { this._dataView().setBigUint64(offset, BigInt(value), false); return offset + 8; }
        writeBigInt64LE(value, offset = 0) { this._dataView().setBigInt64(offset, BigInt(value), true); return offset + 8; }
        writeBigInt64BE(value, offset = 0) { this._dataView().setBigInt64(offset, BigInt(value), false); return offset + 8; }
        swap16() { return this._swap(2); }
        swap32() { return this._swap(4); }
        swap64() { return this._swap(8); }
        _swap(width) {
            if (this.length % width !== 0) throw new RangeError(`Buffer size must be a multiple of ${width * 8}-bits`);
            for (let offset = 0; offset < this.length; offset += width) {
                for (let left = 0, right = width - 1; left < right; left++, right--) {
                    const value = this[offset + left];
                    this[offset + left] = this[offset + right];
                    this[offset + right] = value;
                }
            }
            return this;
        }
        toJSON() { return { type: "Buffer", data: [...this] }; }
    }
    Buffer.poolSize = 8192;
    for (const method of ["from", "alloc", "allocUnsafe", "allocUnsafeSlow", "isBuffer", "isEncoding", "byteLength", "compare", "concat"]) {
        Object.defineProperty(Buffer, method, { enumerable: true });
    }

    class Blob {
        constructor(sources = [], options = {}) {
            const chunks = [];
            for (const source of sources) {
                if (source instanceof Blob) chunks.push(source._buffer);
                else if (typeof source === "string") chunks.push(Buffer.from(source));
                else if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
                    chunks.push(Buffer.from(source));
                } else chunks.push(Buffer.from(String(source)));
            }
            this._buffer = Buffer.concat(chunks);
            const type = String(options.type || "").toLowerCase();
            this.type = /^[\x20-\x7e]*$/.test(type) ? type : "";
        }
        get size() { return this._buffer.length; }
        arrayBuffer() {
            const bytes = new Uint8Array(this._buffer);
            return Promise.resolve(bytes.buffer);
        }
        bytes() { return Promise.resolve(new Uint8Array(this._buffer)); }
        text() { return Promise.resolve(this._buffer.toString()); }
        slice(start = 0, end = this.size, type = "") {
            const normalize = value => value < 0
                ? Math.max(this.size + Math.trunc(value), 0)
                : Math.min(Math.trunc(value), this.size);
            const first = normalize(Number(start) || 0);
            const last = normalize(end === undefined ? this.size : Number(end) || 0);
            return new Blob([this._buffer.subarray(first, Math.max(first, last))], { type });
        }
    }
    class File extends Blob {
        constructor(sources, name, options = {}) {
            super(sources, options);
            this.name = String(name);
            this.lastModified = options.lastModified === undefined
                ? Date.now()
                : Number(options.lastModified);
        }
    }
    function atob(value) {
        return decodeBase64(String(value)).map(byte => String.fromCharCode(byte)).join("");
    }
    function btoa(value) {
        const bytes = [];
        for (const character of String(value)) {
            const code = character.charCodeAt(0);
            if (code > 0xff) throw new DOMException("Invalid character", "InvalidCharacterError");
            bytes.push(code);
        }
        return encodeBase64(bytes);
    }

    class TextEncoder {
        encode(value) { return new Uint8Array(encodeUtf8(value)); }
    }

    class TextDecoder {
        decode(value = new Uint8Array()) { return decodeUtf8(value); }
    }

    class StringDecoder {
        constructor(encoding = "utf8") {
            this.encoding = normalizeEncoding(encoding);
            this._pending = [];
            this.lastNeed = 0;
            this.lastTotal = 0;
            this.lastChar = Buffer.alloc(4);
        }
        _split(bytes) {
            let complete = bytes.length;
            if (this.encoding === "utf8" && complete > 0) {
                let lead = complete - 1;
                while (lead >= 0 && (bytes[lead] & 0xc0) === 0x80) lead--;
                if (lead >= 0) {
                    const first = bytes[lead];
                    const expected = first >= 0xf0 && first <= 0xf4 ? 4
                        : first >= 0xe0 ? 3
                            : first >= 0xc2 ? 2
                                : 1;
                    if (complete - lead < expected) complete = lead;
                }
            } else if (this.encoding === "utf16le") {
                complete -= complete % 2;
                if (complete >= 2) {
                    const code = bytes[complete - 2] | bytes[complete - 1] << 8;
                    if (code >= 0xd800 && code <= 0xdbff) complete -= 2;
                }
            } else if (this.encoding === "base64" || this.encoding === "base64url") {
                complete -= complete % 3;
            }
            return complete;
        }
        write(value) {
            const bytes = this._pending.concat([...value]);
            const complete = this._split(bytes);
            this._pending = bytes.slice(complete);
            this.lastNeed = this._pending.length;
            this.lastTotal = this.lastNeed ? complete + this.lastNeed : 0;
            return decodeBytes(new Uint8Array(bytes.slice(0, complete)), this.encoding);
        }
        end(value) {
            let output = value === undefined ? "" : this.write(value);
            if (this._pending.length) {
                output += this.encoding === "utf8" || this.encoding === "utf16le"
                    ? "\ufffd"
                    : decodeBytes(new Uint8Array(this._pending), this.encoding);
            }
            this._pending = [];
            this.lastNeed = 0;
            this.lastTotal = 0;
            return output;
        }
        text(value, offset = 0) {
            this._pending = [];
            return this.write([...value].slice(offset));
        }
    }

    function queryUnescape(value, decode = decodeURIComponent) {
        const source = String(value).replace(/\+/g, " ");
        try { return decode(source); }
        catch {
            return source.replace(/(?:%[0-9a-fA-F]{2})+/g, sequence => {
                try { return decodeURIComponent(sequence); }
                catch { return sequence; }
            });
        }
    }
    function queryParse(value, separator = "&", equals = "=", options = {}) {
        const output = Object.create(null);
        const source = String(value);
        if (!source) return output;
        const maxKeys = options.maxKeys === undefined ? 1000 : Number(options.maxKeys);
        const parts = source.split(String(separator));
        const limit = maxKeys > 0 ? Math.min(parts.length, maxKeys) : parts.length;
        const decode = typeof options.decodeURIComponent === "function" ? options.decodeURIComponent : decodeURIComponent;
        for (let index = 0; index < limit; index++) {
            const part = parts[index];
            const equalAt = part.indexOf(String(equals));
            const key = queryUnescape(equalAt < 0 ? part : part.slice(0, equalAt), decode);
            const item = queryUnescape(equalAt < 0 ? "" : part.slice(equalAt + String(equals).length), decode);
            if (!(key in output)) output[key] = item;
            else if (Array.isArray(output[key])) output[key].push(item);
            else output[key] = [output[key], item];
        }
        return output;
    }
    function queryPrimitive(value) {
        return typeof value === "string" || typeof value === "number" || typeof value === "bigint" || typeof value === "boolean"
            ? String(value)
            : "";
    }
    function queryStringify(value, separator = "&", equals = "=", options = {}) {
        if (value === null || typeof value !== "object") return "";
        const encode = typeof options.encodeURIComponent === "function" ? options.encodeURIComponent : encodeURIComponent;
        const parts = [];
        for (const key of Object.keys(value)) {
            const encodedKey = encode(key);
            const items = Array.isArray(value[key]) ? value[key] : [value[key]];
            if (!items.length) parts.push(`${encodedKey}${equals}`);
            for (const item of items) parts.push(`${encodedKey}${equals}${encode(queryPrimitive(item))}`);
        }
        return parts.join(String(separator));
    }
    const querystringModule = {
        decode: queryParse,
        parse: queryParse,
        encode: queryStringify,
        stringify: queryStringify,
        escape: encodeURIComponent,
        unescape: queryUnescape,
        unescapeBuffer(value, decodeSpaces) {
            return Buffer.from(queryUnescape(decodeSpaces ? String(value).replace(/\+/g, " ") : value));
        }
    };

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
    function parseArgs(config = {}) {
        const args = config.args === undefined ? process.argv.slice(2) : Array.from(config.args, String);
        const definitions = config.options || {};
        const strict = config.strict !== false;
        const allowPositionals = config.allowPositionals === undefined ? !strict : Boolean(config.allowPositionals);
        const allowNegative = Boolean(config.allowNegative);
        const values = Object.create(null);
        const positionals = [];
        const tokens = [];
        const shortNames = new Map();
        for (const [name, definition] of Object.entries(definitions)) {
            if (!definition || (definition.type !== "string" && definition.type !== "boolean")) {
                throw new TypeError(`option '${name}' must declare type 'string' or 'boolean'`);
            }
            if (definition.short !== undefined) shortNames.set(String(definition.short), name);
            if (definition.default !== undefined) {
                values[name] = definition.multiple && Array.isArray(definition.default)
                    ? definition.default.slice()
                    : definition.default;
            }
        }
        const setOption = (name, value, definition) => {
            if (definition?.multiple) {
                if (!Array.isArray(values[name])) values[name] = [];
                values[name].push(value);
            } else values[name] = value;
        };
        const unknownOption = rawName => {
            if (strict) throw new TypeError(`Unknown option '${rawName}'`);
        };
        let index = 0;
        while (index < args.length) {
            const argumentIndex = index;
            const argument = args[index];
            if (argument === "--") {
                if (config.tokens) tokens.push({ kind: "option-terminator", index: argumentIndex });
                for (index += 1; index < args.length; index++) {
                    positionals.push(args[index]);
                    if (config.tokens) tokens.push({ kind: "positional", index, value: args[index] });
                }
                break;
            }
            if (argument.startsWith("--") && argument.length > 2) {
                const equal = argument.indexOf("=");
                const rawName = equal < 0 ? argument.slice(2) : argument.slice(2, equal);
                let name = rawName;
                let negative = false;
                if (allowNegative && rawName.startsWith("no-") && definitions[rawName.slice(3)]?.type === "boolean") {
                    name = rawName.slice(3);
                    negative = true;
                }
                const definition = definitions[name];
                if (!definition) unknownOption(`--${rawName}`);
                let value;
                let inlineValue = false;
                if (!definition) {
                    value = equal < 0 ? true : argument.slice(equal + 1);
                    inlineValue = equal >= 0;
                } else if (definition.type === "boolean") {
                    if (equal >= 0 && strict) throw new TypeError(`Option '--${rawName}' does not take an argument`);
                    value = negative ? false : equal < 0 ? true : argument.slice(equal + 1) !== "false";
                    inlineValue = equal >= 0;
                } else {
                    if (negative) throw new TypeError(`Option '--${rawName}' cannot be negated`);
                    if (equal >= 0) { value = argument.slice(equal + 1); inlineValue = true; }
                    else if (index + 1 < args.length) value = args[++index];
                    else throw new TypeError(`Option '--${rawName}' argument is missing`);
                }
                setOption(name, value, definition);
                if (config.tokens) tokens.push({ kind: "option", index: argumentIndex, name, rawName: `--${rawName}`, value, inlineValue });
                index++;
                continue;
            }
            if (argument.startsWith("-") && argument !== "-") {
                const group = argument.slice(1);
                let consumedValue = false;
                for (let offset = 0; offset < group.length; offset++) {
                    const short = group[offset];
                    const name = shortNames.get(short) || short;
                    const definition = definitions[name];
                    if (!definition) unknownOption(`-${short}`);
                    let value = true;
                    let inlineValue = false;
                    if (definition?.type === "string") {
                        if (offset + 1 < group.length) {
                            value = group.slice(offset + 1);
                            inlineValue = true;
                        } else if (index + 1 < args.length) value = args[++index];
                        else throw new TypeError(`Option '-${short}' argument is missing`);
                        consumedValue = true;
                    }
                    setOption(name, value, definition);
                    if (config.tokens) tokens.push({ kind: "option", index: argumentIndex, name, rawName: `-${short}`, value, inlineValue });
                    if (consumedValue) break;
                }
                index++;
                continue;
            }
            if (!allowPositionals) throw new TypeError(`Unexpected argument '${argument}'`);
            positionals.push(argument);
            if (config.tokens) tokens.push({ kind: "positional", index, value: argument });
            index++;
        }
        const result = { values, positionals };
        if (config.tokens) result.tokens = tokens;
        return result;
    }
    const mimeTokenPattern = /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/;
    function splitMimeSegments(value) {
        const segments = [];
        let current = "";
        let quoted = false;
        let escaped = false;
        for (const character of String(value)) {
            if (escaped) { current += character; escaped = false; continue; }
            if (quoted && character === "\\") { current += character; escaped = true; continue; }
            if (character === '"') quoted = !quoted;
            if (character === ";" && !quoted) { segments.push(current); current = ""; }
            else current += character;
        }
        segments.push(current);
        return segments;
    }
    function parseMimeParameter(value) {
        const source = value.trim();
        if (source.startsWith('"') && source.endsWith('"')) {
            return source.slice(1, -1).replace(/\\([\\"])/g, "$1");
        }
        return source;
    }
    function serializeMimeParameter(value) {
        const source = String(value);
        return source && mimeTokenPattern.test(source)
            ? source
            : `"${source.replace(/([\\"])/g, "\\$1")}"`;
    }
    class MIMEParams {
        constructor(entries) {
            this._values = new Map();
            if (entries) {
                for (const [name, value] of entries) this.set(name, value);
            }
        }
        delete(name) { this._values.delete(String(name).toLowerCase()); }
        entries() { return this._values.entries(); }
        get(name) { return this._values.get(String(name).toLowerCase()) ?? null; }
        has(name) { return this._values.has(String(name).toLowerCase()); }
        keys() { return this._values.keys(); }
        set(name, value) {
            const normalized = String(name).toLowerCase();
            if (!mimeTokenPattern.test(normalized)) throw new TypeError(`Invalid MIME parameter name: ${name}`);
            const contents = String(value);
            if (/[^\t\x20-\x7e\x80-\xff]/.test(contents)) throw new TypeError(`Invalid MIME parameter value: ${value}`);
            this._values.set(normalized, contents);
            return this;
        }
        values() { return this._values.values(); }
        toString() {
            return [...this._values].map(([name, value]) => `${name}=${serializeMimeParameter(value)}`).join(";");
        }
        [Symbol.iterator]() { return this.entries(); }
        get [Symbol.toStringTag]() { return "MIMEParams"; }
    }
    class MIMEType {
        constructor(input) {
            const segments = splitMimeSegments(input);
            const essence = segments.shift().trim();
            const slash = essence.indexOf("/");
            if (slash <= 0 || slash === essence.length - 1) throw new TypeError(`Invalid MIME type: ${input}`);
            this._type = essence.slice(0, slash).trim().toLowerCase();
            this._subtype = essence.slice(slash + 1).trim().toLowerCase();
            if (!mimeTokenPattern.test(this._type) || !mimeTokenPattern.test(this._subtype)) {
                throw new TypeError(`Invalid MIME type: ${input}`);
            }
            this.params = new MIMEParams();
            for (const segment of segments) {
                const equal = segment.indexOf("=");
                if (equal <= 0) continue;
                const name = segment.slice(0, equal).trim().toLowerCase();
                if (!mimeTokenPattern.test(name) || this.params.has(name)) continue;
                this.params.set(name, parseMimeParameter(segment.slice(equal + 1)));
            }
        }
        get type() { return this._type; }
        set type(value) {
            const normalized = String(value).toLowerCase();
            if (!mimeTokenPattern.test(normalized)) throw new TypeError(`Invalid MIME type: ${value}`);
            this._type = normalized;
        }
        get subtype() { return this._subtype; }
        set subtype(value) {
            const normalized = String(value).toLowerCase();
            if (!mimeTokenPattern.test(normalized)) throw new TypeError(`Invalid MIME subtype: ${value}`);
            this._subtype = normalized;
        }
        get essence() { return `${this._type}/${this._subtype}`; }
        toString() {
            const parameters = this.params.toString();
            return `${this.essence}${parameters ? `;${parameters}` : ""}`;
        }
        toJSON() { return this.toString(); }
        get [Symbol.toStringTag]() { return "MIMEType"; }
    }
    const utilTypes = {
        isDate: value => value instanceof Date,
        isRegExp: value => value instanceof RegExp,
        isNativeError: value => value instanceof Error,
        isArrayBuffer: value => value instanceof ArrayBuffer,
        isArrayBufferView: value => ArrayBuffer.isView(value),
        isTypedArray: value => ArrayBuffer.isView(value) && !(value instanceof DataView),
        isUint8Array: value => value instanceof Uint8Array,
        isPromise: value => value instanceof Promise,
        isMap: value => value instanceof Map,
        isSet: value => value instanceof Set
    };
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
        parseArgs,
        MIMEType,
        MIMEParams,
        stripVTControlCharacters(value) { return String(value).replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, ""); },
        TextEncoder,
        TextDecoder,
        types: utilTypes
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
    let defaultByteHighWaterMark = 64 * 1024;
    let defaultObjectHighWaterMark = 16;
    function resolveHighWaterMark(options, objectMode) {
        if (options.highWaterMark === undefined) {
            return objectMode ? defaultObjectHighWaterMark : defaultByteHighWaterMark;
        }
        const value = Number(options.highWaterMark);
        if (!Number.isInteger(value) || value < 0) throw new RangeError("highWaterMark must be a non-negative integer");
        return value;
    }
    function Stream() { EventEmitter.call(this); this.destroyed = false; this.errored = null; }
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
        if (error) this.errored = error;
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
        this._readableObjectMode = Boolean(options.objectMode);
        this.readableHighWaterMark = resolveHighWaterMark(options, this._readableObjectMode);
        this._readableDidRead = false;
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
            this._readableDidRead = true;
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
        if (value !== null) {
            this.readableLength -= this._chunkLength(value);
            this._readableDidRead = true;
        }
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
    Readable.prototype.iterator = function (options = {}) {
        const stream = this;
        const waiting = [];
        let stopped = false;
        let failure;
        const cleanup = () => {
            stream.removeListener("readable", flush);
            stream.removeListener("end", onEnd);
            stream.removeListener("error", onError);
            stream.removeListener("close", onClose);
        };
        const finish = error => {
            if (stopped) return;
            stopped = true;
            failure = error;
            cleanup();
            while (waiting.length > 0) {
                const waiter = waiting.shift();
                if (error) waiter.reject(error);
                else waiter.resolve({ value: undefined, done: true });
            }
        };
        const flush = () => {
            while (waiting.length > 0) {
                const value = stream.read();
                if (value === null) break;
                waiting.shift().resolve({ value, done: false });
            }
            if (stream.readableEnded) finish();
        };
        const onEnd = () => finish();
        const onClose = () => finish();
        const onError = error => finish(error);
        stream.on("readable", flush);
        stream.once("end", onEnd);
        stream.once("error", onError);
        stream.once("close", onClose);
        return {
            next() {
                if (failure) return Promise.reject(failure);
                const value = stream.read();
                if (value !== null) return Promise.resolve({ value, done: false });
                if (stopped || stream.readableEnded) {
                    return Promise.resolve({ value: undefined, done: true });
                }
                return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
            },
            return() {
                finish();
                if (options.destroyOnReturn !== false) stream.destroy();
                return Promise.resolve({ value: undefined, done: true });
            },
            throw(error) {
                finish(error);
                stream.destroy(error);
                return Promise.reject(error);
            },
            [Symbol.asyncIterator]() { return this; }
        };
    };
    Readable.prototype[Symbol.asyncIterator] = function () { return this.iterator(); };
    Readable.from = function (iterable) {
        const readable = new Readable();
        setImmediate(async () => {
            try {
                for await (const chunk of iterable) readable.push(chunk);
                readable.push(null);
            } catch (error) {
                readable.destroy(error);
            }
        });
        return readable;
    };

    function initializeWritable(stream, options) {
        stream.writable = true;
        stream.writableEnded = false;
        stream.writableFinished = false;
        stream.writableLength = 0;
        stream.writableNeedDrain = false;
        stream._writableObjectMode = Boolean(options.objectMode);
        stream.writableHighWaterMark = resolveHighWaterMark(options, stream._writableObjectMode);
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
        const cleanup = () => {
            stream.removeListener(event, onComplete);
            stream.removeListener("error", onError);
        };
        const onComplete = () => { cleanup(); callback(); };
        const onError = error => { cleanup(); callback(error); };
        stream.once(event, onComplete);
        stream.once("error", onError);
        return cleanup;
    }
    function pipeline(...streams) {
        const callback = typeof streams[streams.length - 1] === "function" ? streams.pop() : () => {};
        for (let index = 0; index + 1 < streams.length; index++) streams[index].pipe(streams[index + 1]);
        finished(streams[streams.length - 1], callback);
        return streams[streams.length - 1];
    }
    function addAbortSignal(signal, stream) {
        if (!stream || typeof stream.destroy !== "function") throw new TypeError("stream must be a Stream");
        const disposable = addAbortListener(signal, () => stream.destroy(abortApiError(signal.reason)));
        stream.once("close", () => disposable[disposeSymbol]());
        if (signal.aborted) stream.destroy(abortApiError(signal.reason));
        return stream;
    }
    function getDefaultHighWaterMark(objectMode) {
        return objectMode ? defaultObjectHighWaterMark : defaultByteHighWaterMark;
    }
    function setDefaultHighWaterMark(objectMode, value) {
        const normalized = Number(value);
        if (!Number.isInteger(normalized) || normalized < 0) {
            throw new RangeError("value must be a non-negative integer");
        }
        if (objectMode) defaultObjectHighWaterMark = normalized;
        else defaultByteHighWaterMark = normalized;
    }
    function isDestroyed(stream) {
        return stream && typeof stream === "object" ? Boolean(stream.destroyed) : null;
    }
    function isDisturbed(stream) {
        return stream && typeof stream === "object"
            ? Boolean(stream._readableDidRead || stream.readableEnded)
            : null;
    }
    function isErrored(stream) {
        return stream && typeof stream === "object" ? stream.errored != null : null;
    }
    function isReadable(stream) {
        if (!stream || typeof stream !== "object" || !("readable" in stream)) return null;
        return Boolean(stream.readable && !stream.destroyed && !stream.readableEnded);
    }
    function isWritable(stream) {
        if (!stream || typeof stream !== "object" || !("writable" in stream)) return null;
        return Boolean(stream.writable && !stream.destroyed && !stream.writableEnded);
    }
    Object.assign(Stream, {
        Stream,
        Readable,
        Writable,
        Duplex,
        Transform,
        PassThrough,
        finished,
        pipeline,
        addAbortSignal,
        getDefaultHighWaterMark,
        setDefaultHighWaterMark,
        isDestroyed,
        isDisturbed,
        isErrored,
        isReadable,
        isWritable,
        _isUint8Array: value => value instanceof Uint8Array,
        _uint8ArrayToBuffer: value => Buffer.from(value)
    });
    Readable.Readable = Readable;
    Writable.Writable = Writable;
    Duplex.Duplex = Duplex;
    Transform.Transform = Transform;
    PassThrough.PassThrough = PassThrough;

    function deferred() {
        let resolve;
        let reject;
        const promise = new Promise((resolvePromise, rejectPromise) => {
            resolve = resolvePromise;
            reject = rejectPromise;
        });
        return { promise, resolve, reject };
    }

    class ReadableStreamDefaultController {
        constructor(stream) { this._stream = stream; }
        get desiredSize() {
            const stream = this._stream;
            return stream._state === "readable" ? stream._highWaterMark - stream._queueSize : null;
        }
        enqueue(chunk) { this._stream._enqueue(chunk); }
        close() { this._stream._close(); }
        error(reason) { this._stream._error(reason); }
    }

    class ReadableStreamDefaultReader {
        constructor(stream) {
            if (!(stream instanceof ReadableStream)) throw new TypeError("stream must be a ReadableStream");
            if (stream.locked) throw new TypeError("ReadableStream is locked");
            this._stream = stream;
            stream._reader = this;
        }
        get closed() {
            if (!this._stream) return Promise.reject(new TypeError("Reader has no stream"));
            return this._stream._closed.promise;
        }
        read() {
            if (!this._stream) return Promise.reject(new TypeError("Reader has no stream"));
            return this._stream._read();
        }
        cancel(reason) {
            if (!this._stream) return Promise.reject(new TypeError("Reader has no stream"));
            return this._stream._cancel(reason);
        }
        releaseLock() {
            if (!this._stream) return;
            if (this._stream._reads.length) throw new TypeError("Cannot release a reader with pending reads");
            this._stream._reader = null;
            this._stream = null;
        }
    }

    class ReadableStream {
        constructor(underlyingSource = {}, strategy = {}) {
            if (underlyingSource === null || typeof underlyingSource !== "object") throw new TypeError("underlyingSource must be an object");
            if (underlyingSource.type !== undefined && underlyingSource.type !== "bytes") throw new RangeError("invalid ReadableStream type");
            this._source = underlyingSource;
            this._queue = [];
            this._queueSize = 0;
            this._reads = [];
            this._reader = null;
            this._state = "readable";
            this._storedError = undefined;
            this._closeRequested = false;
            this._pulling = false;
            this._pullAgain = false;
            this._size = typeof strategy.size === "function" ? strategy.size : () => 1;
            this._highWaterMark = strategy.highWaterMark === undefined ? 1 : Number(strategy.highWaterMark);
            if (!Number.isFinite(this._highWaterMark) || this._highWaterMark < 0) throw new RangeError("highWaterMark must be non-negative");
            this._closed = deferred();
            this._controller = new ReadableStreamDefaultController(this);
            let started;
            try { started = underlyingSource.start?.(this._controller); }
            catch (error) { this._error(error); return; }
            Promise.resolve(started).then(() => this._callPull(), error => this._error(error));
        }
        get locked() { return this._reader !== null; }
        cancel(reason) {
            if (this.locked) return Promise.reject(new TypeError("Cannot cancel a locked ReadableStream"));
            return this._cancel(reason);
        }
        _cancel(reason) {
            if (this._state === "closed") return Promise.resolve();
            if (this._state === "errored") return Promise.reject(this._storedError);
            this._queue.length = 0;
            this._queueSize = 0;
            this._finishClose();
            try { return Promise.resolve(this._source.cancel?.(reason)); }
            catch (error) { return Promise.reject(error); }
        }
        getReader(options = {}) {
            if (options.mode === "byob") throw new TypeError("BYOB readers are not supported yet");
            return new ReadableStreamDefaultReader(this);
        }
        _enqueue(chunk) {
            if (this._state !== "readable" || this._closeRequested) throw new TypeError("ReadableStream is not readable");
            if (this._reads.length) this._reads.shift().resolve({ value: chunk, done: false });
            else {
                let size;
                try { size = Number(this._size(chunk)); }
                catch (error) { this._error(error); throw error; }
                if (!Number.isFinite(size) || size < 0) throw new RangeError("chunk size must be non-negative");
                this._queue.push({ chunk, size });
                this._queueSize += size;
            }
            this._callPull();
        }
        _close() {
            if (this._state !== "readable" || this._closeRequested) throw new TypeError("ReadableStream cannot be closed");
            this._closeRequested = true;
            if (!this._queue.length) this._finishClose();
        }
        _finishClose() {
            if (this._state !== "readable") return;
            this._state = "closed";
            while (this._reads.length) this._reads.shift().resolve({ value: undefined, done: true });
            this._closed.resolve();
        }
        _error(reason) {
            if (this._state !== "readable") return;
            this._state = "errored";
            this._storedError = reason;
            this._queue.length = 0;
            this._queueSize = 0;
            while (this._reads.length) this._reads.shift().reject(reason);
            this._closed.reject(reason);
        }
        _read() {
            if (this._queue.length) {
                const entry = this._queue.shift();
                this._queueSize -= entry.size;
                if (this._closeRequested && !this._queue.length) this._finishClose();
                else this._callPull();
                return Promise.resolve({ value: entry.chunk, done: false });
            }
            if (this._state === "closed") return Promise.resolve({ value: undefined, done: true });
            if (this._state === "errored") return Promise.reject(this._storedError);
            const pending = deferred();
            this._reads.push(pending);
            this._callPull();
            return pending.promise;
        }
        _callPull() {
            if (this._state !== "readable" || this._closeRequested || typeof this._source.pull !== "function") return;
            if (!this._reads.length && this._queueSize >= this._highWaterMark) return;
            if (this._pulling) { this._pullAgain = true; return; }
            this._pulling = true;
            let result;
            try { result = this._source.pull(this._controller); }
            catch (error) { this._pulling = false; this._error(error); return; }
            Promise.resolve(result).then(() => {
                this._pulling = false;
                if (this._pullAgain) { this._pullAgain = false; this._callPull(); }
            }, error => { this._pulling = false; this._error(error); });
        }
        pipeThrough(transform, options) {
            this.pipeTo(transform.writable, options).catch(() => {});
            return transform.readable;
        }
        async pipeTo(destination, options = {}) {
            const reader = this.getReader();
            const writer = destination.getWriter();
            const onAbort = () => {
                const reason = options.signal.reason;
                reader.cancel(reason).catch(() => {});
                writer.abort(reason).catch(() => {});
            };
            if (options.signal) {
                if (options.signal.aborted) onAbort();
                else options.signal.addEventListener("abort", onAbort, { once: true });
            }
            try {
                while (true) {
                    const result = await reader.read();
                    if (result.done) break;
                    await writer.write(result.value);
                }
                if (!options.preventClose) await writer.close();
            } catch (error) {
                if (!options.preventAbort) await writer.abort(error).catch(() => {});
                if (!options.preventCancel) await reader.cancel(error).catch(() => {});
                throw error;
            } finally {
                options.signal?.removeEventListener("abort", onAbort);
                reader.releaseLock();
                writer.releaseLock();
            }
        }
        tee() {
            const reader = this.getReader();
            let first;
            let second;
            let reading = false;
            const pull = async () => {
                if (reading) return;
                reading = true;
                try {
                    const result = await reader.read();
                    if (result.done) { first.close(); second.close(); }
                    else { first.enqueue(result.value); second.enqueue(result.value); }
                } catch (error) { first.error(error); second.error(error); }
                finally { reading = false; }
            };
            const left = new ReadableStream({ start(controller) { first = controller; }, pull });
            const right = new ReadableStream({ start(controller) { second = controller; }, pull });
            return [left, right];
        }
        values(options = {}) {
            const reader = this.getReader();
            return {
                next: () => reader.read(),
                async return(value) {
                    if (!options.preventCancel) await reader.cancel(value);
                    reader.releaseLock();
                    return { value, done: true };
                },
                [Symbol.asyncIterator]() { return this; }
            };
        }
        [Symbol.asyncIterator]() { return this.values(); }
        get [Symbol.toStringTag]() { return "ReadableStream"; }
    }

    class WritableStreamDefaultController {
        constructor(stream) { this._stream = stream; }
        error(reason) { this._stream._error(reason); }
        get signal() { return this._stream._abortController.signal; }
    }

    class WritableStreamDefaultWriter {
        constructor(stream) {
            if (!(stream instanceof WritableStream)) throw new TypeError("stream must be a WritableStream");
            if (stream.locked) throw new TypeError("WritableStream is locked");
            this._stream = stream;
            stream._writer = this;
        }
        get closed() { return this._stream ? this._stream._closed.promise : Promise.reject(new TypeError("Writer has no stream")); }
        get ready() { return this._stream ? this._stream._ready : Promise.reject(new TypeError("Writer has no stream")); }
        get desiredSize() { return this._stream ? this._stream._highWaterMark - this._stream._queueSize : null; }
        write(chunk) { return this._stream ? this._stream._write(chunk) : Promise.reject(new TypeError("Writer has no stream")); }
        close() { return this._stream ? this._stream._close() : Promise.reject(new TypeError("Writer has no stream")); }
        abort(reason) { return this._stream ? this._stream._abort(reason) : Promise.reject(new TypeError("Writer has no stream")); }
        releaseLock() {
            if (!this._stream) return;
            this._stream._writer = null;
            this._stream = null;
        }
    }

    class WritableStream {
        constructor(underlyingSink = {}, strategy = {}) {
            if (underlyingSink === null || typeof underlyingSink !== "object") throw new TypeError("underlyingSink must be an object");
            this._sink = underlyingSink;
            this._writer = null;
            this._state = "writable";
            this._storedError = undefined;
            this._size = typeof strategy.size === "function" ? strategy.size : () => 1;
            this._highWaterMark = strategy.highWaterMark === undefined ? 1 : Number(strategy.highWaterMark);
            if (!Number.isFinite(this._highWaterMark) || this._highWaterMark < 0) throw new RangeError("highWaterMark must be non-negative");
            this._queueSize = 0;
            this._closed = deferred();
            this._ready = Promise.resolve();
            this._abortController = new AbortController();
            this._controller = new WritableStreamDefaultController(this);
            try { this._chain = Promise.resolve(underlyingSink.start?.(this._controller)); }
            catch (error) { this._chain = Promise.reject(error); this._error(error); }
        }
        get locked() { return this._writer !== null; }
        close() {
            if (this.locked) return Promise.reject(new TypeError("Cannot close a locked WritableStream"));
            return this._close();
        }
        abort(reason) {
            if (this.locked) return Promise.reject(new TypeError("Cannot abort a locked WritableStream"));
            return this._abort(reason);
        }
        _abort(reason) {
            if (this._state === "closed") return Promise.resolve();
            if (this._state === "errored") return Promise.reject(this._storedError);
            this._abortController.abort(reason);
            this._error(reason);
            try { return Promise.resolve(this._sink.abort?.(reason)); }
            catch (error) { return Promise.reject(error); }
        }
        getWriter() { return new WritableStreamDefaultWriter(this); }
        _write(chunk) {
            if (this._state !== "writable") return Promise.reject(this._storedError || new TypeError("WritableStream is not writable"));
            let size;
            try { size = Number(this._size(chunk)); }
            catch (error) { this._error(error); return Promise.reject(error); }
            if (!Number.isFinite(size) || size < 0) return Promise.reject(new RangeError("chunk size must be non-negative"));
            this._queueSize += size;
            const operation = this._chain.then(() => this._sink.write?.(chunk, this._controller));
            this._chain = operation.then(() => { this._queueSize -= size; }, error => { this._queueSize -= size; this._error(error); });
            return operation;
        }
        _close() {
            if (this._state !== "writable") return Promise.reject(this._storedError || new TypeError("WritableStream cannot be closed"));
            this._state = "closing";
            const operation = this._chain.then(() => this._sink.close?.());
            this._chain = operation.then(() => { this._state = "closed"; this._closed.resolve(); }, error => this._error(error));
            return operation;
        }
        _error(reason) {
            if (this._state === "closed" || this._state === "errored") return;
            this._state = "errored";
            this._storedError = reason;
            this._closed.reject(reason);
        }
        get [Symbol.toStringTag]() { return "WritableStream"; }
    }

    class TransformStreamDefaultController {
        constructor(readableController) { this._readable = readableController; }
        get desiredSize() { return this._readable.desiredSize; }
        enqueue(chunk) { this._readable.enqueue(chunk); }
        error(reason) { this._readable.error(reason); }
        terminate() { this._readable.close(); }
    }

    class TransformStream {
        constructor(transformer = {}, writableStrategy = {}, readableStrategy = {}) {
            let readableController;
            this.readable = new ReadableStream({ start(controller) { readableController = controller; } }, readableStrategy);
            const controller = new TransformStreamDefaultController(readableController);
            let started;
            try { started = transformer.start?.(controller); }
            catch (error) { readableController.error(error); started = Promise.reject(error); }
            const startPromise = Promise.resolve(started);
            this.writable = new WritableStream({
                write(chunk) {
                    return startPromise.then(() => transformer.transform
                        ? transformer.transform(chunk, controller)
                        : controller.enqueue(chunk));
                },
                close() {
                    return startPromise.then(() => transformer.flush?.(controller)).then(() => readableController.close());
                },
                abort(reason) { readableController.error(reason); }
            }, writableStrategy);
        }
        get [Symbol.toStringTag]() { return "TransformStream"; }
    }

    class ByteLengthQueuingStrategy {
        constructor({ highWaterMark }) { this.highWaterMark = Number(highWaterMark); }
        size(chunk) { return chunk.byteLength; }
        get [Symbol.toStringTag]() { return "ByteLengthQueuingStrategy"; }
    }
    class CountQueuingStrategy {
        constructor({ highWaterMark }) { this.highWaterMark = Number(highWaterMark); }
        size() { return 1; }
        get [Symbol.toStringTag]() { return "CountQueuingStrategy"; }
    }

    class TextEncoderStream {
        constructor() {
            const stream = new TransformStream({ transform(chunk, controller) { controller.enqueue(new TextEncoder().encode(chunk)); } });
            this.readable = stream.readable;
            this.writable = stream.writable;
            this.encoding = "utf-8";
        }
        get [Symbol.toStringTag]() { return "TextEncoderStream"; }
    }
    class TextDecoderStream {
        constructor(label = "utf-8") {
            const decoder = new TextDecoder(label);
            const stream = new TransformStream({ transform(chunk, controller) { controller.enqueue(decoder.decode(chunk)); } });
            this.readable = stream.readable;
            this.writable = stream.writable;
            this.encoding = "utf-8";
            this.fatal = false;
            this.ignoreBOM = false;
        }
        get [Symbol.toStringTag]() { return "TextDecoderStream"; }
    }

    const streamWeb = {
        ReadableStream,
        ReadableStreamDefaultReader,
        ReadableStreamDefaultController,
        WritableStream,
        WritableStreamDefaultWriter,
        WritableStreamDefaultController,
        TransformStream,
        TransformStreamDefaultController,
        ByteLengthQueuingStrategy,
        CountQueuingStrategy,
        TextEncoderStream,
        TextDecoderStream
    };
    Object.assign(globalThis, streamWeb);
    Readable.toWeb = (readable, options = {}) => new ReadableStream({
        start(controller) {
            readable.on("data", chunk => controller.enqueue(chunk));
            readable.once("end", () => controller.close());
            readable.once("error", error => controller.error(error));
        },
        cancel(reason) { readable.destroy(reason instanceof Error ? reason : undefined); }
    }, options.strategy);
    Readable.fromWeb = (readable, options = {}) => {
        const output = new Readable(options);
        const reader = readable.getReader();
        setImmediate(async () => {
            try {
                while (true) {
                    const result = await reader.read();
                    if (result.done) break;
                    output.push(result.value);
                }
                output.push(null);
            } catch (error) { output.destroy(error); }
            finally { reader.releaseLock(); }
        });
        return output;
    };
    Writable.toWeb = writable => new WritableStream({
        write(chunk) { return new Promise((resolve, reject) => writable.write(chunk, error => error ? reject(error) : resolve())); },
        close() { return new Promise((resolve, reject) => writable.end(error => error ? reject(error) : resolve())); },
        abort(reason) { writable.destroy(reason instanceof Error ? reason : undefined); }
    });
    Writable.fromWeb = (writable, options = {}) => {
        const writer = writable.getWriter();
        return new Writable({
            ...options,
            write(chunk, _encoding, callback) { writer.write(chunk).then(() => callback(), callback); },
            final(callback) { writer.close().then(() => callback(), callback); }
        });
    };

    function normalizeHeaderName(name) {
        const normalized = String(name).toLowerCase();
        if (!normalized || !/^[!#$%&'*+\-.^_`|~0-9a-z]+$/.test(normalized)) throw new TypeError(`Invalid header name: ${name}`);
        return normalized;
    }
    function normalizeHeaderValue(value) {
        const normalized = String(value).trim();
        if (/\0|\r|\n/.test(normalized)) throw new TypeError("Invalid header value");
        return normalized;
    }
    class Headers {
        constructor(init) {
            this._headers = new Map();
            if (init instanceof Headers) {
                for (const [name, values] of init._headers) this._headers.set(name, [...values]);
            } else if (init != null && typeof init[Symbol.iterator] === "function") {
                for (const entry of init) {
                    if (!entry || typeof entry[Symbol.iterator] !== "function") throw new TypeError("Header entry must be iterable");
                    const pair = [...entry];
                    if (pair.length !== 2) throw new TypeError("Header entry must contain exactly two values");
                    this.append(pair[0], pair[1]);
                }
            } else if (init != null && typeof init === "object") {
                for (const [name, value] of Object.entries(init)) this.append(name, value);
            }
        }
        append(name, value) {
            const key = normalizeHeaderName(name);
            const normalized = normalizeHeaderValue(value);
            const values = this._headers.get(key);
            if (values) values.push(normalized); else this._headers.set(key, [normalized]);
        }
        delete(name) { this._headers.delete(normalizeHeaderName(name)); }
        get(name) {
            const values = this._headers.get(normalizeHeaderName(name));
            return values ? values.join(", ") : null;
        }
        getSetCookie() { return [...(this._headers.get("set-cookie") || [])]; }
        has(name) { return this._headers.has(normalizeHeaderName(name)); }
        set(name, value) { this._headers.set(normalizeHeaderName(name), [normalizeHeaderValue(value)]); }
        *entries() { for (const [name, values] of this._headers) yield [name, values.join(", ")]; }
        *keys() { for (const [name] of this._headers) yield name; }
        *values() { for (const [, values] of this._headers) yield values.join(", "); }
        forEach(callback, thisArg) {
            for (const [name, value] of this) callback.call(thisArg, value, name, this);
        }
        [Symbol.iterator]() { return this.entries(); }
        get [Symbol.toStringTag]() { return "Headers"; }
    }

    class FormData {
        constructor() { this._entries = []; }
        append(name, value, filename) {
            const key = String(name);
            if (value instanceof Blob) {
                const file = value instanceof File && filename === undefined
                    ? value
                    : new File([value], filename === undefined ? "blob" : String(filename), {
                        type: value.type,
                        lastModified: value.lastModified
                    });
                this._entries.push([key, file]);
            } else this._entries.push([key, String(value)]);
        }
        delete(name) {
            const key = String(name);
            this._entries = this._entries.filter(([entryName]) => entryName !== key);
        }
        get(name) {
            const key = String(name);
            return this._entries.find(([entryName]) => entryName === key)?.[1] ?? null;
        }
        getAll(name) {
            const key = String(name);
            return this._entries.filter(([entryName]) => entryName === key).map(([, value]) => value);
        }
        has(name) {
            const key = String(name);
            return this._entries.some(([entryName]) => entryName === key);
        }
        set(name, value, filename) {
            const key = String(name);
            const first = this._entries.findIndex(([entryName]) => entryName === key);
            this.delete(key);
            this.append(key, value, filename);
            if (first >= 0) this._entries.splice(first, 0, this._entries.pop());
        }
        *entries() { yield* this._entries; }
        *keys() { for (const [name] of this._entries) yield name; }
        *values() { for (const [, value] of this._entries) yield value; }
        forEach(callback, thisArg) {
            for (const [name, value] of this._entries) callback.call(thisArg, value, name, this);
        }
        [Symbol.iterator]() { return this.entries(); }
        get [Symbol.toStringTag]() { return "FormData"; }
    }

    let nextFormBoundary = 0;
    function quoteFormName(value) {
        return String(value).replace(/\r/g, "%0D").replace(/\n/g, "%0A").replace(/"/g, "%22");
    }
    function serializeFormData(form) {
        const boundary = `----canaryo-${Date.now().toString(16)}-${++nextFormBoundary}`;
        const chunks = [];
        for (const [name, value] of form) {
            let disposition = `Content-Disposition: form-data; name="${quoteFormName(name)}"`;
            if (value instanceof File) disposition += `; filename="${quoteFormName(value.name)}"`;
            chunks.push(Buffer.from(`--${boundary}\r\n${disposition}\r\n`));
            if (value instanceof File && value.type) chunks.push(Buffer.from(`Content-Type: ${value.type}\r\n`));
            chunks.push(Buffer.from("\r\n"));
            chunks.push(value instanceof File ? Buffer.from(value._buffer) : Buffer.from(value));
            chunks.push(Buffer.from("\r\n"));
        }
        chunks.push(Buffer.from(`--${boundary}--\r\n`));
        return { bytes: Buffer.concat(chunks), type: `multipart/form-data; boundary=${boundary}` };
    }
    function parseFormData(bytes, contentType) {
        const form = new FormData();
        if (/^application\/x-www-form-urlencoded(?:;|$)/i.test(contentType)) {
            for (const [name, value] of new URLSearchParams(bytes.toString())) form.append(name, value);
            return form;
        }
        const boundaryMatch = contentType.match(/boundary=(?:"([^"]+)"|([^;\s]+))/i);
        if (!/^multipart\/form-data(?:;|$)/i.test(contentType) || !boundaryMatch) {
            throw new TypeError("Body cannot be parsed as FormData");
        }
        const boundary = boundaryMatch[1] || boundaryMatch[2];
        const source = bytes.toString("latin1");
        for (let part of source.split(`--${boundary}`).slice(1)) {
            if (part.startsWith("--")) break;
            if (part.startsWith("\r\n")) part = part.slice(2);
            if (part.endsWith("\r\n")) part = part.slice(0, -2);
            const headerEnd = part.indexOf("\r\n\r\n");
            if (headerEnd < 0) continue;
            const rawHeaders = part.slice(0, headerEnd);
            const contents = Buffer.from(part.slice(headerEnd + 4), "latin1");
            const disposition = rawHeaders.split("\r\n").find(line => /^content-disposition:/i.test(line));
            const name = disposition?.match(/(?:^|;)\s*name="([^"]*)"/i)?.[1];
            if (name === undefined) continue;
            const filename = disposition.match(/(?:^|;)\s*filename="([^"]*)"/i)?.[1];
            if (filename !== undefined) {
                const type = rawHeaders.split("\r\n").find(line => /^content-type:/i.test(line))?.split(":", 2)[1]?.trim() || "";
                form.append(name, new File([contents], filename, { type }));
            } else form.append(name, contents.toString());
        }
        return form;
    }

    function bodyWithType(value) {
        if (value === null || value === undefined) return { stream: null, type: null };
        if (value instanceof ReadableStream) return { stream: value, type: null };
        let bytes;
        let type = null;
        if (value instanceof FormData) {
            const serialized = serializeFormData(value);
            bytes = serialized.bytes;
            type = serialized.type;
        } else if (value instanceof Blob) {
            bytes = Buffer.from(value._buffer);
            type = value.type || null;
        } else if (value instanceof URLSearchParams) {
            bytes = Buffer.from(value.toString());
            type = "application/x-www-form-urlencoded;charset=UTF-8";
        } else if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) {
            bytes = Buffer.from(value);
        } else {
            bytes = Buffer.from(String(value));
            type = "text/plain;charset=UTF-8";
        }
        return {
            stream: new ReadableStream({ start(controller) { controller.enqueue(bytes); controller.close(); } }),
            type
        };
    }
    async function consumeBody(owner) {
        if (owner.bodyUsed) throw new TypeError("Body is unusable");
        owner._bodyUsed = true;
        if (owner.body === null) return Buffer.alloc(0);
        const reader = owner.body.getReader();
        const chunks = [];
        try {
            while (true) {
                const result = await reader.read();
                if (result.done) break;
                chunks.push(Buffer.from(result.value));
            }
        } finally { reader.releaseLock(); }
        return Buffer.concat(chunks);
    }
    const Body = {
        arrayBuffer() {
            return consumeBody(this).then(bytes => {
                const copy = new Uint8Array(bytes);
                return copy.buffer;
            });
        },
        blob() { return consumeBody(this).then(bytes => new Blob([bytes], { type: this.headers.get("content-type") || "" })); },
        bytes() { return consumeBody(this).then(bytes => new Uint8Array(bytes)); },
        formData() {
            const contentType = this.headers.get("content-type") || "";
            return consumeBody(this).then(bytes => parseFormData(bytes, contentType));
        },
        json() { return consumeBody(this).then(bytes => JSON.parse(bytes.toString())); },
        text() { return consumeBody(this).then(bytes => bytes.toString()); }
    };
    function installBodyMethods(prototype) {
        for (const [name, method] of Object.entries(Body)) Object.defineProperty(prototype, name, { value: method, writable: true, configurable: true });
    }

    class Request {
        constructor(input, init = {}) {
            const source = input instanceof Request ? input : null;
            this.url = source ? source.url : new URL(String(input)).href;
            this.method = String(init.method || source?.method || "GET").toUpperCase();
            this.headers = new Headers(init.headers === undefined ? source?.headers : init.headers);
            this.redirect = init.redirect || source?.redirect || "follow";
            this.credentials = init.credentials || source?.credentials || "same-origin";
            this.cache = init.cache || source?.cache || "default";
            this.mode = init.mode || source?.mode || "cors";
            this.referrer = init.referrer || source?.referrer || "about:client";
            this.referrerPolicy = init.referrerPolicy || source?.referrerPolicy || "";
            this.integrity = init.integrity || source?.integrity || "";
            this.keepalive = Boolean(init.keepalive ?? source?.keepalive);
            this.signal = init.signal || source?.signal || new AbortController().signal;
            this.duplex = init.duplex || source?.duplex || "half";
            const body = init.body === undefined ? source?.body : init.body;
            if ((this.method === "GET" || this.method === "HEAD") && body != null) throw new TypeError("Request with GET/HEAD method cannot have body");
            const converted = bodyWithType(body);
            this.body = converted.stream;
            this._bodyUsed = false;
            if (converted.type && !this.headers.has("content-type")) this.headers.set("content-type", converted.type);
        }
        get bodyUsed() { return this._bodyUsed; }
        clone() {
            if (this.bodyUsed || this.body?.locked) throw new TypeError("Body is unusable");
            if (this.body === null) return new Request(this);
            const [left, right] = this.body.tee();
            this.body = left;
            return new Request(this, { body: right });
        }
        get [Symbol.toStringTag]() { return "Request"; }
    }
    installBodyMethods(Request.prototype);

    class Response {
        constructor(body = null, init = {}) {
            const status = init.status === undefined ? 200 : Number(init.status);
            if (!Number.isInteger(status) || status < 200 || status > 599) throw new RangeError("status must be between 200 and 599");
            if ([204, 205, 304].includes(status) && body != null) throw new TypeError("Response status cannot have a body");
            const statusText = init.statusText === undefined ? "" : String(init.statusText);
            if (/\r|\n/.test(statusText)) throw new TypeError("Invalid status text");
            this.status = status;
            this.statusText = statusText;
            this.headers = new Headers(init.headers);
            this.type = "default";
            this.url = "";
            this.redirected = false;
            const converted = bodyWithType(body);
            this.body = converted.stream;
            this._bodyUsed = false;
            if (converted.type && !this.headers.has("content-type")) this.headers.set("content-type", converted.type);
        }
        get ok() { return this.status >= 200 && this.status <= 299; }
        get bodyUsed() { return this._bodyUsed; }
        clone() {
            if (this.bodyUsed || this.body?.locked) throw new TypeError("Body is unusable");
            if (this.body === null) return new Response(null, this);
            const [left, right] = this.body.tee();
            this.body = left;
            return new Response(right, this);
        }
        static error() {
            const response = new Response(null, { status: 500 });
            response.type = "error";
            response.status = 0;
            return response;
        }
        static json(value, init = {}) {
            const headers = new Headers(init.headers);
            if (!headers.has("content-type")) headers.set("content-type", "application/json");
            return new Response(JSON.stringify(value), { ...init, headers });
        }
        static redirect(url, status = 302) {
            if (![301, 302, 303, 307, 308].includes(status)) throw new RangeError("Invalid redirect status");
            return new Response(null, { status, headers: { location: new URL(String(url)).href } });
        }
        get [Symbol.toStringTag]() { return "Response"; }
    }
    installBodyMethods(Response.prototype);

    function fetchResponseBody(message) {
        return new ReadableStream({
            start(controller) {
                message.on("data", chunk => {
                    controller.enqueue(Buffer.from(chunk));
                    if (controller.desiredSize <= 0) message.pause();
                });
                message.once("end", () => controller.close());
                message.once("error", error => controller.error(error));
            },
            pull() { message.resume(); },
            cancel(reason) { message.destroy(reason instanceof Error ? reason : undefined); }
        });
    }
    function dispatchFetch(url, method, headers, body, signal, redirectMode, redirectCount) {
        return new Promise((resolve, reject) => {
            const target = new URL(url);
            const transport = target.protocol === "https:"
                ? globalThis.__canaryoHttpsModule
                : target.protocol === "http:"
                    ? globalThis.__canaryoHttpModule
                    : null;
            if (!transport) { reject(new TypeError(`Unsupported protocol: ${target.protocol}`)); return; }
            const request = transport.request(target.href, {
                method,
                headers: Object.fromEntries(headers),
                signal
            }, message => {
                const status = message.statusCode;
                const location = message.headers.location;
                const isRedirect = [301, 302, 303, 307, 308].includes(status) && location;
                if (isRedirect && redirectMode === "error") {
                    message.resume();
                    reject(new TypeError("Redirect encountered"));
                    return;
                }
                if (isRedirect && redirectMode === "follow") {
                    if (redirectCount >= 20) {
                        message.resume();
                        reject(new TypeError("Maximum redirect count exceeded"));
                        return;
                    }
                    message.resume();
                    const switchToGet = status === 303 || ((status === 301 || status === 302) && method === "POST");
                    const nextMethod = switchToGet ? "GET" : method;
                    const nextBody = switchToGet ? null : body;
                    const nextHeaders = new Headers(headers);
                    if (switchToGet) {
                        nextHeaders.delete("content-length");
                        nextHeaders.delete("content-type");
                    }
                    dispatchFetch(new URL(location, target).href, nextMethod, nextHeaders, nextBody, signal, redirectMode, redirectCount + 1).then(resolve, reject);
                    return;
                }
                const responseHeaders = new Headers(message.headers);
                const hasBody = method !== "HEAD" && ![204, 205, 304].includes(status);
                const response = new Response(hasBody ? fetchResponseBody(message) : null, {
                    status,
                    statusText: message.statusMessage,
                    headers: responseHeaders
                });
                response.url = target.href;
                response.redirected = redirectCount > 0;
                resolve(response);
            });
            request.once("error", reject);
            if (body && body.length) request.end(body); else request.end();
        });
    }
    async function fetch(input, init = {}) {
        const request = input instanceof Request && Object.keys(init).length === 0
            ? input
            : new Request(input, init);
        if (request.signal.aborted) throw abortApiError(request.signal.reason);
        if (request.body?.locked || request.bodyUsed) throw new TypeError("Body is unusable");
        const body = request.body === null ? null : await consumeBody(request);
        return dispatchFetch(
            request.url,
            request.method,
            request.headers,
            body,
            request.signal,
            request.redirect,
            0
        );
    }
    Object.assign(globalThis, { Headers, FormData, Request, Response, fetch });

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
    process.stdout = { isTTY: false, write(value) { __canaryoWrite(String(value)); return true; } };
    process.stderr = { isTTY: false, write(value) { __canaryoWriteError(String(value)); return true; } };
    const performanceTimeOrigin = Date.now() - __canaryoPerformanceNow();
    const performanceEntries = [];
    const performanceObservers = new Set();

    class PerformanceEntry {
        constructor(name, entryType, startTime, duration = 0, detail) {
            this.name = String(name);
            this.entryType = String(entryType);
            this.startTime = Number(startTime);
            this.duration = Number(duration);
            if (detail !== undefined) this.detail = detail;
        }
        toJSON() {
            const value = {
                name: this.name,
                entryType: this.entryType,
                startTime: this.startTime,
                duration: this.duration
            };
            if ("detail" in this) value.detail = this.detail;
            return value;
        }
    }

    class PerformanceMark extends PerformanceEntry {
        constructor(name, options = {}) {
            super(name, "mark", options.startTime === undefined ? performance.now() : options.startTime, 0, options.detail);
        }
    }

    class PerformanceMeasure extends PerformanceEntry {
        constructor(name, startTime, duration, detail) {
            super(name, "measure", startTime, duration, detail);
        }
    }

    class PerformanceObserverEntryList {
        constructor(entries) { this._entries = entries; }
        getEntries() { return this._entries.slice().sort((left, right) => left.startTime - right.startTime); }
        getEntriesByType(type) { return this.getEntries().filter(entry => entry.entryType === String(type)); }
        getEntriesByName(name, type) {
            return this.getEntries().filter(entry => entry.name === String(name) && (type === undefined || entry.entryType === String(type)));
        }
    }

    function publishPerformanceEntry(entry) {
        performanceEntries.push(entry);
        for (const observer of performanceObservers) {
            if (!observer._types.has(entry.entryType)) continue;
            observer._records.push(entry);
            if (observer._pending) continue;
            observer._pending = true;
            Promise.resolve().then(() => {
                observer._pending = false;
                if (!performanceObservers.has(observer) || !observer._records.length) return;
                const records = observer.takeRecords();
                observer._callback(new PerformanceObserverEntryList(records), observer);
            });
        }
        return entry;
    }

    class PerformanceObserver {
        constructor(callback) {
            if (typeof callback !== "function") throw new TypeError("callback must be a function");
            this._callback = callback;
            this._types = new Set();
            this._records = [];
            this._pending = false;
        }
        observe(options = {}) {
            const types = options.entryTypes || (options.type === undefined ? [] : [options.type]);
            this._types = new Set([...types].map(String));
            if (!this._types.size) throw new TypeError("entryTypes or type is required");
            performanceObservers.add(this);
            if (options.buffered) {
                this._records.push(...performanceEntries.filter(entry => this._types.has(entry.entryType)));
            }
        }
        disconnect() {
            performanceObservers.delete(this);
            this._records = [];
        }
        takeRecords() {
            const records = this._records;
            this._records = [];
            return records;
        }
        static get supportedEntryTypes() { return ["function", "mark", "measure", "resource"]; }
    }

    function resolvePerformanceTime(value, fallback) {
        if (value === undefined) return fallback;
        if (typeof value === "number") return value;
        const entries = performanceEntries.filter(entry => entry.name === String(value) && entry.entryType === "mark");
        if (!entries.length) throw new Error(`The "${value}" performance mark has not been set`);
        return entries[entries.length - 1].startTime;
    }

    function percentile(values, requested) {
        if (!values.length) return 0;
        const sorted = values.slice().sort((left, right) => left - right);
        const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(Number(requested) / 100 * sorted.length) - 1));
        return sorted[index];
    }

    function createHistogram() {
        const values = [];
        let previous;
        return {
            get count() { return values.length; },
            get countBigInt() { return BigInt(values.length); },
            get min() { return values.length ? Math.min(...values) : 9223372036854776000; },
            get minBigInt() { return BigInt(Math.trunc(this.min)); },
            get max() { return values.length ? Math.max(...values) : 0; },
            get maxBigInt() { return BigInt(Math.trunc(this.max)); },
            get mean() { return values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : NaN; },
            get stddev() {
                if (!values.length) return NaN;
                const mean = this.mean;
                return Math.sqrt(values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length);
            },
            get exceeds() { return 0; },
            get exceedsBigInt() { return 0n; },
            get percentiles() { return new Map([0, 25, 50, 75, 90, 99, 100].map(point => [point, percentile(values, point)])); },
            get percentilesBigInt() { return new Map([...this.percentiles].map(([point, value]) => [point, BigInt(Math.trunc(value))])); },
            record(value) {
                const number = Number(value);
                if (!Number.isFinite(number) || number < 1) throw new RangeError("value must be a positive integer");
                values.push(Math.trunc(number));
            },
            recordDelta() {
                const now = performance.now() * 1000000;
                if (previous !== undefined) values.push(Math.max(1, Math.trunc(now - previous)));
                previous = now;
            },
            percentile(point) { return percentile(values, point); },
            percentileBigInt(point) { return BigInt(Math.trunc(percentile(values, point))); },
            reset() { values.length = 0; previous = undefined; }
        };
    }

    const performance = {
        timeOrigin: performanceTimeOrigin,
        now: () => __canaryoPerformanceNow(),
        mark(name, options = {}) { return publishPerformanceEntry(new PerformanceMark(name, options)); },
        measure(name, startOrOptions, endMark) {
            const now = this.now();
            let start;
            let end;
            let duration;
            let detail;
            if (startOrOptions && typeof startOrOptions === "object") {
                detail = startOrOptions.detail;
                start = resolvePerformanceTime(startOrOptions.start, 0);
                end = resolvePerformanceTime(startOrOptions.end, now);
                if (startOrOptions.duration !== undefined) {
                    duration = Number(startOrOptions.duration);
                    if (startOrOptions.start === undefined) start = end - duration;
                    else end = start + duration;
                }
            } else {
                start = resolvePerformanceTime(startOrOptions, 0);
                end = resolvePerformanceTime(endMark, now);
            }
            return publishPerformanceEntry(new PerformanceMeasure(name, start, duration === undefined ? end - start : duration, detail));
        },
        getEntries() { return new PerformanceObserverEntryList(performanceEntries).getEntries(); },
        getEntriesByType(type) { return this.getEntries().filter(entry => entry.entryType === String(type)); },
        getEntriesByName(name, type) { return this.getEntries().filter(entry => entry.name === String(name) && (type === undefined || entry.entryType === String(type))); },
        clearMarks(name) { clearPerformanceEntries("mark", name); },
        clearMeasures(name) { clearPerformanceEntries("measure", name); },
        clearResourceTimings() { clearPerformanceEntries("resource"); },
        setResourceTimingBufferSize() {},
        markResourceTiming(timingInfo, requestedUrl, initiatorType = "fetch") {
            const startTime = timingInfo && Number(timingInfo.startTime) || this.now();
            return publishPerformanceEntry(new PerformanceEntry(requestedUrl, "resource", startTime, 0, { initiatorType }));
        },
        timerify(fn, options = {}) {
            if (typeof fn !== "function") throw new TypeError("fn must be a function");
            const name = fn.name || "anonymous";
            return function (...args) {
                const started = performance.now();
                const finish = () => {
                    const duration = performance.now() - started;
                    publishPerformanceEntry(new PerformanceEntry(name, "function", started, duration, args));
                    if (options.histogram) options.histogram.record(Math.max(1, Math.round(duration * 1000000)));
                };
                try {
                    const result = fn.apply(this, args);
                    if (result && typeof result.then === "function") return result.then(value => { finish(); return value; }, error => { finish(); throw error; });
                    finish();
                    return result;
                } catch (error) { finish(); throw error; }
            };
        },
        eventLoopUtilization(previous) {
            const idle = 0;
            const active = this.now();
            const baseIdle = previous ? Number(previous.idle) || 0 : 0;
            const baseActive = previous ? Number(previous.active) || 0 : 0;
            const currentActive = Math.max(0, active - baseActive);
            return { idle: Math.max(0, idle - baseIdle), active: currentActive, utilization: currentActive ? 1 : 0 };
        },
        nodeTiming: new PerformanceEntry("node", "node", 0, 0)
    };

    function clearPerformanceEntries(type, name) {
        for (let index = performanceEntries.length - 1; index >= 0; index--) {
            const entry = performanceEntries[index];
            if (entry.entryType === type && (name === undefined || entry.name === String(name))) performanceEntries.splice(index, 1);
        }
    }

    function monitorEventLoopDelay() {
        const histogram = createHistogram();
        histogram.enable = () => true;
        histogram.disable = () => true;
        histogram.ref = () => histogram;
        histogram.unref = () => histogram;
        histogram.hasRef = () => false;
        return histogram;
    }

    const perfHooksModule = {
        performance,
        PerformanceEntry,
        PerformanceMark,
        PerformanceMeasure,
        PerformanceObserver,
        PerformanceObserverEntryList,
        monitorEventLoopDelay,
        createHistogram,
        constants: {}
    };
    const scheduledTimers = new Map();
    let nextTimerId = 1;
    let pendingTimerContext;

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
            referenced: true,
            context: asyncContextEnabled ? captureAsyncContext() : undefined
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
            if (timer.context) {
                pendingTimerContext = enterAsyncContext(timer.context);
                try { timer.callback(...timer.args); }
                catch (error) {
                    restoreAsyncContext(pendingTimerContext);
                    pendingTimerContext = undefined;
                    throw error;
                }
                break;
            }
            timer.callback(...timer.args);
        }

        let nextDelay;
        const updatedNow = Date.now();
        for (const timer of scheduledTimers.values()) {
            const delay = Math.max(0, timer.due - updatedNow);
            if (nextDelay === undefined || delay < nextDelay) nextDelay = delay;
        }
        if (!pendingTimerContext) return nextDelay === undefined ? -1 : nextDelay;
        return nextDelay === undefined ? -2 : -nextDelay - 3;
    };
    globalThis.__canaryoRestoreTimerContext = () => {
        if (!pendingTimerContext) return;
        restoreAsyncContext(pendingTimerContext);
        pendingTimerContext = undefined;
    };
    globalThis.__canaryoHasReferencedTimers = () => {
        for (const timer of scheduledTimers.values()) {
            if (timer.referenced) return true;
        }
        return false;
    };

    globalThis.Buffer = Buffer;
    globalThis.Blob = Blob;
    globalThis.File = File;
    globalThis.atob = atob;
    globalThis.btoa = btoa;
    globalThis.DOMException = DOMException;
    globalThis.Event = Event;
    globalThis.MessageEvent = MessageEvent;
    globalThis.EventTarget = EventTarget;
    globalThis.AbortController = AbortController;
    globalThis.AbortSignal = AbortSignal;
    globalThis.TextEncoder = TextEncoder;
    globalThis.TextDecoder = TextDecoder;
    globalThis.URL = URL;
    globalThis.URLSearchParams = URLSearchParams;
    globalThis.performance = performance;
    globalThis.PerformanceEntry = PerformanceEntry;
    globalThis.PerformanceMark = PerformanceMark;
    globalThis.PerformanceMeasure = PerformanceMeasure;
    globalThis.PerformanceObserver = PerformanceObserver;
    globalThis.setImmediate = setImmediate;
    globalThis.clearImmediate = clearTimer;
    globalThis.setTimeout = setTimeout;
    globalThis.clearTimeout = clearTimer;
    globalThis.setInterval = setInterval;
    globalThis.clearInterval = clearTimer;
    globalThis.queueMicrotask = callback => {
        if (!asyncContextEnabled) {
            Promise.resolve().then(callback);
            return;
        }
        const snapshot = captureAsyncContext();
        originalPromiseThen.call(
            Promise.resolve(),
            snapshot
                ? () => runInAsyncContext(snapshot, callback, undefined, [])
            : callback
        );
    };

    function schedulePromiseTimer(schedule, delay, value, options = {}) {
        return new Promise((resolve, reject) => {
            if (options.signal?.aborted) {
                reject(abortApiError(options.signal.reason));
                return;
            }
            let handle;
            const onAbort = () => {
                clearTimer(handle);
                reject(abortApiError(options.signal.reason));
            };
            const complete = () => {
                options.signal?.removeEventListener?.("abort", onAbort);
                resolve(value);
            };
            handle = schedule(complete, delay);
            if (options.ref === false) handle.unref();
            options.signal?.addEventListener?.("abort", onAbort, { once: true });
        });
    }
    const timersPromises = {
        setTimeout(delay = 1, value, options) {
            return schedulePromiseTimer(setTimeout, delay, value, options);
        },
        setImmediate(value, options) {
            return schedulePromiseTimer(callback => setImmediate(callback), 0, value, options);
        },
        setInterval(delay = 1, value, options = {}) {
            let buffered = 0;
            let stopped = false;
            let failure;
            const waiting = [];
            const handle = setInterval(() => {
                const waiter = waiting.shift();
                if (waiter) waiter.resolve({ value, done: false });
                else buffered++;
            }, delay);
            if (options.ref === false) handle.unref();
            const onAbort = () => {
                failure = abortApiError(options.signal.reason);
                stopped = true;
                clearTimer(handle);
                while (waiting.length > 0) waiting.shift().reject(failure);
            };
            if (options.signal?.aborted) onAbort();
            else options.signal?.addEventListener?.("abort", onAbort, { once: true });
            return {
                next() {
                    if (failure) return Promise.reject(failure);
                    if (buffered > 0) {
                        buffered--;
                        return Promise.resolve({ value, done: false });
                    }
                    if (stopped) return Promise.resolve({ value: undefined, done: true });
                    return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
                },
                return() {
                    stopped = true;
                    clearTimer(handle);
                    options.signal?.removeEventListener?.("abort", onAbort);
                    while (waiting.length > 0) waiting.shift().resolve({ value: undefined, done: true });
                    return Promise.resolve({ value: undefined, done: true });
                },
                [Symbol.asyncIterator]() { return this; }
            };
        }
    };
    timersPromises.scheduler = {
        wait(delay, options) { return timersPromises.setTimeout(delay, undefined, options); },
        yield() { return timersPromises.setImmediate(); }
    };

    const streamPromises = {
        finished(stream, options = {}) {
            return new Promise((resolve, reject) => {
                let abortDisposable;
                const cleanup = finished(stream, error => {
                    abortDisposable?.[disposeSymbol]();
                    if (error) reject(error);
                    else resolve();
                });
                if (options.signal) {
                    abortDisposable = addAbortListener(options.signal, () => {
                        cleanup();
                        reject(abortApiError(options.signal.reason));
                    });
                }
            });
        },
        pipeline(...streams) {
            let options = {};
            if (streams.length > 0) {
                const candidate = streams[streams.length - 1];
                if (candidate && typeof candidate === "object" &&
                    typeof candidate.pipe !== "function" && typeof candidate.on !== "function") {
                    options = candidate;
                    streams.pop();
                }
            }
            return new Promise((resolve, reject) => {
                pipeline(...streams, error => error ? reject(error) : resolve());
                if (options.signal) {
                    for (const stream of streams) addAbortSignal(options.signal, stream);
                }
            });
        }
    };
    Stream.promises = streamPromises;

    async function consumeStream(stream) {
        const chunks = [];
        for await (const chunk of stream) {
            if (typeof chunk === "string") chunks.push(Buffer.from(chunk));
            else if (chunk instanceof ArrayBuffer || ArrayBuffer.isView(chunk)) chunks.push(Buffer.from(chunk));
            else chunks.push(Buffer.from(String(chunk)));
        }
        return Buffer.concat(chunks);
    }
    const streamConsumers = {
        async arrayBuffer(stream) {
            const contents = await consumeStream(stream);
            return new Uint8Array(contents).buffer;
        },
        async blob(stream) { return new Blob([await consumeStream(stream)]); },
        buffer: consumeStream,
        async json(stream) { return JSON.parse((await consumeStream(stream)).toString()); },
        async text(stream) { return (await consumeStream(stream)).toString(); }
    };

    function Stats(values, bigint = false) {
        Object.assign(this, values);
        this.mtime = new Date(values.mtimeMs);
        this.atime = new Date(values.atimeMs);
        this.ctime = new Date(values.ctimeMs);
        this.birthtime = new Date(values.birthtimeMs);
        if (bigint) {
            for (const name of ["dev", "ino", "mode", "nlink", "uid", "gid", "rdev", "size", "blksize", "blocks", "atimeMs", "mtimeMs", "ctimeMs", "birthtimeMs"]) {
                this[name] = BigInt(Math.trunc(values[name]));
            }
            this.atimeNs = BigInt(Math.trunc(values.atimeMs * 1e6));
            this.mtimeNs = BigInt(Math.trunc(values.mtimeMs * 1e6));
            this.ctimeNs = BigInt(Math.trunc(values.ctimeMs * 1e6));
            this.birthtimeNs = BigInt(Math.trunc(values.birthtimeMs * 1e6));
        }
    }
    Stats.prototype.isFile = function () { return this.file; };
    Stats.prototype.isDirectory = function () { return this.directory; };
    Stats.prototype.isSymbolicLink = function () { return this.symlink; };
    Stats.prototype.isBlockDevice = Stats.prototype.isCharacterDevice =
        Stats.prototype.isFIFO = Stats.prototype.isSocket = function () { return false; };

    function Dirent(name, parentPath, stats) {
        this.name = name;
        this.parentPath = parentPath;
        this.path = parentPath;
        this._stats = stats;
    }
    for (const method of ["isFile", "isDirectory", "isSymbolicLink", "isBlockDevice", "isCharacterDevice", "isFIFO", "isSocket"]) {
        Dirent.prototype[method] = function () { return this._stats[method](); };
    }

    function encodingFrom(options) {
        return typeof options === "string" ? options : options && options.encoding;
    }
    function normalizeFsPath(filename) {
        return filename instanceof URL ? fileURLToPath(filename) : String(filename);
    }
    let nextFileDescriptor = 10;
    const openFiles = new Map();
    function fileDescriptor(value) {
        const fd = Number(value);
        const file = openFiles.get(fd);
        if (!Number.isInteger(fd) || !file) throw new Error(`EBADF: bad file descriptor, fd ${value}`);
        return file;
    }
    function normalizeOpenFlags(flags = "r") {
        if (typeof flags === "number") {
            const access = flags & 3;
            return {
                readable: access === 0 || access === 2,
                writable: access === 1 || access === 2,
                create: Boolean(flags & 0x40),
                exclusive: Boolean(flags & 0x80),
                truncate: Boolean(flags & 0x200),
                append: Boolean(flags & 0x400),
                value: flags
            };
        }
        const value = String(flags);
        if (!["r", "r+", "rs", "rs+", "sr", "sr+", "w", "wx", "w+", "wx+", "xw", "xw+", "a", "ax", "a+", "ax+", "as", "as+"].includes(value)) {
            throw new TypeError(`Unknown file open flag: ${value}`);
        }
        return {
            readable: value.startsWith("r") || value.includes("+"),
            writable: !value.startsWith("r") || value.includes("+"),
            create: value.startsWith("w") || value.startsWith("a"),
            exclusive: value.includes("x"),
            truncate: value.startsWith("w"),
            append: value.startsWith("a"),
            value
        };
    }
    function openSync(filename, flags = "r", _mode = 0o666) {
        const path = normalizeFsPath(filename);
        const options = normalizeOpenFlags(flags);
        const exists = existsSync(path);
        if (!exists && !options.create) throw new Error(`ENOENT: no such file or directory, open '${path}'`);
        if (exists && options.exclusive && options.create) throw new Error(`EEXIST: file already exists, open '${path}'`);
        if (!exists) writeFileSync(path, Buffer.alloc(0));
        else if (options.truncate) writeFileSync(path, Buffer.alloc(0));
        const fd = nextFileDescriptor++;
        openFiles.set(fd, { path, ...options, position: options.append ? statSync(path).size : 0 });
        return fd;
    }
    function closeSync(fd) {
        fileDescriptor(fd);
        openFiles.delete(Number(fd));
    }
    function readFileSync(filename, options) {
        let buffer;
        if (typeof filename === "number") {
            const file = fileDescriptor(filename);
            if (!file.readable) throw new Error(`EBADF: file is not open for reading, fd ${filename}`);
            const contents = Buffer.from(__canaryoFsRead(file.path));
            buffer = contents.subarray(file.position);
            file.position = contents.length;
        } else buffer = Buffer.from(__canaryoFsRead(normalizeFsPath(filename)));
        return encodingFrom(options) ? buffer.toString(encodingFrom(options)) : buffer;
    }
    function writeFileSync(filename, value, options) {
        if (typeof filename === "number") {
            const encoding = encodingFrom(options);
            const bytes = Buffer.from(value, encoding);
            const file = fileDescriptor(filename);
            if (!file.writable) throw new Error(`EBADF: file is not open for writing, fd ${filename}`);
            writeSync(filename, bytes, 0, bytes.length, null);
            return;
        }
        __canaryoFsWrite(normalizeFsPath(filename), Buffer.from(value, encodingFrom(options)), false);
    }
    function appendFileSync(filename, value, _options) {
        if (typeof filename === "number") {
            const file = fileDescriptor(filename);
            const bytes = Buffer.from(value, encodingFrom(_options));
            writeSync(filename, bytes, 0, bytes.length, statSync(file.path).size);
            return;
        }
        __canaryoFsWrite(normalizeFsPath(filename), Buffer.from(value), true);
    }
    function statSync(filename, options = {}) {
        const path = normalizeFsPath(filename);
        if (options.throwIfNoEntry === false && !existsSync(path)) return undefined;
        return new Stats(__canaryoFsStat(path), Boolean(options.bigint));
    }
    function lstatSync(filename, options = {}) {
        const path = normalizeFsPath(filename);
        if (options.throwIfNoEntry === false && !existsSync(path)) return undefined;
        return new Stats(__canaryoFsLstat(path), Boolean(options.bigint));
    }
    function existsSync(filename) { return __canaryoFsExists(normalizeFsPath(filename)); }
    function accessSync(filename) {
        if (!existsSync(filename)) throw new Error(`ENOENT: no such file or directory, access '${filename}'`);
    }
    function mkdirSync(filename, options) {
        const recursive = options === true || Boolean(options && options.recursive);
        __canaryoFsMkdir(normalizeFsPath(filename), recursive);
    }
    function collectDirectoryEntries(parentPath, options, relativeParent = "") {
        const entries = [];
        for (const name of __canaryoFsReaddir(parentPath)) {
            const childPath = __canaryoBuiltins.path.join(parentPath, name);
            const relativePath = relativeParent ? __canaryoBuiltins.path.join(relativeParent, name) : name;
            const stats = lstatSync(childPath);
            entries.push({ name, parentPath, relativePath, stats });
            if (options.recursive && stats.isDirectory()) {
                entries.push(...collectDirectoryEntries(childPath, options, relativePath));
            }
        }
        return entries;
    }
    function readdirSync(filename, options) {
        const parentPath = normalizeFsPath(filename);
        const settings = typeof options === "string" ? { encoding: options } : options || {};
        const encoding = encodingFrom(settings);
        const entries = collectDirectoryEntries(parentPath, settings);
        if (!settings.withFileTypes) {
            const names = entries.map(entry => settings.recursive ? entry.relativePath : entry.name);
            return encoding === "buffer" ? names.map(name => Buffer.from(name)) : names;
        }
        return entries.map(entry => new Dirent(
            encoding === "buffer" ? Buffer.from(entry.name) : entry.name,
            entry.parentPath,
            entry.stats
        ));
    }
    function Dir(path, options = {}) {
        if (!(this instanceof Dir)) return new Dir(path, options);
        this.path = normalizeFsPath(path);
        this._options = typeof options === "string" ? { encoding: options } : options || {};
        this._entries = collectDirectoryEntries(this.path, this._options).map(entry => new Dirent(
            encodingFrom(this._options) === "buffer" ? Buffer.from(entry.name) : entry.name,
            entry.parentPath,
            entry.stats
        ));
        this._position = 0;
        this._closed = false;
    }
    Dir.prototype.readSync = function () {
        if (this._closed) throw new Error(`ERR_DIR_CLOSED: Directory handle was closed: ${this.path}`);
        return this._entries[this._position++] || null;
    };
    Dir.prototype.read = function (callback) {
        if (typeof callback === "function") {
            callbackOperation(callback, () => this.readSync());
            return;
        }
        return Promise.resolve().then(() => this.readSync());
    };
    Dir.prototype.closeSync = function () {
        if (this._closed) throw new Error(`ERR_DIR_CLOSED: Directory handle was closed: ${this.path}`);
        this._closed = true;
    };
    Dir.prototype.close = function (callback) {
        if (typeof callback === "function") {
            callbackOperation(callback, () => this.closeSync());
            return;
        }
        return Promise.resolve().then(() => this.closeSync());
    };
    Dir.prototype[Symbol.asyncIterator] = function () {
        const directory = this;
        return {
            async next() {
                const value = await directory.read();
                if (value !== null) return { value, done: false };
                if (!directory._closed) await directory.close();
                return { value: undefined, done: true };
            },
            async return() {
                if (!directory._closed) await directory.close();
                return { value: undefined, done: true };
            }
        };
    };
    function opendirSync(filename, options) { return new Dir(filename, options); }
    function unlinkSync(filename) { __canaryoFsUnlink(normalizeFsPath(filename)); }
    function renameSync(from, to) { __canaryoFsRename(normalizeFsPath(from), normalizeFsPath(to)); }
    function copyFileSync(from, to, _mode) { __canaryoFsCopy(normalizeFsPath(from), normalizeFsPath(to)); }
    function cpSync(source, destination, options = {}) {
        const from = normalizeFsPath(source);
        const to = normalizeFsPath(destination);
        const force = options.force !== false;
        if (typeof options.filter === "function" && !options.filter(from, to)) return;
        const metadata = options.dereference ? statSync(from) : lstatSync(from);
        if (metadata.isDirectory()) {
            if (!options.recursive) throw new Error(`ERR_FS_EISDIR: recursive option is required to copy '${from}'`);
            if (!existsSync(to)) mkdirSync(to, { recursive: true });
            for (const name of readdirSync(from)) cpSync(__canaryoBuiltins.path.join(from, name), __canaryoBuiltins.path.join(to, name), options);
            if (options.preserveTimestamps) utimesSync(to, metadata.atime, metadata.mtime);
            return;
        }
        if (metadata.isSymbolicLink() && !options.dereference) {
            if (existsSync(to)) {
                if (!force) {
                    if (options.errorOnExist) throw new Error(`EEXIST: destination already exists, copy '${to}'`);
                    return;
                }
                rmSync(to, { recursive: true, force: true });
            }
            symlinkSync(readlinkSync(from), to);
            return;
        }
        if (existsSync(to) && !force) {
            if (options.errorOnExist) throw new Error(`EEXIST: destination already exists, copy '${to}'`);
            return;
        }
        copyFileSync(from, to, options.mode);
        if (options.preserveTimestamps) utimesSync(to, metadata.atime, metadata.mtime);
    }
    function rmSync(filename, options = {}) {
        __canaryoFsRemove(normalizeFsPath(filename), Boolean(options.recursive), Boolean(options.force));
    }
    function rmdirSync(filename, options = {}) {
        __canaryoFsRemove(normalizeFsPath(filename), Boolean(options.recursive), false);
    }
    function realpathSync(filename, _options) { return __canaryoFsRealpath(normalizeFsPath(filename)); }
    realpathSync.native = realpathSync;
    function readlinkSync(filename, options) {
        const value = __canaryoFsReadlink(normalizeFsPath(filename));
        return encodingFrom(options) === "buffer" ? Buffer.from(value) : value;
    }
    function linkSync(existingPath, newPath) {
        __canaryoFsLink(normalizeFsPath(existingPath), normalizeFsPath(newPath));
    }
    function symlinkSync(target, path, type) {
        __canaryoFsSymlink(normalizeFsPath(target), normalizeFsPath(path), type === "dir" || type === "junction");
    }
    function chmodSync(filename, mode) { __canaryoFsChmod(normalizeFsPath(filename), Number(mode)); }
    function fchmodSync(fd, mode) { chmodSync(fileDescriptor(fd).path, mode); }
    function timestampSeconds(value) {
        const number = value instanceof Date ? value.getTime() / 1000 : Number(value);
        if (!Number.isFinite(number)) throw new TypeError("time must be a finite number or Date");
        return number;
    }
    function utimesSync(filename, atime, mtime) {
        __canaryoFsUtimes(normalizeFsPath(filename), timestampSeconds(atime), timestampSeconds(mtime));
    }
    function futimesSync(fd, atime, mtime) { utimesSync(fileDescriptor(fd).path, atime, mtime); }
    function mkdtempSync(prefix, options) {
        const encoding = encodingFrom(options);
        for (let attempt = 0; attempt < 100; attempt++) {
            const suffix = randomBytes(6).toString("hex").slice(0, 6);
            const path = `${normalizeFsPath(prefix)}${suffix}`;
            if (existsSync(path)) continue;
            mkdirSync(path);
            return encoding === "buffer" ? Buffer.from(path) : path;
        }
        throw new Error(`EEXIST: could not create a unique temporary directory for '${prefix}'`);
    }
    function emptyStats() {
        return new Stats({
            size: 0, file: false, directory: false, symlink: false,
            atimeMs: 0, mtimeMs: 0, ctimeMs: 0, birthtimeMs: 0,
            mode: 0, blocks: 0, blksize: 0, dev: 0, ino: 0, nlink: 0, uid: 0, gid: 0, rdev: 0
        });
    }
    function watchSnapshot(path) {
        if (!existsSync(path)) return { exists: false, stats: emptyStats(), entries: new Map() };
        const stats = lstatSync(path);
        const entries = new Map();
        if (stats.isDirectory()) {
            for (const name of readdirSync(path)) {
                const child = `${path}${path.endsWith("/") || path.endsWith("\\") ? "" : __canaryoBuiltins.path.sep}${name}`;
                try {
                    const metadata = lstatSync(child);
                    entries.set(name, `${metadata.size}:${metadata.mtimeMs}:${metadata.mode}`);
                } catch {}
            }
        }
        return { exists: true, stats, entries };
    }
    function changedStats(left, right) {
        return left.size !== right.size || left.mtimeMs !== right.mtimeMs || left.mode !== right.mode ||
            left.file !== right.file || left.directory !== right.directory || left.symlink !== right.symlink;
    }
    function FSWatcher() {
        EventEmitter.call(this);
        this._timer = null;
        this._closed = false;
    }
    util.inherits(FSWatcher, EventEmitter);
    FSWatcher.prototype.close = function () {
        if (this._closed) return;
        this._closed = true;
        if (this._timer) clearInterval(this._timer);
        this._timer = null;
        this.emit("close");
    };
    FSWatcher.prototype.ref = function () { this._timer?.ref(); return this; };
    FSWatcher.prototype.unref = function () { this._timer?.unref(); return this; };
    FSWatcher.prototype.hasRef = function () { return this._timer?.hasRef() ?? false; };
    function watch(filename, options, listener) {
        if (typeof options === "function") { listener = options; options = {}; }
        else if (typeof options === "string") options = { encoding: options };
        options ||= {};
        const path = normalizeFsPath(filename);
        if (!existsSync(path)) throw new Error(`ENOENT: no such file or directory, watch '${path}'`);
        const watcher = new FSWatcher();
        if (typeof listener === "function") watcher.on("change", listener);
        let previous = watchSnapshot(path);
        const encodeName = name => options.encoding === "buffer" ? Buffer.from(name) : name;
        watcher._timer = setInterval(() => {
            if (watcher._closed) return;
            const current = watchSnapshot(path);
            if (previous.stats.isDirectory() || current.stats.isDirectory()) {
                const names = new Set([...previous.entries.keys(), ...current.entries.keys()]);
                for (const name of names) {
                    if (!previous.entries.has(name) || !current.entries.has(name)) watcher.emit("change", "rename", encodeName(name));
                    else if (previous.entries.get(name) !== current.entries.get(name)) watcher.emit("change", "change", encodeName(name));
                }
            } else if (previous.exists !== current.exists) {
                watcher.emit("change", "rename", encodeName(__canaryoBuiltins.path.basename(path)));
            } else if (changedStats(previous.stats, current.stats)) {
                watcher.emit("change", "change", encodeName(__canaryoBuiltins.path.basename(path)));
            }
            previous = current;
        }, options.interval === undefined ? 100 : Math.max(0, Number(options.interval) || 0));
        if (options.persistent === false) watcher.unref();
        if (options.signal) addAbortListener(options.signal, () => watcher.close());
        return watcher;
    }
    const statWatchers = new Map();
    function StatWatcher(path, interval, persistent) {
        EventEmitter.call(this);
        this.path = path;
        this._closed = false;
        let previous = watchSnapshot(path).stats;
        this._timer = setInterval(() => {
            if (this._closed) return;
            const current = watchSnapshot(path).stats;
            if (changedStats(previous, current)) this.emit("change", current, previous);
            previous = current;
        }, interval);
        if (!persistent) this.unref();
    }
    util.inherits(StatWatcher, EventEmitter);
    StatWatcher.prototype.stop = StatWatcher.prototype.close = function () {
        if (this._closed) return;
        this._closed = true;
        clearInterval(this._timer);
        this._timer = null;
        statWatchers.delete(this.path);
        this.emit("stop");
    };
    StatWatcher.prototype.ref = FSWatcher.prototype.ref;
    StatWatcher.prototype.unref = FSWatcher.prototype.unref;
    StatWatcher.prototype.hasRef = FSWatcher.prototype.hasRef;
    function watchFile(filename, options, listener) {
        if (typeof options === "function") { listener = options; options = {}; }
        options ||= {};
        if (typeof listener !== "function") throw new TypeError("listener must be a function");
        const path = normalizeFsPath(filename);
        let watcher = statWatchers.get(path);
        if (!watcher) {
            watcher = new StatWatcher(path, options.interval === undefined ? 5007 : Math.max(0, Number(options.interval) || 0), options.persistent !== false);
            statWatchers.set(path, watcher);
        }
        watcher.on("change", listener);
        return watcher;
    }
    function unwatchFile(filename, listener) {
        const watcher = statWatchers.get(normalizeFsPath(filename));
        if (!watcher) return;
        if (typeof listener === "function") watcher.off("change", listener);
        else watcher.removeAllListeners("change");
        if (watcher.listenerCount("change") === 0) watcher.close();
    }
    function readSync(fd, buffer, offset = 0, length = buffer.byteLength - offset, position = null) {
        const file = fileDescriptor(fd);
        if (!file.readable) throw new Error(`EBADF: file is not open for reading, fd ${fd}`);
        if (!ArrayBuffer.isView(buffer)) throw new TypeError("buffer must be an ArrayBuffer view");
        const start = Number(offset);
        const count = Number(length);
        if (!Number.isInteger(start) || !Number.isInteger(count) || start < 0 || count < 0 || start + count > buffer.byteLength) {
            throw new RangeError("offset and length are outside the buffer");
        }
        const source = Buffer.from(__canaryoFsRead(file.path));
        const filePosition = position === null || position === undefined ? file.position : Number(position);
        if (!Number.isInteger(filePosition) || filePosition < 0) throw new RangeError("position must be a non-negative integer or null");
        const bytesRead = Math.min(count, Math.max(0, source.length - filePosition));
        const target = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
        target.set(source.subarray(filePosition, filePosition + bytesRead), start);
        if (position === null || position === undefined) file.position += bytesRead;
        return bytesRead;
    }
    function writeSync(fd, value, offsetOrPosition, lengthOrEncoding, position) {
        const file = fileDescriptor(fd);
        if (!file.writable) throw new Error(`EBADF: file is not open for writing, fd ${fd}`);
        let bytes;
        let filePosition;
        let usesCurrentPosition;
        if (typeof value === "string") {
            usesCurrentPosition = offsetOrPosition === undefined || offsetOrPosition === null;
            filePosition = usesCurrentPosition ? file.position : Number(offsetOrPosition);
            bytes = Buffer.from(value, typeof lengthOrEncoding === "string" ? lengthOrEncoding : "utf8");
        } else {
            if (!ArrayBuffer.isView(value)) throw new TypeError("buffer must be an ArrayBuffer view");
            const offset = offsetOrPosition === undefined ? 0 : Number(offsetOrPosition);
            const length = lengthOrEncoding === undefined ? value.byteLength - offset : Number(lengthOrEncoding);
            if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < 0 || length < 0 || offset + length > value.byteLength) {
                throw new RangeError("offset and length are outside the buffer");
            }
            bytes = Buffer.from(value).subarray(offset, offset + length);
            usesCurrentPosition = position === undefined || position === null;
            filePosition = usesCurrentPosition ? file.position : Number(position);
        }
        const existing = Buffer.from(__canaryoFsRead(file.path));
        if (file.append) filePosition = existing.length;
        if (!Number.isInteger(filePosition) || filePosition < 0) throw new RangeError("position must be a non-negative integer or null");
        const output = Buffer.alloc(Math.max(existing.length, filePosition + bytes.length));
        existing.copy(output);
        bytes.copy(output, filePosition);
        __canaryoFsWrite(file.path, output, false);
        if (usesCurrentPosition) file.position = filePosition + bytes.length;
        return bytes.length;
    }
    function readvSync(fd, buffers, position = null) {
        if (!Array.isArray(buffers)) throw new TypeError("buffers must be an array of ArrayBuffer views");
        let bytesRead = 0;
        let currentPosition = position;
        for (const buffer of buffers) {
            const count = readSync(fd, buffer, 0, buffer.byteLength, currentPosition);
            bytesRead += count;
            if (currentPosition !== null && currentPosition !== undefined) currentPosition = Number(currentPosition) + count;
            if (count < buffer.byteLength) break;
        }
        return bytesRead;
    }
    function writevSync(fd, buffers, position = null) {
        if (!Array.isArray(buffers)) throw new TypeError("buffers must be an array of ArrayBuffer views");
        let bytesWritten = 0;
        let currentPosition = position;
        for (const buffer of buffers) {
            if (!ArrayBuffer.isView(buffer)) throw new TypeError("buffers must contain only ArrayBuffer views");
            const count = writeSync(fd, buffer, 0, buffer.byteLength, currentPosition);
            bytesWritten += count;
            if (currentPosition !== null && currentPosition !== undefined) currentPosition = Number(currentPosition) + count;
        }
        return bytesWritten;
    }
    function fstatSync(fd, options) { return statSync(fileDescriptor(fd).path, options); }
    function ftruncateSync(fd, length = 0) {
        const file = fileDescriptor(fd);
        if (!file.writable) throw new Error(`EINVAL: file is not open for writing, fd ${fd}`);
        const size = Number(length);
        if (!Number.isInteger(size) || size < 0) throw new RangeError("length must be a non-negative integer");
        const current = Buffer.from(__canaryoFsRead(file.path));
        const output = Buffer.alloc(size);
        current.copy(output, 0, 0, Math.min(current.length, size));
        __canaryoFsWrite(file.path, output, false);
    }
    function fsyncSync(fd) { fileDescriptor(fd); }
    const fdatasyncSync = fsyncSync;
    function truncateSync(filename, length = 0) {
        if (typeof filename === "number") return ftruncateSync(filename, length);
        const fd = openSync(filename, "r+");
        try { return ftruncateSync(fd, length); }
        finally { closeSync(fd); }
    }
    function callbackOperation(callback, operation) {
        queueMicrotask(() => {
            try { callback(null, operation()); }
            catch (error) { callback(error); }
        });
    }
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
    class FileHandle {
        constructor(fd) { this.fd = fd; }
        get [Symbol.toStringTag]() { return "FileHandle"; }
        appendFile(value, options) { return Promise.resolve().then(() => appendFileSync(this.fd, value, options)); }
        chmod(mode) { return Promise.resolve().then(() => fchmodSync(this.fd, mode)); }
        chown(_uid, _gid) { return Promise.resolve(); }
        close() { return Promise.resolve().then(() => { closeSync(this.fd); this.fd = -1; }); }
        datasync() { return Promise.resolve().then(() => fdatasyncSync(this.fd)); }
        read(buffer, offset, length, position) {
            return Promise.resolve().then(() => ({ bytesRead: readSync(this.fd, buffer, offset, length, position), buffer }));
        }
        readv(buffers, position) {
            return Promise.resolve().then(() => ({ bytesRead: readvSync(this.fd, buffers, position), buffers }));
        }
        readFile(options) { return Promise.resolve().then(() => readFileSync(this.fd, options)); }
        stat(options) { return Promise.resolve().then(() => fstatSync(this.fd, options)); }
        sync() { return Promise.resolve().then(() => fsyncSync(this.fd)); }
        truncate(length) { return Promise.resolve().then(() => ftruncateSync(this.fd, length)); }
        utimes(atime, mtime) { return Promise.resolve().then(() => futimesSync(this.fd, atime, mtime)); }
        write(value, offsetOrPosition, lengthOrEncoding, position) {
            return Promise.resolve().then(() => ({
                bytesWritten: writeSync(this.fd, value, offsetOrPosition, lengthOrEncoding, position),
                buffer: value
            }));
        }
        writev(buffers, position) {
            return Promise.resolve().then(() => ({ bytesWritten: writevSync(this.fd, buffers, position), buffers }));
        }
        writeFile(value, options) { return Promise.resolve().then(() => writeFileSync(this.fd, value, options)); }
    }
    const fsConstants = {
        F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1,
        O_RDONLY: 0, O_WRONLY: 1, O_RDWR: 2,
        O_CREAT: 0x40, O_EXCL: 0x80, O_TRUNC: 0x200, O_APPEND: 0x400
    };
    const fsPromises = {
        constants: fsConstants,
        open(filename, flags, mode) { return Promise.resolve().then(() => new FileHandle(openSync(filename, flags, mode))); },
        readFile(filename, options) { return Promise.resolve().then(() => readFileSync(filename, options)); },
        writeFile(filename, value, options) { return Promise.resolve().then(() => writeFileSync(filename, value, options)); },
        appendFile(filename, value, options) { return Promise.resolve().then(() => appendFileSync(filename, value, options)); },
        stat(filename, options) { return Promise.resolve().then(() => statSync(filename, options)); },
        lstat(filename, options) { return Promise.resolve().then(() => lstatSync(filename, options)); },
        access(filename) { return Promise.resolve().then(() => accessSync(filename)); },
        mkdir(filename, options) { return Promise.resolve().then(() => mkdirSync(filename, options)); },
        readdir(filename, options) { return Promise.resolve().then(() => readdirSync(filename, options)); },
        opendir(filename, options) { return Promise.resolve().then(() => opendirSync(filename, options)); },
        unlink(filename) { return Promise.resolve().then(() => unlinkSync(filename)); },
        rename(from, to) { return Promise.resolve().then(() => renameSync(from, to)); },
        copyFile(from, to, mode) { return Promise.resolve().then(() => copyFileSync(from, to, mode)); },
        cp(from, to, options) { return Promise.resolve().then(() => cpSync(from, to, options)); },
        rm(filename, options) { return Promise.resolve().then(() => rmSync(filename, options)); },
        rmdir(filename, options) { return Promise.resolve().then(() => rmdirSync(filename, options)); },
        realpath(filename, options) { return Promise.resolve().then(() => realpathSync(filename, options)); },
        readlink(filename, options) { return Promise.resolve().then(() => readlinkSync(filename, options)); },
        link(existingPath, newPath) { return Promise.resolve().then(() => linkSync(existingPath, newPath)); },
        symlink(target, path, type) { return Promise.resolve().then(() => symlinkSync(target, path, type)); },
        chmod(filename, mode) { return Promise.resolve().then(() => chmodSync(filename, mode)); },
        utimes(filename, atime, mtime) { return Promise.resolve().then(() => utimesSync(filename, atime, mtime)); },
        mkdtemp(prefix, options) { return Promise.resolve().then(() => mkdtempSync(prefix, options)); },
        truncate(filename, length) { return Promise.resolve().then(() => truncateSync(filename, length)); }
    };
    const fsModule = {
        Stats,
        Dirent,
        Dir,
        constants: fsConstants,
        ReadStream,
        WriteStream,
        promises: fsPromises,
        readFileSync,
        writeFileSync,
        appendFileSync,
        openSync,
        closeSync,
        readSync,
        readvSync,
        writeSync,
        writevSync,
        fstatSync,
        ftruncateSync,
        fsyncSync,
        fdatasyncSync,
        truncateSync,
        statSync,
        lstatSync,
        existsSync,
        accessSync,
        mkdirSync,
        readdirSync,
        opendirSync,
        unlinkSync,
        renameSync,
        copyFileSync,
        cpSync,
        rmSync,
        rmdirSync,
        realpathSync,
        readlinkSync,
        linkSync,
        symlinkSync,
        chmodSync,
        fchmodSync,
        utimesSync,
        futimesSync,
        mkdtempSync,
        watch,
        watchFile,
        unwatchFile,
        FSWatcher,
        StatWatcher,
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
        open(filename, flags, mode, callback) {
            if (typeof flags === "function") { callback = flags; flags = "r"; mode = undefined; }
            if (typeof mode === "function") { callback = mode; mode = undefined; }
            callbackOperation(callback, () => openSync(filename, flags, mode));
        },
        close(fd, callback) { callbackOperation(callback, () => closeSync(fd)); },
        read(fd, buffer, offset, length, position, callback) {
            queueMicrotask(() => {
                try { callback(null, readSync(fd, buffer, offset, length, position), buffer); }
                catch (error) { callback(error, 0, buffer); }
            });
        },
        readv(fd, buffers, position, callback) {
            if (typeof position === "function") { callback = position; position = null; }
            queueMicrotask(() => {
                try { callback(null, readvSync(fd, buffers, position), buffers); }
                catch (error) { callback(error, 0, buffers); }
            });
        },
        write(fd, value, offsetOrPosition, lengthOrEncoding, position, callback) {
            if (typeof offsetOrPosition === "function") {
                callback = offsetOrPosition; offsetOrPosition = undefined; lengthOrEncoding = undefined; position = undefined;
            } else if (typeof lengthOrEncoding === "function") {
                callback = lengthOrEncoding; lengthOrEncoding = undefined; position = undefined;
            } else if (typeof position === "function") { callback = position; position = undefined; }
            queueMicrotask(() => {
                try { callback(null, writeSync(fd, value, offsetOrPosition, lengthOrEncoding, position), value); }
                catch (error) { callback(error, 0, value); }
            });
        },
        writev(fd, buffers, position, callback) {
            if (typeof position === "function") { callback = position; position = null; }
            queueMicrotask(() => {
                try { callback(null, writevSync(fd, buffers, position), buffers); }
                catch (error) { callback(error, 0, buffers); }
            });
        },
        fstat(fd, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => fstatSync(fd, options));
        },
        ftruncate(fd, length, callback) {
            if (typeof length === "function") { callback = length; length = 0; }
            callbackOperation(callback, () => ftruncateSync(fd, length));
        },
        fsync(fd, callback) { callbackOperation(callback, () => fsyncSync(fd)); },
        fdatasync(fd, callback) { callbackOperation(callback, () => fdatasyncSync(fd)); },
        stat(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => statSync(filename, options));
        },
        lstat(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => lstatSync(filename, options));
        },
        access(filename, mode, callback) {
            if (typeof mode === "function") { callback = mode; mode = 0; }
            callbackOperation(callback, () => accessSync(filename, mode));
        },
        exists(filename, callback) { queueMicrotask(() => callback(existsSync(filename))); },
        mkdir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => mkdirSync(filename, options));
        },
        readdir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => readdirSync(filename, options));
        },
        opendir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => opendirSync(filename, options));
        },
        unlink(filename, callback) { callbackOperation(callback, () => unlinkSync(filename)); },
        rename(from, to, callback) { callbackOperation(callback, () => renameSync(from, to)); },
        copyFile(from, to, mode, callback) {
            if (typeof mode === "function") { callback = mode; mode = 0; }
            callbackOperation(callback, () => copyFileSync(from, to, mode));
        },
        cp(from, to, options, callback) {
            if (typeof options === "function") { callback = options; options = {}; }
            callbackOperation(callback, () => cpSync(from, to, options));
        },
        truncate(filename, length, callback) {
            if (typeof length === "function") { callback = length; length = 0; }
            callbackOperation(callback, () => truncateSync(filename, length));
        },
        rm(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => rmSync(filename, options));
        },
        rmdir(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => rmdirSync(filename, options));
        },
        realpath(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => realpathSync(filename, options));
        },
        readlink(filename, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => readlinkSync(filename, options));
        },
        link(existingPath, newPath, callback) { callbackOperation(callback, () => linkSync(existingPath, newPath)); },
        symlink(target, path, type, callback) {
            if (typeof type === "function") { callback = type; type = undefined; }
            callbackOperation(callback, () => symlinkSync(target, path, type));
        },
        chmod(filename, mode, callback) { callbackOperation(callback, () => chmodSync(filename, mode)); },
        fchmod(fd, mode, callback) { callbackOperation(callback, () => fchmodSync(fd, mode)); },
        utimes(filename, atime, mtime, callback) { callbackOperation(callback, () => utimesSync(filename, atime, mtime)); },
        futimes(fd, atime, mtime, callback) { callbackOperation(callback, () => futimesSync(fd, atime, mtime)); },
        mkdtemp(prefix, options, callback) {
            if (typeof options === "function") { callback = options; options = undefined; }
            callbackOperation(callback, () => mkdtempSync(prefix, options));
        },
        createReadStream(filename, options) { return new ReadStream(filename, options); },
        createWriteStream(filename, options) { return new WriteStream(filename, options); }
    };
    fsModule.realpath.native = fsModule.realpath;

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
    const untransferableObjects = new WeakSet();
    const uncloneableObjects = new WeakSet();

    function structuredClone(value, options = {}) {
        const transfer = options && options.transfer === undefined ? [] : options.transfer;
        if (!Array.isArray(transfer)) throw new TypeError("transfer must be an Array");
        const transferSet = new Set();
        for (const item of transfer) {
            if ((typeof item !== "object" && typeof item !== "function") || item === null) {
                throw new DOMException("Value is not transferable", "DataCloneError");
            }
            if (transferSet.has(item)) throw new DOMException("Transfer list contains duplicate values", "DataCloneError");
            if (untransferableObjects.has(item)) throw new DOMException("Value is marked as untransferable", "DataCloneError");
            transferSet.add(item);
        }
        const seen = new Map();
        function clone(item) {
            if (item === null || typeof item === "undefined" || typeof item === "string" ||
                typeof item === "number" || typeof item === "boolean" || typeof item === "bigint") return item;
            if (typeof item === "symbol" || typeof item === "function") {
                throw new DOMException("Value could not be cloned", "DataCloneError");
            }
            if (uncloneableObjects.has(item)) throw new DOMException("Value is marked as uncloneable", "DataCloneError");
            if (seen.has(item)) return seen.get(item);
            if (item instanceof ArrayBuffer) {
                const output = item.slice(0);
                seen.set(item, output);
                return output;
            }
            if (ArrayBuffer.isView(item)) {
                const buffer = clone(item.buffer);
                const output = item instanceof DataView
                    ? new DataView(buffer, item.byteOffset, item.byteLength)
                    : new item.constructor(buffer, item.byteOffset, item.length);
                seen.set(item, output);
                return output;
            }
            if (item instanceof Date) return new Date(item.getTime());
            if (item instanceof RegExp) return new RegExp(item.source, item.flags);
            if (item instanceof Blob) {
                const output = item instanceof File
                    ? new File([item], item.name, { type: item.type, lastModified: item.lastModified })
                    : new Blob([item], { type: item.type });
                seen.set(item, output);
                return output;
            }
            if (item instanceof Map) {
                const output = new Map();
                seen.set(item, output);
                for (const [key, mapValue] of item) output.set(clone(key), clone(mapValue));
                return output;
            }
            if (item instanceof Set) {
                const output = new Set();
                seen.set(item, output);
                for (const setValue of item) output.add(clone(setValue));
                return output;
            }
            if (item instanceof Error) {
                const output = new Error(item.message);
                seen.set(item, output);
                output.name = item.name;
                output.stack = item.stack;
                if ("cause" in item) output.cause = clone(item.cause);
                return output;
            }
            const output = Array.isArray(item) ? [] : {};
            seen.set(item, output);
            for (const key of Object.keys(item)) output[key] = clone(item[key]);
            return output;
        }
        return clone(value);
    }

    globalThis.structuredClone = structuredClone;
    function MessagePort() {
        EventEmitter.call(this);
        this._peer = null;
        this._messageQueue = [];
        this._closed = false;
        this.onmessage = null;
        this.onmessageerror = null;
        this._eventListeners = new Map();
    }
    util.inherits(MessagePort, EventEmitter);
    MessagePort.prototype.postMessage = function (value) {
        if (this._closed || !this._peer || this._peer._closed) return;
        const peer = this._peer;
        let message;
        try { message = structuredClone(value); }
        catch (error) {
            if (typeof this.onmessageerror === "function") this.onmessageerror(new MessageEvent("messageerror", { data: value }));
            throw error;
        }
        peer._messageQueue.push(message);
        setImmediate(() => {
            const index = peer._messageQueue.indexOf(message);
            if (index !== -1) peer._messageQueue.splice(index, 1);
            if (!peer._closed) {
                peer.emit("message", message);
                if (typeof peer.onmessage === "function") peer.onmessage(new MessageEvent("message", { data: message, ports: [] }));
            }
        });
    };
    MessagePort.prototype.addEventListener = function (type, callback) {
        if (typeof callback !== "function" && typeof callback?.handleEvent !== "function") return;
        const listener = value => {
            const event = value instanceof MessageEvent ? value : new MessageEvent(type, { data: value });
            if (typeof callback === "function") callback.call(this, event);
            else callback.handleEvent(event);
        };
        const listeners = this._eventListeners.get(callback) || [];
        listeners.push({ type: String(type), listener });
        this._eventListeners.set(callback, listeners);
        this.on(String(type), listener);
    };
    MessagePort.prototype.removeEventListener = function (type, callback) {
        const listeners = this._eventListeners.get(callback) || [];
        for (const entry of listeners.filter(entry => entry.type === String(type))) this.off(entry.type, entry.listener);
        const remaining = listeners.filter(entry => entry.type !== String(type));
        if (remaining.length) this._eventListeners.set(callback, remaining);
        else this._eventListeners.delete(callback);
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
    const broadcastChannels = new Map();
    function BroadcastChannel(name) {
        EventTarget.call(this);
        this.name = String(name);
        this.onmessage = null;
        this.onmessageerror = null;
        this._closed = false;
        const channels = broadcastChannels.get(this.name) || new Set();
        channels.add(this);
        broadcastChannels.set(this.name, channels);
    }
    BroadcastChannel.prototype = Object.create(EventTarget.prototype, {
        constructor: { value: BroadcastChannel, writable: true, configurable: true }
    });
    BroadcastChannel.prototype.postMessage = function (value) {
        if (this._closed) throw new Error("BroadcastChannel is closed");
        const message = structuredClone(value);
        for (const channel of broadcastChannels.get(this.name) || []) {
            if (channel === this || channel._closed) continue;
            const cloned = structuredClone(message);
            setImmediate(() => {
                if (!channel._closed) channel.dispatchEvent(new MessageEvent("message", { data: cloned }));
            });
        }
    };
    BroadcastChannel.prototype.close = function () {
        if (this._closed) return;
        this._closed = true;
        const channels = broadcastChannels.get(this.name);
        channels?.delete(this);
        if (!channels?.size) broadcastChannels.delete(this.name);
    };
    BroadcastChannel.prototype.ref = function () { return this; };
    BroadcastChannel.prototype.unref = function () { return this; };
    BroadcastChannel.prototype.hasRef = function () { return false; };
    globalThis.BroadcastChannel = BroadcastChannel;
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
        BroadcastChannel,
        getEnvironmentData(key) { return workerEnvironment.get(key); },
        setEnvironmentData(key, value) { workerEnvironment.set(key, value); },
        receiveMessageOnPort(port) {
            if (!(port instanceof MessagePort)) throw new TypeError("port must be a MessagePort");
            return port._messageQueue.length ? { message: port._messageQueue.shift() } : undefined;
        },
        markAsUntransferable(value) { untransferableObjects.add(value); },
        markAsUncloneable(value) { uncloneableObjects.add(value); },
        isMarkedAsUntransferable: value => untransferableObjects.has(value),
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
            if (this._canaryoServerSocket) this.__canaryoServerOutgoing.push(body);
            else __canaryoNetWrite(this._canaryoNetId, body);
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
            const event = __canaryoNetPoll();
            if (event === undefined || event === null) break;
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
                socket.__canaryoNetReceiveBytes(Buffer.from(event.body));
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
        "_stream_duplex", "_stream_passthrough", "_stream_readable", "_stream_transform", "_stream_writable",
        "assert", "assert/strict", "async_hooks", "buffer", "console", "constants", "crypto", "diagnostics_channel", "dns",
        "dns/promises", "events", "fs", "fs/promises", "http", "https", "module", "net", "os",
        "path", "path/posix", "path/win32", "perf_hooks", "process", "querystring", "stream", "stream/consumers", "stream/promises", "stream/web", "string_decoder",
        "timers", "timers/promises", "tty",
        "sys", "url", "util", "util/types", "worker_threads", "zlib"
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
    function cryptoOutput(bytes, encoding) {
        const output = Buffer.from(bytes);
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
                const contents = Buffer.concat(chunks);
                const result = key === undefined
                    ? __canaryoHash(algorithm, contents)
                    : __canaryoHmac(algorithm, key, contents);
                return cryptoOutput(result, encoding);
            }
        };
    }
    function randomBytes(size, callback) {
        const length = Number(size);
        if (!Number.isInteger(length) || length < 0) throw new RangeError("size must be a non-negative integer");
        const output = Buffer.from(__canaryoRandomBytes(length));
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
        bytes.set(__canaryoRandomBytes(length), start);
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
    function pbkdf2Sync(password, salt, iterations, keyLength, digest) {
        const rounds = Number(iterations);
        const length = Number(keyLength);
        if (!Number.isInteger(rounds) || rounds <= 0 || rounds > 0x7fffffff) {
            throw new RangeError("iterations must be a positive 32-bit integer");
        }
        if (!Number.isInteger(length) || length < 0 || length > 0x7fffffff) {
            throw new RangeError("keylen must be a non-negative 32-bit integer");
        }
        if (digest === undefined) throw new TypeError("digest is required");
        return Buffer.from(__canaryoPbkdf2(
            cryptoInput(password),
            cryptoInput(salt),
            rounds,
            length,
            String(digest)
        ));
    }
    function pbkdf2(password, salt, iterations, keyLength, digest, callback) {
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        process.nextTick(() => {
            try { callback(null, pbkdf2Sync(password, salt, iterations, keyLength, digest)); }
            catch (error) { callback(error); }
        });
    }
    function hkdfSync(digest, key, salt, info, keyLength) {
        const length = Number(keyLength);
        if (!Number.isInteger(length) || length < 0 || length > 0x7fffffff) {
            throw new RangeError("keylen must be a non-negative 32-bit integer");
        }
        return cryptoArrayBuffer(__canaryoHkdf(
            String(digest),
            cryptoInput(key),
            cryptoInput(salt),
            cryptoInput(info),
            length
        ));
    }
    function hkdf(digest, key, salt, info, keyLength, callback) {
        if (typeof callback !== "function") throw new TypeError("callback must be a function");
        process.nextTick(() => {
            try { callback(null, hkdfSync(digest, key, salt, info, keyLength)); }
            catch (error) { callback(error); }
        });
    }
    function randomUUID() {
        const bytes = randomBytes(16);
        bytes[6] = bytes[6] & 0x0f | 0x40;
        bytes[8] = bytes[8] & 0x3f | 0x80;
        const hex = bytes.toString("hex");
        return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    }
    function randomInt(min, max, callback) {
        if (typeof max === "function") { callback = max; max = min; min = 0; }
        else if (max === undefined) { max = min; min = 0; }
        const lower = Number(min);
        const upper = Number(max);
        const range = upper - lower;
        if (!Number.isSafeInteger(lower) || !Number.isSafeInteger(upper) || range <= 0 || range > 281474976710656) {
            throw new RangeError("min and max must define a safe positive range no larger than 2^48");
        }
        const limit = Math.floor(281474976710656 / range) * range;
        let sample;
        do {
            const bytes = randomBytes(6);
            sample = bytes[0] * 1099511627776 + bytes[1] * 4294967296 + bytes[2] * 16777216 +
                bytes[3] * 65536 + bytes[4] * 256 + bytes[5];
        } while (sample >= limit);
        const result = lower + sample % range;
        if (typeof callback === "function") { process.nextTick(callback, null, result); return undefined; }
        return result;
    }
    function randomFloat() {
        const bytes = randomBytes(7);
        const high = (bytes[0] & 0x1f) * 281474976710656 + bytes[1] * 1099511627776 +
            bytes[2] * 4294967296 + bytes[3] * 16777216 + bytes[4] * 65536 + bytes[5] * 256 + bytes[6];
        return high / 9007199254740992;
    }
    function webCryptoBytes(value) {
        if (value instanceof ArrayBuffer) return new Uint8Array(value);
        if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
        throw new TypeError("value must be a BufferSource");
    }
    function webCryptoAlgorithm(value) {
        const name = typeof value === "string" ? value : value?.name;
        if (!name) throw new TypeError("algorithm name is required");
        return String(name).toUpperCase().replace(/_/g, "-");
    }
    function cryptoArrayBuffer(value) {
        const bytes = Buffer.from(value);
        return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    }
    const cryptoKeyToken = {};
    class CryptoKey {
        constructor(token, type, algorithm, extractable, usages, bytes) {
            if (token !== cryptoKeyToken) throw new TypeError("Illegal constructor");
            this.type = type;
            this.algorithm = Object.freeze(algorithm);
            this.extractable = Boolean(extractable);
            this.usages = Object.freeze([...usages]);
            this._bytes = Buffer.from(bytes);
        }
        get [Symbol.toStringTag]() { return "CryptoKey"; }
    }
    const subtleCryptoToken = {};
    class SubtleCrypto {
        constructor(token) { if (token !== subtleCryptoToken) throw new TypeError("Illegal constructor"); }
        async digest(algorithm, data) {
            const name = webCryptoAlgorithm(algorithm);
            return cryptoArrayBuffer(__canaryoHash(name, webCryptoBytes(data)));
        }
        async importKey(format, keyData, algorithm, extractable, keyUsages) {
            if (String(format) !== "raw") throw new TypeError("only raw keys are supported");
            const name = webCryptoAlgorithm(algorithm);
            if (name === "PBKDF2" || name === "HKDF") {
                if (extractable) throw new DOMException(`${name} keys cannot be extractable`, "SyntaxError");
                return new CryptoKey(cryptoKeyToken, "secret", { name }, false, keyUsages, webCryptoBytes(keyData));
            }
            if (name !== "HMAC") throw new TypeError("only HMAC and PBKDF2 keys are supported");
            const hash = webCryptoAlgorithm(algorithm.hash);
            return new CryptoKey(cryptoKeyToken, "secret", { name: "HMAC", hash: { name: hash }, length: webCryptoBytes(keyData).byteLength * 8 }, extractable, keyUsages, webCryptoBytes(keyData));
        }
        async exportKey(format, key) {
            if (String(format) !== "raw" || !(key instanceof CryptoKey)) throw new TypeError("only raw CryptoKey export is supported");
            if (!key.extractable) throw new DOMException("key is not extractable", "InvalidAccessError");
            return cryptoArrayBuffer(key._bytes);
        }
        async generateKey(algorithm, extractable, keyUsages) {
            if (webCryptoAlgorithm(algorithm) !== "HMAC") throw new TypeError("only HMAC keys are supported");
            const hash = webCryptoAlgorithm(algorithm.hash);
            const length = algorithm.length === undefined ? 256 : Number(algorithm.length);
            if (!Number.isInteger(length) || length <= 0 || length % 8 !== 0) throw new RangeError("HMAC key length must be a positive multiple of 8");
            return new CryptoKey(cryptoKeyToken, "secret", { name: "HMAC", hash: { name: hash }, length }, extractable, keyUsages, randomBytes(length / 8));
        }
        async deriveBits(algorithm, baseKey, length) {
            const name = webCryptoAlgorithm(algorithm);
            if (!(baseKey instanceof CryptoKey) || baseKey.algorithm.name !== name || (name !== "PBKDF2" && name !== "HKDF")) {
                throw new DOMException("key and derivation algorithm do not match", "InvalidAccessError");
            }
            const bitLength = Number(length);
            if (!Number.isInteger(bitLength) || bitLength <= 0 || bitLength % 8 !== 0) {
                throw new DOMException("derived bit length must be a positive multiple of 8", "OperationError");
            }
            if (name === "PBKDF2") {
                return cryptoArrayBuffer(__canaryoPbkdf2(
                    baseKey._bytes,
                    webCryptoBytes(algorithm.salt),
                    Number(algorithm.iterations),
                    bitLength / 8,
                    webCryptoAlgorithm(algorithm.hash)
                ));
            }
            return cryptoArrayBuffer(__canaryoHkdf(
                webCryptoAlgorithm(algorithm.hash),
                baseKey._bytes,
                webCryptoBytes(algorithm.salt),
                webCryptoBytes(algorithm.info),
                bitLength / 8
            ));
        }
        async deriveKey(algorithm, baseKey, derivedKeyAlgorithm, extractable, keyUsages) {
            if (webCryptoAlgorithm(derivedKeyAlgorithm) !== "HMAC") {
                throw new TypeError("only HMAC derived keys are supported");
            }
            const hash = webCryptoAlgorithm(derivedKeyAlgorithm.hash);
            const defaultLength = hash === "SHA-384" || hash === "SHA-512" ? 1024 : 512;
            const length = derivedKeyAlgorithm.length === undefined ? defaultLength : Number(derivedKeyAlgorithm.length);
            const bytes = new Uint8Array(await this.deriveBits(algorithm, baseKey, length));
            return new CryptoKey(cryptoKeyToken, "secret", { name: "HMAC", hash: { name: hash }, length }, extractable, keyUsages, bytes);
        }
        async sign(algorithm, key, data) {
            if (webCryptoAlgorithm(algorithm) !== "HMAC" || !(key instanceof CryptoKey) || key.algorithm.name !== "HMAC") {
                throw new DOMException("key and algorithm must use HMAC", "InvalidAccessError");
            }
            return cryptoArrayBuffer(__canaryoHmac(key.algorithm.hash.name, key._bytes, webCryptoBytes(data)));
        }
        async verify(algorithm, key, signature, data) {
            const expected = new Uint8Array(await this.sign(algorithm, key, data));
            const actual = webCryptoBytes(signature);
            return expected.byteLength === actual.byteLength && __canaryoTimingSafeEqual(expected, actual);
        }
    }
    const cryptoToken = {};
    class Crypto {
        constructor(token) {
            if (token !== cryptoToken) throw new TypeError("Illegal constructor");
            this.subtle = new SubtleCrypto(subtleCryptoToken);
        }
        getRandomValues(view) {
            if (!ArrayBuffer.isView(view)) throw new TypeError("value must be an integer ArrayBuffer view");
            if (view.byteLength > 65536) throw new RangeError("requested too many random bytes");
            randomFillSync(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
            return view;
        }
        randomUUID() { return randomUUID(); }
        get [Symbol.toStringTag]() { return "Crypto"; }
    }
    const webcrypto = new Crypto(cryptoToken);
    const cryptoModule = {
        createHash(algorithm) { return createDigest(algorithm); },
        createHmac(algorithm, key, encoding) { return createDigest(algorithm, cryptoInput(key, encoding)); },
        hash(algorithm, data, outputEncoding) { return cryptoOutput(__canaryoHash(algorithm, cryptoInput(data)), outputEncoding); },
        randomBytes,
        randomFill,
        randomFillSync,
        randomInt,
        randomFloat,
        randomUUID,
        pbkdf2,
        pbkdf2Sync,
        hkdf,
        hkdfSync,
        timingSafeEqual(left, right) {
            return __canaryoTimingSafeEqual(cryptoInput(left), cryptoInput(right));
        },
        getHashes() { return ["sha1", "sha256", "sha384", "sha512"]; },
        webcrypto,
        subtle: webcrypto.subtle,
        Crypto,
        CryptoKey,
        SubtleCrypto,
        constants: {}
    };
    Object.assign(globalThis, { crypto: webcrypto, Crypto, CryptoKey, SubtleCrypto });

    function zlibSync(operation, input) {
        return Buffer.from(__canaryoZlibTransform(operation, Buffer.from(input)));
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

    function webCompressionOperation(format, decompress) {
        const normalized = String(format);
        if (normalized === "gzip") return decompress ? "gunzip" : "gzip";
        if (normalized === "deflate") return decompress ? "inflate" : "deflate";
        if (normalized === "deflate-raw") return decompress ? "inflateRaw" : "deflateRaw";
        throw new TypeError(`Unsupported compression format: ${format}`);
    }

    function createWebCompressionStream(format, decompress) {
        const operation = webCompressionOperation(format, decompress);
        const chunks = [];
        return new TransformStream({
            transform(chunk) {
                if (!ArrayBuffer.isView(chunk) && !(chunk instanceof ArrayBuffer)) {
                    throw new TypeError("Compression stream chunks must be BufferSource values");
                }
                chunks.push(Buffer.from(chunk));
            },
            flush(controller) {
                controller.enqueue(new Uint8Array(zlibSync(operation, Buffer.concat(chunks))));
            }
        });
    }

    class CompressionStream {
        constructor(format) {
            const stream = createWebCompressionStream(format, false);
            this.readable = stream.readable;
            this.writable = stream.writable;
        }
        get [Symbol.toStringTag]() { return "CompressionStream"; }
    }

    class DecompressionStream {
        constructor(format) {
            const stream = createWebCompressionStream(format, true);
            this.readable = stream.readable;
            this.writable = stream.writable;
        }
        get [Symbol.toStringTag]() { return "DecompressionStream"; }
    }

    Object.assign(streamWeb, { CompressionStream, DecompressionStream });
    Object.assign(globalThis, { CompressionStream, DecompressionStream });

    globalThis.__canaryoBuiltins = Object.freeze({
        "_stream_duplex": Duplex,
        "_stream_passthrough": PassThrough,
        "_stream_readable": Readable,
        "_stream_transform": Transform,
        "_stream_writable": Writable,
        assert,
        "assert/strict": strictAssert,
        async_hooks: {
            AsyncLocalStorage,
            AsyncResource,
            createHook,
            executionAsyncId,
            triggerAsyncId,
            executionAsyncResource
        },
        buffer: {
            Buffer,
            Blob,
            File,
            SlowBuffer: Buffer,
            atob,
            btoa,
            INSPECT_MAX_BYTES: 50,
            kMaxLength: 0x7fffffff,
            constants: { MAX_LENGTH: 0x7fffffff, MAX_STRING_LENGTH: 0x1fffffe8 }
        },
        console: globalThis.console,
        constants: fsConstants,
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
        "path/posix": posixPath,
        "path/win32": win32Path,
        perf_hooks: perfHooksModule,
        process,
        querystring: querystringModule,
        stream: Stream,
        "stream/consumers": streamConsumers,
        "stream/promises": streamPromises,
        "stream/web": streamWeb,
        string_decoder: { StringDecoder },
        sys: util,
        timers: { setImmediate, clearImmediate: clearTimer, setTimeout, clearTimeout: clearTimer, setInterval, clearInterval: clearTimer },
        "timers/promises": timersPromises,
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
        "util/types": utilTypes,
        worker_threads: workerThreads,
        zlib: zlibModule
    });
})();
