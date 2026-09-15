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

