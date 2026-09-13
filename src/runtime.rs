use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    io::{self, Read, Write},
    net::{IpAddr, Shutdown, TcpStream, ToSocketAddrs},
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
    thread,
    time::Duration,
};

use base64::Engine;
use ring::rand::SecureRandom;
use rquickjs::{Array, CatchResultExt, Context, Function, Module, Object, Promise, Runtime};
use subtle::ConstantTimeEq;

use crate::{esm, http, modules};

const BOOTSTRAP: &str = r#"
globalThis.global = globalThis;
(() => {
function Console(stdout, stderr = stdout) {
    if (stdout && stdout.stdout) {
        stderr = stdout.stderr || stdout.stdout;
        stdout = stdout.stdout;
    }
    this._stdout = stdout;
    this._stderr = stderr;
}
Console.prototype.log = Console.prototype.info = Console.prototype.debug = function (...values) {
    this._stdout.write(values.map(formatValue).join(" ") + "\n");
};
Console.prototype.error = Console.prototype.warn = function (...values) {
    this._stderr.write(values.map(formatValue).join(" ") + "\n");
};

globalThis.console = Object.freeze({
    log(...values) {
        __canaryoPrint(values.map(formatValue).join(" "));
    },
    info(...values) {
        __canaryoPrint(values.map(formatValue).join(" "));
    },
    debug(...values) {
        __canaryoPrint(values.map(formatValue).join(" "));
    },
    error(...values) {
        __canaryoPrintError(values.map(formatValue).join(" "));
    },
    warn(...values) {
        __canaryoPrintError(values.map(formatValue).join(" "));
    },
    Console
});

function formatValue(value) {
    if (typeof value === "string") return value;
    if (typeof value !== "object" || value === null) return String(value);
    try {
        return JSON.stringify(value);
    } catch {
        return String(value);
    }
}

function IncomingMessage() {}
function ServerResponse() {}
function Socket() {}

IncomingMessage.prototype.setEncoding = function(encoding) {
    this.__canaryoEncoding = String(encoding);
    return this;
};
IncomingMessage.prototype.pause = function() {
    this.__canaryoPaused = true;
    return this;
};
IncomingMessage.prototype.resume = function() {
    this.__canaryoPaused = false;
    const queue = this.__canaryoChunkQueue || [];
    while (!this.__canaryoPaused && queue.length > 0) {
        this.__canaryoDeliverChunk(queue.shift());
    }
    if (!this.__canaryoPaused && this.__canaryoEndPending) this.__canaryoFinishBody();
    return this;
};
IncomingMessage.prototype.__canaryoDeliverChunk = function(buffer) {
    if (this.readableEnded) return;
    if (this.__canaryoPaused) {
        (this.__canaryoChunkQueue || (this.__canaryoChunkQueue = [])).push(buffer);
        return;
    }
    const chunk = this.__canaryoEncoding ? buffer.toString(this.__canaryoEncoding) : buffer;
    this.emit("data", chunk);
};
IncomingMessage.prototype.__canaryoFinishBody = function() {
    if (this.readableEnded) return;
    if (this.__canaryoPaused || (this.__canaryoChunkQueue && this.__canaryoChunkQueue.length)) {
        this.__canaryoEndPending = true;
        return;
    }
    this.__canaryoEndPending = false;
    this.readable = false;
    this.readableEnded = true;
    this.complete = true;
    this.emit("end");
    this.emit("close");
};
IncomingMessage.prototype.__canaryoDeliverBytes = function(bytes) {
    this.__canaryoDeliverChunk(Buffer.from(bytes));
};

Socket.prototype.setTimeout = function(value, callback) {
    this.timeout = Number(value);
    if (typeof callback === "function") this.on("timeout", callback);
    return this;
};
Socket.prototype.ref = function() { return this; };
Socket.prototype.unref = function() { return this; };
Socket.prototype.destroy = function() {
    if (!this.destroyed) {
        this.destroyed = true;
        this.emit("close");
    }
    return this;
};

ServerResponse.prototype.setHeader = function(name, value) {
    this.__canaryoHeaders[String(name).toLowerCase()] = String(value);
    return this;
};
ServerResponse.prototype.getHeader = function(name) {
    return this.__canaryoHeaders[String(name).toLowerCase()];
};
ServerResponse.prototype.hasHeader = function(name) {
    return Object.prototype.hasOwnProperty.call(
        this.__canaryoHeaders,
        String(name).toLowerCase()
    );
};
ServerResponse.prototype.removeHeader = function(name) {
    delete this.__canaryoHeaders[String(name).toLowerCase()];
};
ServerResponse.prototype.getHeaders = function() {
    return Object.assign(Object.create(null), this.__canaryoHeaders);
};
ServerResponse.prototype.getHeaderNames = function() {
    return Object.keys(this.__canaryoHeaders);
};
ServerResponse.prototype.getRawHeaderNames = ServerResponse.prototype.getHeaderNames;
ServerResponse.prototype.flushHeaders = function() {
    this.headersSent = true;
};
ServerResponse.prototype.writeHead = function(status, statusMessageOrHeaders, headers) {
    this.statusCode = Number(status);
    if (typeof statusMessageOrHeaders === "string") {
        this.statusMessage = statusMessageOrHeaders;
    }
    const values = typeof statusMessageOrHeaders === "object"
        ? statusMessageOrHeaders
        : headers;
    if (values) {
        for (const [name, value] of Object.entries(values)) {
            this.setHeader(name, value);
        }
    }
    this.headersSent = true;
    return this;
};
function appendResponseChunk(response, chunk) {
    if (chunk === undefined || chunk === null) return;
    if (typeof chunk === "string" && response.__canaryoBody.length === 0) {
        response.__canaryoTextBody += chunk;
        return;
    }
    if (response.__canaryoTextBody.length > 0) {
        const textBytes = Buffer.from(response.__canaryoTextBody);
        for (const byte of textBytes) response.__canaryoBody.push(byte);
        response.__canaryoTextBody = "";
    }
    const bytes = Buffer.from(chunk);
    for (const byte of bytes) response.__canaryoBody.push(byte);
}
ServerResponse.prototype.write = function(chunk) {
    appendResponseChunk(this, chunk);
    this.headersSent = true;
    return true;
};
ServerResponse.prototype.end = function(chunk) {
    if (this.writableEnded) return this;
    appendResponseChunk(this, chunk);
    this.headersSent = true;
    this.writableEnded = true;
    this.writableFinished = true;
    this.finished = true;
    this.emit("finish");
    return this;
};

function ensureHttpEventPrototypes() {
    const EventEmitter = __canaryoBuiltins.events.EventEmitter;
    for (const constructor of [IncomingMessage, ServerResponse, Socket, ClientRequest]) {
        if (!(constructor.prototype instanceof EventEmitter)) {
            Object.setPrototypeOf(constructor.prototype, EventEmitter.prototype);
        }
    }
    if (!IncomingMessage.prototype[Symbol.asyncIterator]) {
        IncomingMessage.prototype.iterator = function() {
            const events = EventEmitter.on(this, "data", { close: ["end", "close"] });
            return {
                next() {
                    return events.next().then(result => result.done
                        ? result
                        : { value: result.value[0], done: false });
                },
                return() { return events.return(); },
                throw(error) { return events.throw(error); },
                [Symbol.asyncIterator]() { return this; }
            };
        };
        IncomingMessage.prototype[Symbol.asyncIterator] = function() { return this.iterator(); };
    }
    return EventEmitter;
}

function normalizeClientRequest(input, options, defaultProtocol) {
    let values;
    if (typeof input === "string" || input instanceof URL) {
        const parsed = input instanceof URL ? input : new URL(input);
        values = Object.assign({
            protocol: parsed.protocol,
            hostname: parsed.hostname,
            port: parsed.port,
            path: parsed.pathname + parsed.search,
            auth: parsed.username ? `${parsed.username}:${parsed.password}` : undefined
        }, options || {});
    } else {
        values = Object.assign({}, input || {}, options || {});
    }
    const protocol = values.protocol || defaultProtocol;
    const hostname = values.hostname || values.host || "localhost";
    const port = values.port ? `:${values.port}` : "";
    const path = values.path || values.pathname || "/";
    return {
        method: String(values.method || "GET").toUpperCase(),
        url: `${protocol}//${hostname}${port}${path}`,
        protocol,
        hostname,
        port: values.port || "",
        path,
        headers: Object.assign({}, values.headers || {}),
        timeout: Number(values.timeout) || 0,
        agent: values.agent,
        signal: values.signal
    };
}

function ClientRequest(input, options, callback, defaultProtocol = "http:", defaultAgent) {
    const EventEmitter = ensureHttpEventPrototypes();
    EventEmitter.call(this);
    const normalized = normalizeClientRequest(input, options, defaultProtocol);
    const requestAgent = normalized.agent === undefined
        ? (defaultAgent || (normalized.protocol === "https:" ? httpsGlobalAgent : globalAgent))
        : normalized.agent;
    if (requestAgent !== false && (!requestAgent || requestAgent.__canaryoAgentId === undefined)) {
        throw new TypeError("options.agent must be an instance of Agent or false");
    }
    if (requestAgent && requestAgent.protocol !== normalized.protocol) {
        throw new Error(`Protocol ${normalized.protocol} not supported by Agent using ${requestAgent.protocol}`);
    }
    this.method = normalized.method;
    this.protocol = normalized.protocol;
    this.host = normalized.hostname;
    this.path = normalized.path;
    this.finished = false;
    this.writableEnded = false;
    this.destroyed = false;
    this.timeout = normalized.timeout || Number(requestAgent && requestAgent.options.timeout) || 0;
    this.agent = requestAgent;
    this._agentId = requestAgent === false ? 0 : requestAgent.__canaryoAgentId;
    this._url = normalized.url;
    this._headers = Object.create(null);
    for (const [name, value] of Object.entries(normalized.headers)) this.setHeader(name, value);
    if (typeof callback === "function") this.once("response", callback);
    if (normalized.signal) {
        const abortRequest = () => {
            const error = new Error("The operation was aborted");
            error.name = "AbortError";
            error.code = "ABORT_ERR";
            error.cause = normalized.signal.reason;
            this.destroy(error);
        };
        if (normalized.signal.aborted) process.nextTick(abortRequest);
        else this._abortDisposable = __canaryoBuiltins.events.addAbortListener(
            normalized.signal,
            abortRequest
        );
    }
}
ClientRequest.prototype.setHeader = function(name, value) {
    if (this.headersSent) throw new Error("Cannot set headers after they are sent");
    this._headers[String(name).toLowerCase()] = Array.isArray(value) ? value.join(", ") : String(value);
};
ClientRequest.prototype.getHeader = function(name) { return this._headers[String(name).toLowerCase()]; };
ClientRequest.prototype.hasHeader = function(name) {
    return Object.prototype.hasOwnProperty.call(this._headers, String(name).toLowerCase());
};
ClientRequest.prototype.removeHeader = function(name) {
    if (this.headersSent) throw new Error("Cannot remove headers after they are sent");
    delete this._headers[String(name).toLowerCase()];
};
ClientRequest.prototype.getHeaders = function() { return Object.assign(Object.create(null), this._headers); };
ClientRequest.prototype.getHeaderNames = function() { return Object.keys(this._headers); };
ClientRequest.prototype.__canaryoStart = function(hasBody) {
    if (this._requestId !== undefined) return;
    this._requestId = __canaryoHttpRequestStart(
        this.method,
        this._url,
        JSON.stringify(Object.entries(this._headers)),
        Boolean(hasBody),
        this._agentId
    );
    this.headersSent = true;
    pendingClientRequests.set(this._requestId, this);
    this.__canaryoArmTimeout();
};
ClientRequest.prototype.flushHeaders = function() {
    this.__canaryoStart(true);
    return this;
};
ClientRequest.prototype.__canaryoArmTimeout = function() {
    if (this._timeoutHandle) clearTimeout(this._timeoutHandle);
    if (this.timeout > 0 && this._requestId !== undefined && pendingClientRequests.has(this._requestId)) {
        this._timeoutHandle = setTimeout(() => {
            if (!this.destroyed && pendingClientRequests.has(this._requestId)) this.emit("timeout");
        }, this.timeout);
    }
};
ClientRequest.prototype.write = function(chunk, encoding, callback) {
    if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
    if (this.writableEnded) throw new Error("write after end");
    const bytes = Buffer.from(chunk, encoding);
    this.__canaryoStart(true);
    __canaryoHttpRequestWrite(this._requestId, bytes.toString("base64"));
    if (callback) process.nextTick(callback);
    return true;
};
ClientRequest.prototype.end = function(chunk, encoding, callback) {
    if (typeof chunk === "function") { callback = chunk; chunk = undefined; }
    else if (typeof encoding === "function") { callback = encoding; encoding = undefined; }
    if (chunk !== undefined) this.write(chunk, encoding);
    if (this.writableEnded) return this;
    this.finished = true;
    this.writableEnded = true;
    this.emit("finish");
    if (callback) process.nextTick(callback);
    try {
        this.__canaryoStart(false);
        __canaryoHttpRequestEnd(this._requestId);
    } catch (error) {
        process.nextTick(() => this.emit("error", error));
    }
    return this;
};
ClientRequest.prototype.abort = function() {
    this.aborted = true;
    this.destroy();
};
ClientRequest.prototype.destroy = function(error) {
    if (this.destroyed) return this;
    this.destroyed = true;
    if (this._requestId !== undefined) {
        pendingClientRequests.delete(this._requestId);
        __canaryoHttpRequestAbort(this._requestId);
    }
    if (this._timeoutHandle) clearTimeout(this._timeoutHandle);
    this._abortDisposable?.[Symbol.dispose || Symbol.for("nodejs.dispose")]();
    if (error) this.emit("error", error);
    this.emit("close");
    return this;
};
ClientRequest.prototype.setTimeout = function(timeout, callback) {
    this.timeout = Number(timeout);
    if (typeof callback === "function") this.once("timeout", callback);
    this.__canaryoArmTimeout();
    return this;
};

function clientRequest(defaultProtocol, defaultAgent, input, options, callback) {
    if (typeof options === "function") { callback = options; options = undefined; }
    return new ClientRequest(input, options, callback, defaultProtocol, defaultAgent);
}
function Agent(options = {}) {
    this.options = Object.assign({}, options);
    this.protocol = options.protocol || "http:";
    this.keepAlive = Boolean(options.keepAlive);
    this.keepAliveMsecs = Number(options.keepAliveMsecs) || 1000;
    this.maxSockets = options.maxSockets === undefined ? Agent.defaultMaxSockets : Number(options.maxSockets);
    this.maxFreeSockets = options.maxFreeSockets === undefined ? 256 : Number(options.maxFreeSockets);
    this.maxTotalSockets = options.maxTotalSockets === undefined ? Infinity : Number(options.maxTotalSockets);
    this.scheduling = options.scheduling || "lifo";
    this.requests = Object.create(null);
    this.sockets = Object.create(null);
    this.freeSockets = Object.create(null);
    this.__canaryoAgentId = __canaryoHttpAgentCreate(JSON.stringify({
        protocol: this.protocol,
        keepAlive: this.keepAlive,
        maxFreeSockets: this.maxFreeSockets,
        maxSockets: Number.isFinite(this.maxSockets) ? this.maxSockets : null,
        maxTotalSockets: Number.isFinite(this.maxTotalSockets) ? this.maxTotalSockets : null,
        timeout: Number(options.timeout) || 0,
        noDelay: options.noDelay !== false
    }));
}
Agent.defaultMaxSockets = Infinity;
Agent.prototype.getName = function(options = {}) {
    const host = options.host || options.hostname || "localhost";
    const port = options.port || (this.protocol === "https:" ? 443 : 80);
    return `${host}:${port}:${options.localAddress || ""}`;
};
Agent.prototype.destroy = function() {
    __canaryoHttpAgentDestroy(this.__canaryoAgentId);
    this.__canaryoAgentId = __canaryoHttpAgentCreate(JSON.stringify({
        protocol: this.protocol,
        keepAlive: this.keepAlive,
        maxFreeSockets: this.maxFreeSockets,
        maxSockets: Number.isFinite(this.maxSockets) ? this.maxSockets : null,
        maxTotalSockets: Number.isFinite(this.maxTotalSockets) ? this.maxTotalSockets : null,
        timeout: Number(this.options.timeout) || 0,
        noDelay: this.options.noDelay !== false
    }));
};
function HttpsAgent(options = {}) {
    Agent.call(this, Object.assign({}, options, { protocol: "https:" }));
}
HttpsAgent.prototype = Object.create(Agent.prototype, { constructor: { value: HttpsAgent } });
HttpsAgent.defaultMaxSockets = Agent.defaultMaxSockets;
const globalAgent = new Agent({ keepAlive: true });
const httpsGlobalAgent = new HttpsAgent({ keepAlive: true });
const pendingClientRequests = new Map();

function finishClientRequest(request) {
    pendingClientRequests.delete(request._requestId);
    if (request._timeoutHandle) clearTimeout(request._timeoutHandle);
    request._abortDisposable?.[Symbol.dispose || Symbol.for("nodejs.dispose")]();
    request.emit("close");
}

globalThis.__canaryoPollHttpRequests = function() {
    for (let count = 0; count < 64; count++) {
        const raw = __canaryoHttpRequestPoll();
        if (raw === undefined || raw === null) break;
        const event = JSON.parse(raw);
        const request = pendingClientRequests.get(event.id);
        if (!request || request.destroyed) continue;
        if (event.type === "headers") {
            const response = new IncomingMessage();
            ensureHttpEventPrototypes().call(response);
            response.statusCode = event.status;
            response.statusMessage = event.statusText;
            response.headers = Object.create(null);
            response.rawHeaders = [];
            for (const [name, value] of event.headers) {
                response.headers[name.toLowerCase()] = value;
                response.rawHeaders.push(name, value);
            }
            response.httpVersion = "1.1";
            response.method = null;
            response.url = request._url;
            response.readable = true;
            response.readableEnded = false;
            response.complete = false;
            response.__canaryoPaused = false;
            response.__canaryoChunkQueue = [];
            request._response = response;
            request.emit("response", response);
        } else if (event.type === "data") {
            if (request._response) {
                request._response.__canaryoDeliverChunk(Buffer.from(event.body, "base64"));
            }
        } else if (event.type === "end") {
            if (request._response) request._response.__canaryoFinishBody();
            finishClientRequest(request);
        } else if (event.type === "error") {
            request.emit("error", new Error(event.message));
            finishClientRequest(request);
        }
    }
    const pendingNetSockets = typeof globalThis.__canaryoPollNetSockets === "function"
        ? globalThis.__canaryoPollNetSockets()
        : 0;
    return pendingClientRequests.size + pendingNetSockets;
};
globalThis.__canaryoPendingHttpRequests = () => pendingClientRequests.size;

const httpModule = Object.freeze({
    IncomingMessage,
    ServerResponse,
    ClientRequest,
    Agent,
    globalAgent,
    METHODS: ["GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH"],
    STATUS_CODES: { 200: "OK", 201: "Created", 202: "Accepted", 204: "No Content", 400: "Bad Request", 404: "Not Found", 413: "Payload Too Large", 500: "Internal Server Error", 504: "Gateway Timeout" },
    request(input, options, callback) { return clientRequest("http:", globalAgent, input, options, callback); },
    get(input, options, callback) {
        const request = clientRequest("http:", globalAgent, input, options, callback);
        request.end();
        return request;
    },
    createServer(optionsOrListener, listener, tlsOptions) {
        const requestListener = typeof optionsOrListener === "function"
            ? optionsOrListener
            : listener;
        if (typeof requestListener !== "function") {
            throw new TypeError("createServer requer uma função");
        }

        const server = {
            listening: false,
            __canaryoCloseRequested: false,
            __canaryoCloseAllConnectionsRequested: false,
            __canaryoCloseIdleConnectionsRequested: false,
            __canaryoCloseCallbacks: [],
            maxConnections: Infinity,
            maxRequestSize: Number(optionsOrListener?.maxRequestSize) || 1024 * 1024,
            keepAliveTimeout: 5000,
            requestTimeout: 300000,
            timeout: 0,
            listen(port, hostOrCallback, callback) {
                const options = port !== null && typeof port === "object" ? port : null;
                const listenPort = Number(options ? options.port : port);
                const listenHost = options ? options.host : typeof hostOrCallback === "string" ? hostOrCallback : "127.0.0.1";
                const explicitCallback = typeof hostOrCallback === "function"
                    ? hostOrCallback
                    : typeof callback === "function"
                        ? callback
                        : null;
                this.__canaryoAddress = { address: listenHost || "127.0.0.1", family: "IPv4", port: listenPort };
                const currentServer = this;
                currentServer.__canaryoCloseRequested = false;
                currentServer.__canaryoCloseAllConnectionsRequested = false;
                currentServer.__canaryoCloseIdleConnectionsRequested = false;
                globalThis.__canaryoActiveServer = currentServer;
                globalThis.__canaryoPendingServerStart = () => {
                    const listenArguments = [
                        listenPort,
                        requestListener,
                        () => {
                            currentServer.listening = true;
                            currentServer.emit("listening");
                            if (explicitCallback) explicitCallback();
                        },
                        IncomingMessage.prototype,
                        ServerResponse.prototype,
                        Socket.prototype
                    ];
                    if (tlsOptions) {
                        const key = tlsOptions.key?.toString?.() ?? String(tlsOptions.key ?? "");
                        const cert = tlsOptions.cert?.toString?.() ?? String(tlsOptions.cert ?? "");
                        globalThis.__canaryoServerTlsOptions = JSON.stringify({ key, cert });
                    } else {
                        globalThis.__canaryoServerTlsOptions = null;
                    }
                    globalThis.__canaryoServerOptions = JSON.stringify({
                        maxRequestSize: Number.isFinite(currentServer.maxRequestSize)
                            ? Math.max(1, Math.trunc(currentServer.maxRequestSize))
                            : 1024 * 1024
                    });
                    __canaryoListen(...listenArguments);
                    currentServer.listening = false;
                    const callbacks = currentServer.__canaryoCloseCallbacks.splice(0);
                    for (const closeCallback of callbacks) closeCallback();
                    currentServer.emit("close");
                };
                return this;
            },
            address() { return this.__canaryoAddress || null; },
            setTimeout(value, callback) {
                this.timeout = Number(value);
                if (typeof callback === "function") this.on("timeout", callback);
                return this;
            },
            ref() { return this; },
            unref() { return this; },
            close(callback) {
                if (typeof callback === "function") this.__canaryoCloseCallbacks.push(callback);
                this.__canaryoCloseRequested = true;
                return this;
            },
            closeAllConnections() { this.__canaryoCloseAllConnectionsRequested = true; },
            closeIdleConnections() { this.__canaryoCloseIdleConnectionsRequested = true; }
        };
        const EventEmitter = ensureHttpEventPrototypes();
        EventEmitter.call(server);
        Object.setPrototypeOf(server, EventEmitter.prototype);
        return server;
    }
});
const httpsModule = Object.freeze(Object.assign({}, httpModule, {
    Agent: HttpsAgent,
    globalAgent: httpsGlobalAgent,
    request(input, options, callback) { return clientRequest("https:", httpsGlobalAgent, input, options, callback); },
    get(input, options, callback) {
        const request = clientRequest("https:", httpsGlobalAgent, input, options, callback);
        request.end();
        return request;
    },
    createServer(options, listener) {
        if (typeof options === "function") {
            throw new TypeError("https.createServer requires TLS options");
        }
        return httpModule.createServer(options, listener, options || {});
    }
}));

const moduleCache = Object.create(null);
const resolutionCache = new Map();

function loadModule(filename) {
    if (moduleCache[filename]) return moduleCache[filename].exports;

    const module = {
        id: filename,
        filename,
        exports: {},
        loaded: false,
        children: []
    };
    moduleCache[filename] = module;

    try {
        const source = __canaryoReadFile(filename);
        if (filename.endsWith(".json")) {
            module.exports = JSON.parse(source);
        } else {
            const wrapper = new Function(
                "require",
                "module",
                "exports",
                "__filename",
                "__dirname",
                `${source}\n//# sourceURL=${filename}`
            );
            wrapper(
                createRequire(filename),
                module,
                module.exports,
                filename,
                __canaryoDirname(filename)
            );
        }
        module.loaded = true;
        return module.exports;
    } catch (error) {
        delete moduleCache[filename];
        if (error && typeof error.stack === "string") {
            error.stack += `\n    while loading ${filename}`;
        }
        throw error;
    }
}

function createRequire(parentFilename) {
    function require(name) {
        if (name === "http" || name === "node:http") return httpModule;
        if (name === "https" || name === "node:https") return httpsModule;
        if (name === "http2" || name === "node:http2") {
            return {
                constants: {},
                createServer() { throw new Error("node:http2 ainda não é suportado"); },
                createSecureServer() { throw new Error("node:http2 ainda não é suportado"); }
            };
        }
        const normalized = name.startsWith("node:") ? name.slice(5) : name;
        if (Object.prototype.hasOwnProperty.call(__canaryoBuiltins, normalized)) {
            return __canaryoBuiltins[normalized];
        }
        return loadModule(resolveModule(parentFilename, name));
    }

    require.resolve = name => resolveModule(parentFilename, name);
    require.cache = moduleCache;
    return require;
}

globalThis.__canaryoCreateRequire = createRequire;
globalThis.__canaryoModuleCache = moduleCache;

function resolveModule(parentFilename, name) {
    const separator = Math.max(
        parentFilename.lastIndexOf("/"),
        parentFilename.lastIndexOf("\\")
    );
    const key = `${parentFilename.slice(0, separator + 1)}\0${name}`;
    if (resolutionCache.has(key)) return resolutionCache.get(key);
    const filename = __canaryoResolve(parentFilename, name);
    resolutionCache.set(key, filename);
    return filename;
}

globalThis.__canaryoHttpModule = httpModule;
globalThis.__canaryoHttpsModule = httpsModule;
globalThis.__canaryoLoadCommonJS = filename => loadModule(filename);
globalThis.__canaryoRunMain = filename => loadModule(filename);
globalThis.__canaryoStartPendingServer = () => {
    const start = globalThis.__canaryoPendingServerStart;
    globalThis.__canaryoPendingServerStart = null;
    if (start) start();
};
globalThis.__canaryoServerMaxConnections = () => {
    const value = Number(globalThis.__canaryoActiveServer?.maxConnections);
    return Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : null;
};
globalThis.__canaryoServerShouldClose = () => Boolean(
    globalThis.__canaryoActiveServer && globalThis.__canaryoActiveServer.__canaryoCloseRequested
);
globalThis.__canaryoServerConnectionDropped = (remoteAddress, remotePort, localAddress, localPort) => {
    globalThis.__canaryoActiveServer?.emit("drop", {
        localAddress,
        localPort,
        localFamily: "IPv4",
        remoteAddress,
        remotePort,
        remoteFamily: "IPv4"
    });
};
globalThis.__canaryoServerControl = () => {
    const server = globalThis.__canaryoActiveServer;
    if (!server) return 0;
    let control = server.__canaryoCloseRequested ? 1 : 0;
    if (server.__canaryoCloseAllConnectionsRequested) {
        server.__canaryoCloseAllConnectionsRequested = false;
        server.__canaryoCloseIdleConnectionsRequested = false;
        control |= 4;
    }
    if (!(control & 4) && server.__canaryoCloseIdleConnectionsRequested) {
        server.__canaryoCloseIdleConnectionsRequested = false;
        control |= 2;
    }
    return control;
};
})();
"#;

const POLYFILLS: &str = include_str!("polyfills.js");

pub fn execute(path: &str, arguments: &[String]) -> Result<(), String> {
    fs::metadata(path).map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    let entry = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("não foi possível resolver {path}: {error}"))?;
    let runtime = Runtime::new().map_err(|error| format!("erro ao criar runtime: {error}"))?;
    runtime.set_gc_threshold(8 * 1024 * 1024);
    runtime.set_loader(esm::NodeResolver, esm::NodeLoader);
    let context =
        Context::full(&runtime).map_err(|error| format!("erro ao criar contexto: {error}"))?;

    context.with(|context| {
        install_host_globals(&context, path, arguments)?;
        context
            .eval::<(), _>(BOOTSTRAP)
            .catch(&context)
            .map_err(|error| error.to_string())?;
        context
            .eval::<(), _>(POLYFILLS)
            .catch(&context)
            .map_err(|error| error.to_string())?;
        let module_evaluation: Option<Promise> = if modules::is_esm_path(&entry) {
            let source = fs::read(&entry).map_err(|error| error.to_string())?;
            Some(
                Module::evaluate(context.clone(), entry.to_string_lossy().as_bytes(), source)
                    .catch(&context)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            let run_main: Function = context
                .globals()
                .get("__canaryoRunMain")
                .map_err(|error| error.to_string())?;
            run_main
                .call::<_, ()>((entry.to_string_lossy().as_ref(),))
                .catch(&context)
                .map_err(|error| error.to_string())?;
            None
        };

        while context.execute_pending_job() {}
        let start_server: Function = context
            .globals()
            .get("__canaryoStartPendingServer")
            .map_err(|error| error.to_string())?;
        start_server
            .call::<_, ()>(())
            .catch(&context)
            .map_err(|error| error.to_string())?;
        if let Some(module_evaluation) = module_evaluation {
            module_evaluation
                .finish::<()>()
                .catch(&context)
                .map_err(|error| error.to_string())?;
        }
        while context.execute_pending_job() {}
        drain_event_loop(&context)?;
        Ok(())
    })
}

fn drain_event_loop(context: &rquickjs::Ctx<'_>) -> Result<(), String> {
    let poll_http: Function = context
        .globals()
        .get("__canaryoPollHttpRequests")
        .map_err(|error| error.to_string())?;
    let run_timers: Function = context
        .globals()
        .get("__canaryoRunTimers")
        .map_err(|error| error.to_string())?;
    let restore_timer_context: Function = context
        .globals()
        .get("__canaryoRestoreTimerContext")
        .map_err(|error| error.to_string())?;
    let has_referenced_timers: Function = context
        .globals()
        .get("__canaryoHasReferencedTimers")
        .map_err(|error| error.to_string())?;
    loop {
        let pending_http = poll_http
            .call::<_, usize>(())
            .catch(context)
            .map_err(|error| error.to_string())?;
        let timer_result = run_timers
            .call::<_, i64>(())
            .catch(context)
            .map_err(|error| error.to_string())?;
        let timer_delay = match timer_result {
            -1 | -2 => None,
            value if value < -2 => Some((-value - 3) as u64),
            value => Some(value as u64),
        };
        while context.execute_pending_job() {}
        if timer_result < -1 {
            restore_timer_context
                .call::<_, ()>(())
                .catch(context)
                .map_err(|error| error.to_string())?;
        }
        let timers_keep_alive = has_referenced_timers
            .call::<_, bool>(())
            .catch(context)
            .map_err(|error| error.to_string())?;
        if pending_http == 0 && !timers_keep_alive {
            return Ok(());
        }
        let delay = if pending_http > 0 {
            timer_delay.unwrap_or(2).min(2)
        } else {
            timer_delay.unwrap_or_default()
        };
        if delay > 0 {
            thread::sleep(Duration::from_millis(delay));
        }
    }
}

fn install_host_globals<'js>(
    context: &rquickjs::Ctx<'js>,
    path: &str,
    arguments: &[String],
) -> Result<(), String> {
    let globals = context.globals();
    let print = Function::new(context.clone(), |message: String| println!("{message}"))
        .map_err(|error| error.to_string())?;
    let print_error = Function::new(context.clone(), |message: String| eprintln!("{message}"))
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPrint", print)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPrintError", print_error)
        .map_err(|error| error.to_string())?;
    let write = Function::new(context.clone(), |message: String| {
        use std::io::Write;
        print!("{message}");
        let _ = std::io::stdout().flush();
    })
    .map_err(|error| error.to_string())?;
    let write_error = Function::new(context.clone(), |message: String| {
        use std::io::Write;
        eprint!("{message}");
        let _ = std::io::stderr().flush();
    })
    .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoWrite", write)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoWriteError", write_error)
        .map_err(|error| error.to_string())?;
    let cwd = Function::new(context.clone(), cwd).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoCwd", cwd)
        .map_err(|error| error.to_string())?;
    let hash = Function::new(context.clone(), crypto_digest).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHash", hash)
        .map_err(|error| error.to_string())?;
    let hmac = Function::new(context.clone(), crypto_hmac).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHmac", hmac)
        .map_err(|error| error.to_string())?;
    let random_bytes =
        Function::new(context.clone(), crypto_random_bytes).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoRandomBytes", random_bytes)
        .map_err(|error| error.to_string())?;
    let timing_safe_equal = Function::new(context.clone(), crypto_timing_safe_equal)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoTimingSafeEqual", timing_safe_equal)
        .map_err(|error| error.to_string())?;
    let zlib_transform =
        Function::new(context.clone(), zlib_transform).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoZlibTransform", zlib_transform)
        .map_err(|error| error.to_string())?;
    let byte_length =
        Function::new(context.clone(), byte_length).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoByteLength", byte_length)
        .map_err(|error| error.to_string())?;
    let is_ip = Function::new(context.clone(), is_ip).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoIsIp", is_ip)
        .map_err(|error| error.to_string())?;
    let dns_lookup =
        Function::new(context.clone(), dns_lookup).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoDnsLookup", dns_lookup)
        .map_err(|error| error.to_string())?;
    let (http_sender, http_receiver) = sync_channel(64);
    let http_runtime = Arc::new(OutboundHttpRuntime {
        sender: http_sender,
        next_request_id: AtomicU64::new(1),
        next_agent_id: AtomicU64::new(1),
        agents: Mutex::new(HashMap::new()),
        uploads: Mutex::new(HashMap::new()),
    });
    let http_agent_create = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(context.clone(), move |context, options| {
            create_outbound_http_agent(context, options, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpAgentCreate", http_agent_create)
        .map_err(|error| error.to_string())?;
    let http_agent_destroy = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(context.clone(), move |id| {
            destroy_outbound_http_agent(id, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpAgentDestroy", http_agent_destroy)
        .map_err(|error| error.to_string())?;
    let http_request = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(
            context.clone(),
            move |context, method, url, headers, has_body, agent_id| {
                start_outbound_http_request(
                    context, method, url, headers, has_body, agent_id, &runtime,
                )
            },
        )
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpRequestStart", http_request)
        .map_err(|error| error.to_string())?;
    let http_write = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(context.clone(), move |context, id, body| {
            write_outbound_http_request(context, id, body, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpRequestWrite", http_write)
        .map_err(|error| error.to_string())?;
    let http_end = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(context.clone(), move |context, id| {
            finish_outbound_http_request(context, id, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpRequestEnd", http_end)
        .map_err(|error| error.to_string())?;
    let http_abort = {
        let runtime = Arc::clone(&http_runtime);
        Function::new(context.clone(), move |id| {
            abort_outbound_http_request(id, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpRequestAbort", http_abort)
        .map_err(|error| error.to_string())?;
    let http_receiver = Arc::new(Mutex::new(http_receiver));
    let poll_http_request = {
        let receiver = http_receiver;
        Function::new(context.clone(), move || {
            receiver
                .lock()
                .ok()
                .and_then(|receiver| receiver.try_recv().ok())
                .map(|event| event.to_json())
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoHttpRequestPoll", poll_http_request)
        .map_err(|error| error.to_string())?;
    let (net_sender, net_receiver) = sync_channel(64);
    let net_runtime = Arc::new(NetClientRuntime {
        sender: net_sender,
        next_id: AtomicU64::new(1),
        commands: Arc::new(Mutex::new(HashMap::new())),
    });
    let net_connect = {
        let runtime = Arc::clone(&net_runtime);
        Function::new(context.clone(), move |context, host, port| {
            start_net_client(context, host, port, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoNetConnect", net_connect)
        .map_err(|error| error.to_string())?;
    let net_write = {
        let runtime = Arc::clone(&net_runtime);
        Function::new(context.clone(), move |context, id, body| {
            send_net_client_data(context, id, body, &runtime)
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoNetWrite", net_write)
        .map_err(|error| error.to_string())?;
    let net_end = {
        let runtime = Arc::clone(&net_runtime);
        Function::new(
            context.clone(),
            move |context: rquickjs::Ctx<'_>, id: u64| {
                send_net_client_command(&context, id, NetClientCommand::End, &runtime)
            },
        )
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoNetEnd", net_end)
        .map_err(|error| error.to_string())?;
    let net_destroy = {
        let runtime = Arc::clone(&net_runtime);
        Function::new(context.clone(), move |id| {
            if let Ok(commands) = runtime.commands.lock()
                && let Some(sender) = commands.get(&id)
            {
                let _ = sender.send(NetClientCommand::Destroy);
            }
        })
        .map_err(|error| error.to_string())?
    };
    globals
        .set("__canaryoNetDestroy", net_destroy)
        .map_err(|error| error.to_string())?;
    let net_receiver = Arc::new(Mutex::new(net_receiver));
    let net_poll = Function::new(context.clone(), move || {
        net_receiver
            .lock()
            .ok()
            .and_then(|receiver| receiver.try_recv().ok())
            .map(|event| event.to_json())
    })
    .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoNetPoll", net_poll)
        .map_err(|error| error.to_string())?;
    let net_listen =
        Function::new(context.clone(), crate::net::listen).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoNetListen", net_listen)
        .map_err(|error| error.to_string())?;
    let os_info = Object::new(context.clone()).map_err(|error| error.to_string())?;
    os_info
        .set("platform", node_platform())
        .map_err(|error| error.to_string())?;
    os_info
        .set("arch", node_arch())
        .map_err(|error| error.to_string())?;
    os_info
        .set("type", os_type())
        .map_err(|error| error.to_string())?;
    os_info
        .set("tempDir", env::temp_dir().to_string_lossy().into_owned())
        .map_err(|error| error.to_string())?;
    os_info
        .set("homeDir", home_dir())
        .map_err(|error| error.to_string())?;
    os_info
        .set("hostname", hostname())
        .map_err(|error| error.to_string())?;
    os_info
        .set(
            "parallelism",
            std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1),
        )
        .map_err(|error| error.to_string())?;
    os_info
        .set(
            "endianness",
            if cfg!(target_endian = "little") {
                "LE"
            } else {
                "BE"
            },
        )
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoOsInfo", os_info)
        .map_err(|error| error.to_string())?;
    let listen = Function::new(context.clone(), http::listen).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoListen", listen)
        .map_err(|error| error.to_string())?;
    let read_file = Function::new(context.clone(), read_file).map_err(|error| error.to_string())?;
    let resolve = Function::new(context.clone(), resolve).map_err(|error| error.to_string())?;
    let dirname = Function::new(context.clone(), dirname).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoReadFile", read_file)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoResolve", resolve)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoDirname", dirname)
        .map_err(|error| error.to_string())?;
    let fs_read = Function::new(context.clone(), fs_read).map_err(|error| error.to_string())?;
    let fs_write = Function::new(context.clone(), fs_write).map_err(|error| error.to_string())?;
    let fs_stat = Function::new(context.clone(), fs_stat).map_err(|error| error.to_string())?;
    let fs_exists = Function::new(context.clone(), fs_exists).map_err(|error| error.to_string())?;
    let fs_mkdir = Function::new(context.clone(), fs_mkdir).map_err(|error| error.to_string())?;
    let fs_readdir =
        Function::new(context.clone(), fs_readdir).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsRead", fs_read)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsWrite", fs_write)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsStat", fs_stat)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsExists", fs_exists)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsMkdir", fs_mkdir)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsReaddir", fs_readdir)
        .map_err(|error| error.to_string())?;
    let fs_unlink = Function::new(context.clone(), fs_unlink).map_err(|error| error.to_string())?;
    let fs_rename = Function::new(context.clone(), fs_rename).map_err(|error| error.to_string())?;
    let fs_copy = Function::new(context.clone(), fs_copy).map_err(|error| error.to_string())?;
    let fs_remove = Function::new(context.clone(), fs_remove).map_err(|error| error.to_string())?;
    let fs_realpath =
        Function::new(context.clone(), fs_realpath).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsUnlink", fs_unlink)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsRename", fs_rename)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsCopy", fs_copy)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsRemove", fs_remove)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsRealpath", fs_realpath)
        .map_err(|error| error.to_string())?;

    let process = Object::new(context.clone()).map_err(|error| error.to_string())?;
    let argv = Array::new(context.clone()).map_err(|error| error.to_string())?;
    argv.set(0, "canaryo").map_err(|error| error.to_string())?;
    argv.set(1, path).map_err(|error| error.to_string())?;
    for (index, argument) in arguments.iter().enumerate() {
        argv.set(index + 2, argument.as_str())
            .map_err(|error| error.to_string())?;
    }

    let environment = Object::new(context.clone()).map_err(|error| error.to_string())?;
    for (key, value) in env::vars() {
        environment
            .set(key, value)
            .map_err(|error| error.to_string())?;
    }

    process
        .set("argv", argv)
        .map_err(|error| error.to_string())?;
    process
        .set("env", environment)
        .map_err(|error| error.to_string())?;
    process
        .set("pid", std::process::id())
        .map_err(|error| error.to_string())?;
    process
        .set(
            "execPath",
            env::current_exe()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "canaryo".into()),
        )
        .map_err(|error| error.to_string())?;
    globals
        .set("process", process)
        .map_err(|error| error.to_string())?;
    globals
        .set("__filename", path)
        .map_err(|error| error.to_string())?;
    globals
        .set(
            "__dirname",
            Path::new(path)
                .parent()
                .and_then(Path::to_str)
                .unwrap_or("."),
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn read_file<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<String> {
    fs::read_to_string(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn is_ip(value: String) -> u8 {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => 4,
        Ok(IpAddr::V6(_)) => 6,
        Err(_) => 0,
    }
}

enum NetClientCommand {
    Data(Vec<u8>),
    End,
    Destroy,
}

enum NetClientEvent {
    Connected {
        id: u64,
        local_address: String,
        local_port: u16,
        remote_address: String,
        remote_port: u16,
        family: &'static str,
    },
    Data {
        id: u64,
        body: String,
    },
    End {
        id: u64,
    },
    Error {
        id: u64,
        message: String,
    },
    Close {
        id: u64,
    },
}

impl NetClientEvent {
    fn to_json(&self) -> String {
        let value = match self {
            Self::Connected {
                id,
                local_address,
                local_port,
                remote_address,
                remote_port,
                family,
            } => serde_json::json!({
                "type": "connected",
                "id": id,
                "localAddress": local_address,
                "localPort": local_port,
                "remoteAddress": remote_address,
                "remotePort": remote_port,
                "family": family,
            }),
            Self::Data { id, body } => {
                serde_json::json!({ "type": "data", "id": id, "body": body })
            }
            Self::End { id } => serde_json::json!({ "type": "end", "id": id }),
            Self::Error { id, message } => {
                serde_json::json!({ "type": "error", "id": id, "message": message })
            }
            Self::Close { id } => serde_json::json!({ "type": "close", "id": id }),
        };
        value.to_string()
    }
}

struct NetClientRuntime {
    sender: SyncSender<NetClientEvent>,
    next_id: AtomicU64,
    commands: Arc<Mutex<HashMap<u64, SyncSender<NetClientCommand>>>>,
}

fn start_net_client<'js>(
    context: rquickjs::Ctx<'js>,
    host: String,
    port: u16,
    runtime: &NetClientRuntime,
) -> rquickjs::Result<u64> {
    if host.is_empty() || port == 0 {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "TCP host and port are required",
        ));
    }
    let id = runtime.next_id.fetch_add(1, Ordering::Relaxed);
    let (command_sender, command_receiver) = sync_channel(64);
    runtime
        .commands
        .lock()
        .map_err(|_| rquickjs::Exception::throw_message(&context, "TCP client lock poisoned"))?
        .insert(id, command_sender);
    let sender = runtime.sender.clone();
    let commands = Arc::clone(&runtime.commands);
    thread::spawn(move || {
        perform_net_client(id, &host, port, command_receiver, &sender);
        if let Ok(mut commands) = commands.lock() {
            commands.remove(&id);
        }
    });
    Ok(id)
}

fn send_net_client_data<'js>(
    context: rquickjs::Ctx<'js>,
    id: u64,
    body_base64: String,
    runtime: &NetClientRuntime,
) -> rquickjs::Result<()> {
    use base64::Engine;

    let body = base64::engine::general_purpose::STANDARD
        .decode(body_base64)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    send_net_client_command(&context, id, NetClientCommand::Data(body), runtime)
}

fn send_net_client_command<'js>(
    context: &rquickjs::Ctx<'js>,
    id: u64,
    command: NetClientCommand,
    runtime: &NetClientRuntime,
) -> rquickjs::Result<()> {
    let commands = runtime
        .commands
        .lock()
        .map_err(|_| rquickjs::Exception::throw_message(context, "TCP client lock poisoned"))?;
    let sender = commands
        .get(&id)
        .ok_or_else(|| rquickjs::Exception::throw_message(context, "TCP socket is not active"))?;
    sender
        .send(command)
        .map_err(|_| rquickjs::Exception::throw_message(context, "TCP socket is closed"))
}

fn perform_net_client(
    id: u64,
    host: &str,
    port: u16,
    commands: Receiver<NetClientCommand>,
    events: &SyncSender<NetClientEvent>,
) {
    use base64::Engine;

    let mut stream = match TcpStream::connect((host, port)) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = events.send(NetClientEvent::Error {
                id,
                message: error.to_string(),
            });
            let _ = events.send(NetClientEvent::Close { id });
            return;
        }
    };
    let _ = stream.set_nonblocking(true);
    let _ = stream.set_nodelay(true);
    let local = stream.local_addr().ok();
    let remote = stream.peer_addr().ok();
    if events
        .send(NetClientEvent::Connected {
            id,
            local_address: local
                .map(|address| address.ip().to_string())
                .unwrap_or_default(),
            local_port: local.map_or(0, |address| address.port()),
            remote_address: remote
                .map(|address| address.ip().to_string())
                .unwrap_or_else(|| host.to_string()),
            remote_port: remote.map_or(port, |address| address.port()),
            family: if remote.is_some_and(|address| address.is_ipv6()) {
                "IPv6"
            } else {
                "IPv4"
            },
        })
        .is_err()
    {
        return;
    }

    let mut outgoing = VecDeque::<Vec<u8>>::new();
    let mut written = 0;
    let mut end_requested = false;
    let mut write_closed = false;
    let mut input = [0_u8; 16 * 1024];
    loop {
        loop {
            match commands.try_recv() {
                Ok(NetClientCommand::Data(body)) => outgoing.push_back(body),
                Ok(NetClientCommand::End) => end_requested = true,
                Ok(NetClientCommand::Destroy) => {
                    let _ = stream.shutdown(Shutdown::Both);
                    let _ = events.send(NetClientEvent::Close { id });
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
            }
        }

        while let Some(chunk) = outgoing.front() {
            match stream.write(&chunk[written..]) {
                Ok(0) => break,
                Ok(length) => {
                    written += length;
                    if written == chunk.len() {
                        outgoing.pop_front();
                        written = 0;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    let _ = events.send(NetClientEvent::Error {
                        id,
                        message: error.to_string(),
                    });
                    let _ = events.send(NetClientEvent::Close { id });
                    return;
                }
            }
        }
        if end_requested && outgoing.is_empty() && !write_closed {
            let _ = stream.shutdown(Shutdown::Write);
            write_closed = true;
        }

        match stream.read(&mut input) {
            Ok(0) => {
                let _ = events.send(NetClientEvent::End { id });
                let _ = events.send(NetClientEvent::Close { id });
                return;
            }
            Ok(length) => {
                if events
                    .send(NetClientEvent::Data {
                        id,
                        body: base64::engine::general_purpose::STANDARD.encode(&input[..length]),
                    })
                    .is_err()
                {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => {
                let _ = events.send(NetClientEvent::Error {
                    id,
                    message: error.to_string(),
                });
                let _ = events.send(NetClientEvent::Close { id });
                return;
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
}

enum OutboundHttpEvent {
    Headers {
        id: u64,
        status: u16,
        status_text: String,
        headers: Vec<(String, String)>,
    },
    Data {
        id: u64,
        body: String,
    },
    End {
        id: u64,
    },
    Error {
        id: u64,
        message: String,
    },
}

struct OutboundHttpRuntime {
    sender: SyncSender<OutboundHttpEvent>,
    next_request_id: AtomicU64,
    next_agent_id: AtomicU64,
    agents: Mutex<HashMap<u64, OutboundHttpAgent>>,
    uploads: Mutex<HashMap<u64, SyncSender<OutboundHttpUpload>>>,
}

struct OutboundHttpAgentOptions {
    protocol: String,
    keep_alive: bool,
    max_free_sockets: usize,
    max_sockets: usize,
    max_total_sockets: usize,
    timeout: Duration,
    no_delay: bool,
}

impl OutboundHttpAgentOptions {
    fn parse<'js>(context: &rquickjs::Ctx<'js>, json: &str) -> rquickjs::Result<Self> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|error| rquickjs::Exception::throw_message(context, &error.to_string()))?;
        let max_free_sockets = value
            .get("maxFreeSockets")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(256)
            .min(usize::MAX as u64) as usize;
        Ok(Self {
            protocol: value
                .get("protocol")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("http:")
                .to_string(),
            keep_alive: value
                .get("keepAlive")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            max_free_sockets,
            max_sockets: parse_http_socket_limit(&value, "maxSockets"),
            max_total_sockets: parse_http_socket_limit(&value, "maxTotalSockets"),
            timeout: Duration::from_millis(
                value
                    .get("timeout")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            ),
            no_delay: value
                .get("noDelay")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        })
    }

    fn build(&self) -> OutboundHttpAgent {
        let idle_connections = if self.keep_alive {
            self.max_free_sockets
        } else {
            0
        };
        let mut builder = ureq::AgentBuilder::new()
            .https_only(self.protocol == "https:")
            .max_idle_connections(idle_connections.min(self.max_total_sockets))
            .max_idle_connections_per_host(idle_connections.min(self.max_sockets))
            .no_delay(self.no_delay)
            .redirects(0);
        if !self.timeout.is_zero() {
            builder = builder.timeout(self.timeout);
        }
        OutboundHttpAgent {
            client: builder.build(),
            limiter: Arc::new(OutboundHttpAgentLimiter {
                max_sockets: self.max_sockets,
                max_total_sockets: self.max_total_sockets,
                state: Mutex::new(OutboundHttpAgentLimitState {
                    total: 0,
                    origins: HashMap::new(),
                }),
                available: Condvar::new(),
            }),
        }
    }
}

fn parse_http_socket_limit(value: &serde_json::Value, name: &str) -> usize {
    value
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .map(|limit| limit.max(1).min(usize::MAX as u64) as usize)
        .unwrap_or(usize::MAX)
}

#[derive(Clone)]
struct OutboundHttpAgent {
    client: ureq::Agent,
    limiter: Arc<OutboundHttpAgentLimiter>,
}

struct OutboundHttpAgentLimiter {
    max_sockets: usize,
    max_total_sockets: usize,
    state: Mutex<OutboundHttpAgentLimitState>,
    available: Condvar,
}

struct OutboundHttpAgentLimitState {
    total: usize,
    origins: HashMap<String, usize>,
}

impl OutboundHttpAgentLimiter {
    fn acquire(self: &Arc<Self>, origin: String) -> OutboundHttpAgentPermit {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            let origin_count = state.origins.get(&origin).copied().unwrap_or(0);
            if state.total < self.max_total_sockets && origin_count < self.max_sockets {
                state.total += 1;
                *state.origins.entry(origin.clone()).or_default() += 1;
                return OutboundHttpAgentPermit {
                    limiter: Arc::clone(self),
                    origin,
                };
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

struct OutboundHttpAgentPermit {
    limiter: Arc<OutboundHttpAgentLimiter>,
    origin: String,
}

impl Drop for OutboundHttpAgentPermit {
    fn drop(&mut self) {
        let mut state = self
            .limiter
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.total = state.total.saturating_sub(1);
        if let Some(count) = state.origins.get_mut(&self.origin) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.origins.remove(&self.origin);
            }
        }
        self.limiter.available.notify_one();
    }
}

enum OutboundHttpUpload {
    Data(Vec<u8>),
    End,
    Abort,
}

struct OutboundHttpUploadReader {
    receiver: Receiver<OutboundHttpUpload>,
    chunk: Vec<u8>,
    offset: usize,
}

impl Read for OutboundHttpUploadReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        while self.offset == self.chunk.len() {
            match self.receiver.recv() {
                Ok(OutboundHttpUpload::Data(chunk)) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Ok(OutboundHttpUpload::End) | Err(_) => return Ok(0),
                Ok(OutboundHttpUpload::Abort) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "request aborted",
                    ));
                }
            }
        }
        let available = &self.chunk[self.offset..];
        let length = available.len().min(output.len());
        output[..length].copy_from_slice(&available[..length]);
        self.offset += length;
        Ok(length)
    }
}

impl OutboundHttpEvent {
    fn to_json(&self) -> String {
        let value = match self {
            Self::Headers {
                id,
                status,
                status_text,
                headers,
            } => serde_json::json!({
                "type": "headers",
                "id": id,
                "status": status,
                "statusText": status_text,
                "headers": headers,
            }),
            Self::Data { id, body } => {
                serde_json::json!({ "type": "data", "id": id, "body": body })
            }
            Self::End { id } => serde_json::json!({ "type": "end", "id": id }),
            Self::Error { id, message } => {
                serde_json::json!({ "type": "error", "id": id, "message": message })
            }
        };
        value.to_string()
    }
}

fn create_outbound_http_agent<'js>(
    context: rquickjs::Ctx<'js>,
    options_json: String,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<u64> {
    let options = OutboundHttpAgentOptions::parse(&context, &options_json)?;
    let agent = options.build();
    let id = runtime.next_agent_id.fetch_add(1, Ordering::Relaxed);
    runtime
        .agents
        .lock()
        .map_err(|_| rquickjs::Exception::throw_message(&context, "HTTP agent lock poisoned"))?
        .insert(id, agent);
    Ok(id)
}

fn destroy_outbound_http_agent(id: u64, runtime: &OutboundHttpRuntime) {
    if let Ok(mut agents) = runtime.agents.lock() {
        agents.remove(&id);
    }
}

fn start_outbound_http_request<'js>(
    context: rquickjs::Ctx<'js>,
    method: String,
    url: String,
    headers_json: String,
    has_body: bool,
    agent_id: u64,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<u64> {
    let headers: Vec<(String, String)> = serde_json::from_str(&headers_json)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let id = runtime.next_request_id.fetch_add(1, Ordering::Relaxed);
    let sender = runtime.sender.clone();
    let agent = if agent_id == 0 {
        OutboundHttpAgentOptions {
            protocol: if url.starts_with("https:") {
                "https:".into()
            } else {
                "http:".into()
            },
            keep_alive: false,
            max_free_sockets: 0,
            max_sockets: 1,
            max_total_sockets: 1,
            timeout: Duration::ZERO,
            no_delay: true,
        }
        .build()
    } else {
        runtime
            .agents
            .lock()
            .map_err(|_| rquickjs::Exception::throw_message(&context, "HTTP agent lock poisoned"))?
            .get(&agent_id)
            .cloned()
            .ok_or_else(|| rquickjs::Exception::throw_message(&context, "HTTP agent not found"))?
    };
    let upload = if has_body {
        let (upload_sender, upload_receiver) = sync_channel(16);
        runtime
            .uploads
            .lock()
            .map_err(|_| rquickjs::Exception::throw_message(&context, "HTTP upload lock poisoned"))?
            .insert(id, upload_sender);
        Some(OutboundHttpUploadReader {
            receiver: upload_receiver,
            chunk: Vec::new(),
            offset: 0,
        })
    } else {
        None
    };
    thread::spawn(move || {
        perform_outbound_http_request(id, method, url, headers, upload, agent, sender)
    });
    Ok(id)
}

fn write_outbound_http_request<'js>(
    context: rquickjs::Ctx<'js>,
    id: u64,
    body_base64: String,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<()> {
    use base64::Engine;

    let body = base64::engine::general_purpose::STANDARD
        .decode(body_base64)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    send_outbound_http_upload(&context, id, OutboundHttpUpload::Data(body), runtime)
}

fn finish_outbound_http_request<'js>(
    context: rquickjs::Ctx<'js>,
    id: u64,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<()> {
    let sender = runtime
        .uploads
        .lock()
        .map_err(|_| rquickjs::Exception::throw_message(&context, "HTTP upload lock poisoned"))?
        .remove(&id);
    if let Some(sender) = sender {
        sender
            .send(OutboundHttpUpload::End)
            .map_err(|_| rquickjs::Exception::throw_message(&context, "HTTP upload closed"))?;
    }
    Ok(())
}

fn abort_outbound_http_request(id: u64, runtime: &OutboundHttpRuntime) {
    if let Ok(mut uploads) = runtime.uploads.lock()
        && let Some(sender) = uploads.remove(&id)
    {
        let _ = sender.send(OutboundHttpUpload::Abort);
    }
}

fn send_outbound_http_upload<'js>(
    context: &rquickjs::Ctx<'js>,
    id: u64,
    message: OutboundHttpUpload,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<()> {
    let uploads = runtime
        .uploads
        .lock()
        .map_err(|_| rquickjs::Exception::throw_message(context, "HTTP upload lock poisoned"))?;
    let sender = uploads
        .get(&id)
        .ok_or_else(|| rquickjs::Exception::throw_message(context, "HTTP upload is not active"))?;
    sender
        .send(message)
        .map_err(|_| rquickjs::Exception::throw_message(context, "HTTP upload closed"))
}

fn perform_outbound_http_request(
    id: u64,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    upload: Option<OutboundHttpUploadReader>,
    agent: OutboundHttpAgent,
    sender: SyncSender<OutboundHttpEvent>,
) {
    use base64::Engine;

    let _permit = agent.limiter.acquire(outbound_http_origin(&url));
    let mut request = agent.client.request(&method, &url);
    for (name, value) in headers {
        request = request.set(&name, &value);
    }
    let result = match upload {
        Some(reader) => request.send(reader),
        None => request.call(),
    };
    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => {
            let _ = sender.send(OutboundHttpEvent::Error {
                id,
                message: error.to_string(),
            });
            return;
        }
    };
    let status = response.status();
    let status_text = response.status_text().to_string();
    let response_headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            response
                .header(&name)
                .map(|value| (name, value.to_string()))
        })
        .collect::<Vec<_>>();
    if sender
        .send(OutboundHttpEvent::Headers {
            id,
            status,
            status_text,
            headers: response_headers,
        })
        .is_err()
    {
        return;
    }
    let mut reader = response.into_reader();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => {
                let _ = sender.send(OutboundHttpEvent::End { id });
                return;
            }
            Ok(read) => {
                if sender
                    .send(OutboundHttpEvent::Data {
                        id,
                        body: base64::engine::general_purpose::STANDARD.encode(&chunk[..read]),
                    })
                    .is_err()
                {
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(OutboundHttpEvent::Error {
                    id,
                    message: error.to_string(),
                });
                return;
            }
        }
    }
}

fn outbound_http_origin(url: &str) -> String {
    let authority_start = url.find("://").map_or(0, |index| index + 3);
    let authority_end = url[authority_start..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |index| authority_start + index);
    url[..authority_end].to_ascii_lowercase()
}

fn node_platform() -> &'static str {
    match env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        value => value,
    }
}

fn node_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        value => value,
    }
}

fn os_type() -> &'static str {
    match env::consts::OS {
        "windows" => "Windows_NT",
        "macos" => "Darwin",
        "linux" => "Linux",
        value => value,
    }
}

fn home_dir() -> String {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn hostname() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".into())
}

fn dns_lookup<'js>(
    context: rquickjs::Ctx<'js>,
    hostname: String,
    family: u8,
) -> rquickjs::Result<Array<'js>> {
    if !matches!(family, 0 | 4 | 6) {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "family must be 0, 4, or 6",
        ));
    }

    let addresses = (hostname.as_str(), 0)
        .to_socket_addrs()
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    let mut unique = Vec::new();

    for address in addresses {
        let address = address.ip();
        let address_family = match address {
            IpAddr::V4(_) => 4,
            IpAddr::V6(_) => 6,
        };
        if family != 0 && family != address_family {
            continue;
        }
        let value = address.to_string();
        if unique.contains(&value) {
            continue;
        }
        unique.push(value.clone());
        let entry = Object::new(context.clone())?;
        entry.set("address", value)?;
        entry.set("family", address_family)?;
        result.set(result.len(), entry)?;
    }

    if result.is_empty() {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "no addresses matched the requested family",
        ));
    }
    Ok(result)
}

fn fs_read<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Array<'js>> {
    let bytes = fs::read(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    for (index, byte) in bytes.into_iter().enumerate() {
        result.set(index, byte)?;
    }
    Ok(result)
}

fn fs_write<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    bytes: Vec<u8>,
    append: bool,
) -> rquickjs::Result<()> {
    use std::io::Write;

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_stat<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Object<'js>> {
    use std::time::UNIX_EPOCH;

    let metadata = fs::metadata(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Object::new(context.clone())?;
    result.set("size", metadata.len() as f64)?;
    result.set("file", metadata.is_file())?;
    result.set("directory", metadata.is_dir())?;
    result.set("symlink", metadata.file_type().is_symlink())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    result.set("mtimeMs", modified)?;
    Ok(result)
}

fn fs_exists(path: String) -> bool {
    Path::new(&path).exists()
}

fn fs_mkdir<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    recursive: bool,
) -> rquickjs::Result<()> {
    let result = if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    };
    result.map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_readdir<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Array<'js>> {
    let entries = fs::read_dir(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    for (index, entry) in entries.enumerate() {
        let entry = entry
            .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
        result.set(index, entry.file_name().to_string_lossy().as_ref())?;
    }
    Ok(result)
}

fn fs_unlink<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<()> {
    fs::remove_file(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_rename<'js>(context: rquickjs::Ctx<'js>, from: String, to: String) -> rquickjs::Result<()> {
    fs::rename(from, to)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_copy<'js>(context: rquickjs::Ctx<'js>, from: String, to: String) -> rquickjs::Result<f64> {
    fs::copy(from, to)
        .map(|bytes| bytes as f64)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_remove<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    recursive: bool,
    force: bool,
) -> rquickjs::Result<()> {
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if force && error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(rquickjs::Exception::throw_message(
                &context,
                &error.to_string(),
            ));
        }
    };
    let result = if metadata.is_dir() {
        if recursive {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        }
    } else {
        fs::remove_file(path)
    };
    result.map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_realpath<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<String> {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn resolve<'js>(
    context: rquickjs::Ctx<'js>,
    parent: String,
    specifier: String,
) -> rquickjs::Result<String> {
    modules::resolve(&parent, &specifier)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn dirname(path: String) -> String {
    Path::new(&path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}

fn byte_length(value: rquickjs::String<'_>) -> rquickjs::Result<usize> {
    Ok(value.to_cstring()?.len())
}

fn cwd() -> String {
    env::current_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".into())
}

fn decode_crypto_input<'js>(
    context: &rquickjs::Ctx<'js>,
    contents: &str,
) -> rquickjs::Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(contents)
        .map_err(|error| rquickjs::Exception::throw_message(context, &error.to_string()))
}

fn digest_algorithm(name: &str) -> Option<&'static ring::digest::Algorithm> {
    match name.to_ascii_lowercase().replace('-', "").as_str() {
        "sha1" => Some(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY),
        "sha256" => Some(&ring::digest::SHA256),
        "sha384" => Some(&ring::digest::SHA384),
        "sha512" => Some(&ring::digest::SHA512),
        _ => None,
    }
}

fn hmac_algorithm(name: &str) -> Option<ring::hmac::Algorithm> {
    match name.to_ascii_lowercase().replace('-', "").as_str() {
        "sha1" => Some(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY),
        "sha256" => Some(ring::hmac::HMAC_SHA256),
        "sha384" => Some(ring::hmac::HMAC_SHA384),
        "sha512" => Some(ring::hmac::HMAC_SHA512),
        _ => None,
    }
}

fn crypto_digest<'js>(
    context: rquickjs::Ctx<'js>,
    algorithm: String,
    contents: String,
) -> rquickjs::Result<String> {
    let algorithm = digest_algorithm(&algorithm).ok_or_else(|| {
        rquickjs::Exception::throw_message(&context, &format!("unsupported hash: {algorithm}"))
    })?;
    let contents = decode_crypto_input(&context, &contents)?;
    let digest = ring::digest::digest(algorithm, &contents);
    Ok(base64::engine::general_purpose::STANDARD.encode(digest.as_ref()))
}

fn crypto_hmac<'js>(
    context: rquickjs::Ctx<'js>,
    algorithm: String,
    key: String,
    contents: String,
) -> rquickjs::Result<String> {
    let algorithm = hmac_algorithm(&algorithm).ok_or_else(|| {
        rquickjs::Exception::throw_message(&context, &format!("unsupported hash: {algorithm}"))
    })?;
    let key = decode_crypto_input(&context, &key)?;
    let contents = decode_crypto_input(&context, &contents)?;
    let tag = ring::hmac::sign(&ring::hmac::Key::new(algorithm, &key), &contents);
    Ok(base64::engine::general_purpose::STANDARD.encode(tag.as_ref()))
}

fn crypto_random_bytes<'js>(context: rquickjs::Ctx<'js>, size: u32) -> rquickjs::Result<String> {
    let mut bytes = vec![0_u8; size as usize];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            rquickjs::Exception::throw_message(&context, "operating system random generator failed")
        })?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn crypto_timing_safe_equal<'js>(
    context: rquickjs::Ctx<'js>,
    left: String,
    right: String,
) -> rquickjs::Result<bool> {
    let left = decode_crypto_input(&context, &left)?;
    let right = decode_crypto_input(&context, &right)?;
    if left.len() != right.len() {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "Input buffers must have the same byte length",
        ));
    }
    Ok(bool::from(left.ct_eq(&right)))
}

fn zlib_transform<'js>(
    context: rquickjs::Ctx<'js>,
    operation: String,
    contents: String,
) -> rquickjs::Result<String> {
    use flate2::{
        Compression,
        read::{DeflateDecoder, GzDecoder, ZlibDecoder},
        write::{DeflateEncoder, GzEncoder, ZlibEncoder},
    };

    let input = decode_crypto_input(&context, &contents)?;
    let result = match operation.as_str() {
        "gzip" => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&input).and_then(|_| encoder.finish())
        }
        "gunzip" => read_compressed(GzDecoder::new(input.as_slice())),
        "deflate" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&input).and_then(|_| encoder.finish())
        }
        "inflate" => read_compressed(ZlibDecoder::new(input.as_slice())),
        "deflateRaw" => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&input).and_then(|_| encoder.finish())
        }
        "inflateRaw" => read_compressed(DeflateDecoder::new(input.as_slice())),
        "unzip" if input.starts_with(&[0x1f, 0x8b]) => {
            read_compressed(GzDecoder::new(input.as_slice()))
        }
        "unzip" => read_compressed(ZlibDecoder::new(input.as_slice())),
        "brotliCompress" => {
            let mut reader = input.as_slice();
            let mut output = Vec::new();
            let params = brotli::enc::BrotliEncoderParams::default();
            brotli::BrotliCompress(&mut reader, &mut output, &params).map(|_| output)
        }
        "brotliDecompress" => {
            let mut output = Vec::new();
            brotli::BrotliDecompress(&mut input.as_slice(), &mut output).map(|_| output)
        }
        _ => {
            return Err(rquickjs::Exception::throw_message(
                &context,
                &format!("unsupported zlib operation: {operation}"),
            ));
        }
    };
    let output =
        result.map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(output))
}

fn read_compressed(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_javascript() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let result = context.with(|context| context.eval::<i32, _>("21 * 2").unwrap());

        assert_eq!(result, 42);
    }

    #[test]
    fn reports_javascript_exceptions() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let error = context.with(|context| {
            context
                .eval::<(), _>("throw new Error('boom')")
                .catch(&context)
                .unwrap_err()
                .to_string()
        });

        assert!(error.contains("boom"));
    }

    #[test]
    fn buffer_byte_length_counts_utf8_without_allocating_a_buffer() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let matches_node = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    Buffer.byteLength("Canaryo") === 7 &&
                    Buffer.byteLength("can\u00e1rio \ud83d\udc24") === 13 &&
                    Buffer.byteLength("\ud800") === 3 &&
                    Buffer.from("hello").toString() === "hello" &&
                    Buffer.from("ff00a5", "hex").toString("hex") === "ff00a5" &&
                    Buffer.from("Canaryo", "utf8").toString("base64") === "Q2FuYXJ5bw==" &&
                    Buffer.from("Q2FuYXJ5bw==", "base64").toString() === "Canaryo" &&
                    Buffer.from("canário").equals(Buffer.from("canário")) &&
                    Buffer.compare(Buffer.from("a"), Buffer.from("b")) < 0 &&
                    Buffer.isEncoding("utf-16le") && !Buffer.isEncoding("unknown") &&
                    Buffer.from("Canaryo").includes("aryo") &&
                    Buffer.from("Canaryo").indexOf("nar") === 2 &&
                    Buffer.from([0x78, 0x56, 0x34, 0x12]).readUInt32LE() === 0x12345678 &&
                    Buffer.alloc(4).writeUInt32BE(0x12345678) === 4 &&
                    JSON.stringify(Buffer.from([1, 2])) === '{"type":"Buffer","data":[1,2]}' &&
                    new __canaryoBuiltins.string_decoder.StringDecoder("utf-8")
                        .write(Buffer.from("hello")) === "hello" &&
                    Buffer.byteLength(new Uint8Array([1, 2, 3])) === 3 &&
                    Buffer.byteLength(new Uint8Array([1, 2, 3]).subarray(1)) === 2 &&
                    Buffer.byteLength(new ArrayBuffer(4)) === 4 &&
                    __canaryoBuiltins.buffer.Blob === Blob &&
                    __canaryoBuiltins.buffer.File === File &&
                    new Blob(["can", Buffer.from("aryo")], { type: "TEXT/PLAIN" }).size === 7 &&
                    new File(["value"], "data.txt", { lastModified: 42 }).lastModified === 42 &&
                    atob(btoa("Canaryo")) === "Canaryo"
                    "#,
                )
                .unwrap()
        });

        assert!(matches_node);
    }

    #[test]
    fn provides_common_node_crypto_operations() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let matches_node = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const crypto = __canaryoBuiltins.crypto;
                    const digest = crypto.createHash("sha256").update("hello").digest();
                    const hmac = crypto.createHmac("sha256", "key")
                        .update("The quick brown fox jumps over the lazy dog")
                        .digest("hex");
                    const firstRandom = crypto.randomBytes(32);
                    const secondRandom = crypto.randomBytes(32);
                    const partiallyFilled = Buffer.alloc(8, 7);
                    crypto.randomFillSync(partiallyFilled, 2, 4);
                    const webRandom = new Uint16Array(8);
                    crypto.webcrypto.getRandomValues(webRandom);
                    const uuid = crypto.randomUUID();
                    let lengthMismatchThrows = false;
                    try { crypto.timingSafeEqual(Buffer.alloc(1), Buffer.alloc(2)); }
                    catch { lengthMismatchThrows = true; }

                    Buffer.isBuffer(digest) &&
                    digest.toString("hex") === "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" &&
                    hmac === "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8" &&
                    firstRandom.length === 32 && !firstRandom.equals(secondRandom) &&
                    partiallyFilled[0] === 7 && partiallyFilled[1] === 7 &&
                    partiallyFilled[6] === 7 && partiallyFilled[7] === 7 &&
                    webRandom.some(value => value !== 0) &&
                    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(uuid) &&
                    crypto.timingSafeEqual(Buffer.from("same"), Buffer.from("same")) &&
                    !crypto.timingSafeEqual(Buffer.from("same"), Buffer.from("diff")) &&
                    lengthMismatchThrows && globalThis.crypto === crypto.webcrypto
                    "#,
                )
                .unwrap()
        });

        assert!(matches_node);
    }

    #[test]
    fn compresses_with_common_node_zlib_apis() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const zlib = __canaryoBuiltins.zlib;
                    const source = Buffer.from("Canaryo compression: 🐤 ".repeat(20));
                    const gzip = zlib.gzipSync(source);
                    const deflate = zlib.deflateSync(source);
                    const raw = zlib.deflateRawSync(source);
                    const brotli = zlib.brotliCompressSync(source);
                    const streamed = [];
                    const gzipStream = zlib.createGzip();
                    gzipStream.on("data", chunk => streamed.push(chunk));
                    gzipStream.write(source.subarray(0, 40));
                    gzipStream.end(source.subarray(40));
                    globalThis.zlibCallbackPassed = false;
                    zlib.gzip(source, (error, compressed) => {
                        zlibCallbackPassed = !error && zlib.gunzipSync(compressed).equals(source);
                    });

                    gzip[0] === 0x1f && gzip[1] === 0x8b &&
                    zlib.gunzipSync(gzip).equals(source) &&
                    zlib.unzipSync(gzip).equals(source) &&
                    zlib.inflateSync(deflate).equals(source) &&
                    zlib.unzipSync(deflate).equals(source) &&
                    zlib.inflateRawSync(raw).equals(source) &&
                    zlib.brotliDecompressSync(brotli).equals(source) &&
                    zlib.gunzipSync(Buffer.concat(streamed)).equals(source)
                    "#,
                )
                .unwrap();
            assert!(synchronous);

            while context.execute_pending_job() {}
            assert!(
                context
                    .globals()
                    .get::<_, bool>("zlibCallbackPassed")
                    .unwrap()
            );
        });
    }

    #[test]
    fn schedules_and_cancels_timers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let timer_created = context
                .eval::<bool, _>(
                    r#"
                    globalThis.timerResult = 0;
                    const cancelled = setTimeout(() => { timerResult = -1; }, 0);
                    clearTimeout(cancelled);
                    const handle = setTimeout((left, right) => {
                        timerResult = left + right;
                    }, 0, 20, 22);
                    handle.hasRef() && Number(handle) > 0
                    "#,
                )
                .unwrap();
            assert!(timer_created);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            assert_eq!(run_timers.call::<_, i64>(()).unwrap(), -1);
            assert_eq!(context.globals().get::<_, i32>("timerResult").unwrap(), 42);
        });
    }

    #[test]
    fn exposes_promise_timers_streams_and_builtin_aliases() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const timers = __canaryoBuiltins["timers/promises"];
                    const streams = __canaryoBuiltins.stream;
                    const streamPromises = __canaryoBuiltins["stream/promises"];
                    globalThis.promiseTimerValue = 0;
                    globalThis.promiseImmediateValue = "";
                    globalThis.promiseIntervalValue = "";
                    globalThis.promiseSchedulerDone = false;
                    globalThis.promisePipelineDone = false;
                    globalThis.promisePipelineOutput = "";
                    timers.setTimeout(0, 42).then(value => { promiseTimerValue = value; });
                    timers.setImmediate("ready").then(value => { promiseImmediateValue = value; });
                    const interval = timers.setInterval(0, "tick");
                    interval.next().then(result => {
                        promiseIntervalValue = result.value;
                        return interval.return();
                    });
                    timers.scheduler.wait(0).then(() => { promiseSchedulerDone = true; });
                    const source = new streams.Readable();
                    const destination = new streams.Writable({
                        write(chunk, _encoding, callback) {
                            promisePipelineOutput += chunk.toString();
                            callback();
                        }
                    });
                    streamPromises.pipeline(source, destination).then(() => {
                        promisePipelineDone = true;
                    });
                    source.push("canaryo");
                    source.push(null);
                    __canaryoBuiltins.console === globalThis.console &&
                        __canaryoBuiltins.process === process &&
                        streams.promises === streamPromises &&
                        typeof timers.scheduler.wait === "function"
                    "#,
                )
                .unwrap();

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..4 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert_eq!(
                context
                    .globals()
                    .get::<_, i32>("promiseTimerValue")
                    .unwrap(),
                42
            );
            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("promiseImmediateValue")
                    .unwrap(),
                "ready"
            );
            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("promiseIntervalValue")
                    .unwrap(),
                "tick"
            );
            assert!(
                context
                    .globals()
                    .get::<_, bool>("promiseSchedulerDone")
                    .unwrap()
            );
            assert!(
                context
                    .globals()
                    .get::<_, bool>("promisePipelineDone")
                    .unwrap()
            );
            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("promisePipelineOutput")
                    .unwrap(),
                "canaryo"
            );
        });
    }

    #[test]
    fn reads_and_writes_files_through_node_apis() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!("canaryo-fs-{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let filename = directory.join("message.txt");
        let stream_filename = directory.join("stream.bin");
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context
                .globals()
                .set("fixturePath", filename.to_string_lossy().as_ref())
                .unwrap();
            context
                .globals()
                .set("fixtureStreamPath", stream_filename.to_string_lossy().as_ref())
                .unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const fs = __canaryoBuiltins.fs;
                    fs.writeFileSync(fixturePath, "canário");
                    fs.appendFileSync(fixturePath, "!");
                    const raw = fs.readFileSync(fixturePath);
                    globalThis.fsPromiseResult = false;
                    fs.promises.readFile(fixturePath, "utf8").then(value => {
                        fsPromiseResult = value === "canário!";
                    });
                    globalThis.fsStreamResult = false;
                    const output = fs.createWriteStream(fixtureStreamPath);
                    output.on("finish", () => {
                        const chunks = [];
                        const input = fs.createReadStream(fixtureStreamPath, { start: 1, end: 3, highWaterMark: 2 });
                        input.on("data", chunk => chunks.push(chunk));
                        input.on("end", () => {
                            fsStreamResult = Buffer.concat(chunks).equals(Buffer.from([20, 30, 40])) &&
                                input.bytesRead === 3 && output.bytesWritten === 5;
                        });
                    });
                    output.end(Buffer.from([10, 20, 30, 40, 50]));
                    Buffer.isBuffer(raw) && raw.toString() === "canário!" &&
                        fs.existsSync(fixturePath) && fs.statSync(fixturePath).isFile()
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}
            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..4 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert!(context.globals().get::<_, bool>("fsPromiseResult").unwrap());
            assert!(context.globals().get::<_, bool>("fsStreamResult").unwrap());
        });

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn manages_files_through_sync_callback_and_promise_apis() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            env::temp_dir().join(format!("canaryo-fs-manage-{}-{id}", std::process::id()));
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context
                .globals()
                .set("fixtureDirectory", directory.to_string_lossy().as_ref())
                .unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const fs = __canaryoBuiltins.fs;
                    const path = __canaryoBuiltins.path;
                    const nested = path.join(fixtureDirectory, "nested");
                    const original = path.join(nested, "original.txt");
                    const copied = path.join(nested, "copied.txt");
                    const renamed = path.join(nested, "renamed.txt");
                    const promised = path.join(nested, "promised.txt");
                    const callbackCopy = path.join(nested, "callback.txt");
                    fs.mkdirSync(nested, { recursive: true });
                    fs.writeFileSync(original, "managed");
                    fs.copyFileSync(original, copied);
                    fs.renameSync(copied, renamed);
                    const real = fs.realpathSync(renamed);
                    fs.unlinkSync(original);
                    fs.rmSync(path.join(nested, "missing.txt"), { force: true });
                    globalThis.fsManagementPromise = false;
                    globalThis.fsManagementCallback = false;
                    fs.copyFile(renamed, callbackCopy, error => {
                        if (error) throw error;
                        fs.unlink(callbackCopy, unlinkError => {
                            if (unlinkError) throw unlinkError;
                            fsManagementCallback = true;
                            fs.promises.copyFile(renamed, promised)
                                .then(() => fs.promises.unlink(promised))
                                .then(() => fs.promises.rm(fixtureDirectory, { recursive: true }))
                                .then(() => { fsManagementPromise = !fs.existsSync(fixtureDirectory); });
                        });
                    });

                    !fs.existsSync(original) && fs.readFileSync(renamed, "utf8") === "managed" &&
                        typeof real === "string" && real.length > 0 &&
                        fs.realpath.native === fs.realpath && fs.realpathSync.native === fs.realpathSync
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("fsManagementCallback")
                    .unwrap()
            );
            assert!(
                context
                    .globals()
                    .get::<_, bool>("fsManagementPromise")
                    .unwrap()
            );
        });

        if directory.exists() {
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn supports_event_emitter_ordering_and_helpers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const EventEmitter = __canaryoBuiltins.events;
                    const emitter = new EventEmitter();
                    const order = [];
                    function regular(value) { order.push(`regular:${value}`); }
                    emitter.on("value", regular);
                    emitter.prependOnceListener("value", value => order.push(`first:${value}`));
                    emitter.emit("value", 1);
                    emitter.emit("value", 2);
                    globalThis.eventPromiseResolved = false;
                    EventEmitter.once(emitter, "done").then(([value]) => {
                        eventPromiseResolved = value === 42;
                    });
                    emitter.emit("done", 42);
                    let errorThrown = false;
                    try { emitter.emit("error", new Error("boom")); }
                    catch (error) { errorThrown = error.message === "boom"; }
                    order.join(",") === "first:1,regular:1,regular:2" &&
                        emitter.listeners("value")[0] === regular &&
                        emitter.rawListeners("value")[0] === regular &&
                        emitter.eventNames().includes("value") && errorThrown
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("eventPromiseResolved")
                    .unwrap()
            );
        });
    }

    #[test]
    fn captures_event_listener_rejections() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const events = __canaryoBuiltins.events;
                    globalThis.capturedEventError = "";
                    const emitter = new events.EventEmitter({ captureRejections: true });
                    emitter.on("error", error => { capturedEventError = error.message; });
                    emitter.once("task", async () => { throw new Error("rejected task"); });
                    emitter.emit("task");

                    globalThis.customCapturedEvent = "";
                    const custom = new events.EventEmitter({ captureRejections: true });
                    custom[events.captureRejectionSymbol] = (error, name, value) => {
                        customCapturedEvent = `${error.message}:${name}:${value}`;
                    };
                    custom.on("work", () => Promise.reject(new Error("custom rejection")));
                    custom.emit("work", 42);

                    globalThis.monitoredErrorOrder = [];
                    const monitored = new events.EventEmitter();
                    monitored.on(events.errorMonitor, error => monitoredErrorOrder.push(`monitor:${error.message}`));
                    monitored.on("error", error => monitoredErrorOrder.push(`listener:${error.message}`));
                    monitored.emit("error", new Error("observed"));

                    events.captureRejections = true;
                    globalThis.globalCaptureWorked = false;
                    const inherited = new events.EventEmitter();
                    events.captureRejections = false;
                    inherited.on("error", () => { globalCaptureWorked = true; });
                    inherited.on("task", () => Promise.reject(new Error("global")));
                    inherited.emit("task");
                    "#,
                )
                .unwrap();

            while context.execute_pending_job() {}

            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("capturedEventError")
                    .unwrap(),
                "rejected task"
            );
            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("customCapturedEvent")
                    .unwrap(),
                "custom rejection:work:42"
            );
            let order: Array = context.globals().get("monitoredErrorOrder").unwrap();
            assert_eq!(order.get::<String>(0).unwrap(), "monitor:observed");
            assert_eq!(order.get::<String>(1).unwrap(), "listener:observed");
            assert!(
                context
                    .globals()
                    .get::<_, bool>("globalCaptureWorked")
                    .unwrap()
            );
        });
    }

    #[test]
    fn emits_events_in_their_async_resource_scope() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let supported = context
                .eval::<bool, _>(
                    r#"
                    const events = __canaryoBuiltins.events;
                    const hooks = __canaryoBuiltins.async_hooks;
                    const storage = new hooks.AsyncLocalStorage();
                    let emitter;
                    storage.run("creation context", () => {
                        emitter = new events.EventEmitterAsyncResource({ name: "canaryo:event" });
                    });
                    let observedStore;
                    let observedAsyncId;
                    emitter.on("value", () => {
                        observedStore = storage.getStore();
                        observedAsyncId = hooks.executionAsyncId();
                    });
                    storage.run("calling context", () => emitter.emit("value"));
                    const resourceId = emitter.asyncId;
                    emitter.emitDestroy();
                    observedStore === "creation context" &&
                        observedAsyncId === resourceId &&
                        resourceId > 1 && emitter.triggerAsyncId >= 1
                    "#,
                )
                .unwrap();

            assert!(supported);
        });
    }

    #[test]
    fn supports_abort_signals_across_events_timers_and_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const events = __canaryoBuiltins.events;
                    const timers = __canaryoBuiltins["timers/promises"];
                    const streams = __canaryoBuiltins.stream;

                    const target = new EventTarget();
                    let eventCalls = 0;
                    target.addEventListener("value", () => eventCalls++, { once: true });
                    target.dispatchEvent(new Event("value"));
                    target.dispatchEvent(new Event("value"));
                    const cancelable = new Event("cancel", { cancelable: true });
                    target.addEventListener("cancel", event => event.preventDefault());
                    const dispatchResult = target.dispatchEvent(cancelable);

                    const listenerController = new AbortController();
                    let removedListenerCalled = false;
                    target.addEventListener("removed", () => { removedListenerCalled = true; }, {
                        signal: listenerController.signal
                    });
                    listenerController.abort("remove-listener");
                    target.dispatchEvent(new Event("removed"));

                    const combinedLeft = new AbortController();
                    const combinedRight = new AbortController();
                    const combined = AbortSignal.any([combinedLeft.signal, combinedRight.signal]);
                    combinedRight.abort("combined-reason");

                    globalThis.eventAbortRejected = false;
                    const emitter = new events.EventEmitter();
                    const eventController = new AbortController();
                    events.once(emitter, "done", { signal: eventController.signal }).catch(error => {
                        eventAbortRejected = error.name === "AbortError" &&
                            error.code === "ABORT_ERR" && error.cause === "event-reason";
                    });
                    eventController.abort("event-reason");

                    globalThis.timerAbortRejected = false;
                    const timerController = new AbortController();
                    timers.setTimeout(100, 42, { signal: timerController.signal }).catch(error => {
                        timerAbortRejected = error.name === "AbortError" &&
                            error.code === "ABORT_ERR" && error.cause === "timer-reason";
                    });
                    timerController.abort("timer-reason");

                    globalThis.streamAbortObserved = false;
                    const readable = new streams.Readable();
                    readable.on("error", error => {
                        streamAbortObserved = error.name === "AbortError" && error.code === "ABORT_ERR";
                    });
                    const streamController = new AbortController();
                    streams.addAbortSignal(streamController.signal, readable);
                    streamController.abort("stream-reason");

                    globalThis.timeoutSignalAborted = false;
                    const timeoutSignal = AbortSignal.timeout(0);
                    timeoutSignal.addEventListener("abort", () => {
                        timeoutSignalAborted = timeoutSignal.reason.name === "TimeoutError";
                    });

                    eventCalls === 1 && !dispatchResult && cancelable.defaultPrevented &&
                        !removedListenerCalled && combined.aborted &&
                        combined.reason === "combined-reason" && readable.destroyed &&
                        AbortSignal.abort().reason.name === "AbortError"
                    "#,
                )
                .unwrap();

            while context.execute_pending_job() {}
            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            run_timers.call::<_, i64>(()).unwrap();
            while context.execute_pending_job() {}

            assert!(synchronous);
            for name in [
                "eventAbortRejected",
                "timerAbortRejected",
                "streamAbortObserved",
                "timeoutSignalAborted",
            ] {
                assert!(context.globals().get::<_, bool>(name).unwrap(), "{name}");
            }
        });
    }

    #[test]
    fn publishes_diagnostics_channels_and_binds_stores() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const diagnostics = __canaryoBuiltins.diagnostics_channel;
                    const { AsyncLocalStorage } = __canaryoBuiltins.async_hooks;
                    const channel = diagnostics.channel("canaryo.request");
                    const storage = new AsyncLocalStorage();
                    const messages = [];
                    const subscriber = (message, name) => messages.push(`${name}:${message.id}`);
                    diagnostics.subscribe("canaryo.request", subscriber);
                    channel.bindStore(storage, message => ({ requestId: message.id }));
                    let stored;
                    channel.runStores({ id: 42 }, () => {
                        stored = storage.getStore().requestId;
                        channel.publish({ id: 42 });
                    });
                    const removed = diagnostics.unsubscribe("canaryo.request", subscriber);
                    channel.publish({ id: 0 });
                    const unbound = channel.unbindStore(storage);
                    diagnostics.channel("canaryo.request") === channel &&
                        stored === 42 && messages.join() === "canaryo.request:42" &&
                        removed && unbound && !diagnostics.hasSubscribers("canaryo.request")
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn traces_sync_promise_and_callback_operations() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const diagnostics = __canaryoBuiltins.diagnostics_channel;
                    const tracing = diagnostics.tracingChannel("canaryo.operation");
                    globalThis.traceEvents = [];
                    const subscribers = {};
                    for (const event of ["start", "end", "asyncStart", "asyncEnd", "error"]) {
                        subscribers[event] = context => traceEvents.push(
                            `${event}:${context.name}:${context.result ?? ""}:${context.error?.message ?? ""}`
                        );
                    }
                    tracing.subscribe(subscribers);
                    globalThis.syncTraceResult = tracing.traceSync(
                        function (value) { return value + this.offset; },
                        { name: "sync" },
                        { offset: 2 },
                        40
                    );
                    globalThis.callbackTraceResult = tracing.traceCallback(
                        callback => { callback(null, 7); return 9; },
                        0,
                        { name: "callback" },
                        undefined,
                        value => { globalThis.callbackValue = value; }
                    );
                    globalThis.promiseTraceDone = false;
                    tracing.tracePromise(
                        () => Promise.resolve(11),
                        { name: "promise" }
                    ).then(value => { promiseTraceDone = value === 11; });
                    globalThis.traceSubscribers = subscribers;
                    globalThis.traceChannel = tracing;
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            let supported = context
                .eval::<bool, _>(
                    r#"
                    const removed = traceChannel.unsubscribe(traceSubscribers);
                    syncTraceResult === 42 && callbackTraceResult === 9 && callbackValue === null &&
                        promiseTraceDone && removed && !traceChannel.hasSubscribers &&
                        traceEvents.join("|") === [
                            "start:sync::", "end:sync:42:",
                            "start:callback::", "asyncStart:callback:7:",
                            "asyncEnd:callback:7:", "end:callback:7:",
                            "start:promise::", "end:promise::",
                            "asyncStart:promise:11:", "asyncEnd:promise:11:"
                        ].join("|")
                    "#,
                )
                .unwrap();
            assert!(supported);
        });
    }

    #[test]
    fn binds_async_local_storage_snapshots_to_resources() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const { AsyncLocalStorage, AsyncResource } = __canaryoBuiltins.async_hooks;
                    const storage = new AsyncLocalStorage();
                    let bound;
                    let snapshot;
                    let resource;
                    storage.run({ value: 42 }, () => {
                        bound = AsyncLocalStorage.bind(() => storage.getStore()?.value);
                        snapshot = AsyncLocalStorage.snapshot();
                        resource = new AsyncResource("canaryo.test");
                    });
                    storage.getStore() === undefined &&
                        bound() === 42 &&
                        snapshot(() => storage.getStore()?.value) === 42 &&
                        resource.runInAsyncScope(() => storage.getStore()?.value) === 42
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn tracks_async_resource_lifecycle_and_execution_ids() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.asyncHooksModule = __canaryoBuiltins.async_hooks;
                    globalThis.asyncHookEvents = [];
                    globalThis.rootAsyncResource = asyncHooksModule.executionAsyncResource();
                    globalThis.asyncHook = asyncHooksModule.createHook({
                        init(id, type, trigger, resource) {
                            asyncHookEvents.push(`init:${id}:${type}:${trigger}:${resource.type}`);
                        },
                        before(id) {
                            asyncHookEvents.push(`before:${id}:${asyncHooksModule.executionAsyncId()}`);
                        },
                        after(id) {
                            asyncHookEvents.push(`after:${id}:${asyncHooksModule.executionAsyncId()}`);
                        },
                        destroy(id) {
                            asyncHookEvents.push(`destroy:${id}`);
                        }
                    }).enable();
                    globalThis.testAsyncResource = new asyncHooksModule.AsyncResource("CANARYO_TEST");
                    globalThis.asyncResourceResult = testAsyncResource.runInAsyncScope(function (value) {
                        asyncHookEvents.push(`inside:${asyncHooksModule.executionAsyncId()}:${asyncHooksModule.triggerAsyncId()}`);
                        return value + this.offset;
                    }, { offset: 2 }, 40);
                    testAsyncResource.emitDestroy();
                    "#,
                )
                .unwrap();

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            run_timers.call::<_, i64>(()).unwrap();

            let supported = context
                .eval::<bool, _>(
                    r#"
                    asyncHook.disable();
                    asyncResourceResult === 42 &&
                        testAsyncResource.asyncId() === 2 &&
                        testAsyncResource.triggerAsyncId() === 1 &&
                        asyncHooksModule.executionAsyncId() === 1 &&
                        asyncHooksModule.triggerAsyncId() === 0 &&
                        asyncHooksModule.executionAsyncResource() === rootAsyncResource &&
                        asyncHookEvents.join("|") ===
                            "init:2:CANARYO_TEST:1:CANARYO_TEST|before:2:2|inside:2:1|after:2:2|destroy:2"
                    "#,
                )
                .unwrap();
            assert!(supported);
        });
    }

    #[test]
    fn preserves_async_local_storage_in_promise_callbacks() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const { AsyncLocalStorage } = __canaryoBuiltins.async_hooks;
                    const storage = new AsyncLocalStorage();
                    globalThis.promiseStore = 0;
                    storage.run({ value: 42 }, () => {
                        Promise.resolve().then(() => promiseStore = storage.getStore()?.value);
                    });
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            assert_eq!(context.globals().get::<_, i32>("promiseStore").unwrap(), 42);
        });
    }

    #[test]
    fn pipes_data_through_node_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let streamed = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const { Readable, Writable, PassThrough, pipeline } = __canaryoBuiltins.stream;
                    const chunks = [];
                    let completed = false;
                    const source = new Readable();
                    const destination = new Writable({
                        write(chunk, _encoding, callback) {
                            chunks.push(Buffer.from(chunk).toString());
                            callback();
                        }
                    });
                    pipeline(source, new PassThrough(), destination, error => {
                        if (error) throw error;
                        completed = true;
                    });
                    source.push("Canar");
                    source.push("yo");
                    source.push(null);
                    completed && chunks.join("") === "Canaryo" &&
                        destination.writableFinished
                    "#,
                )
                .unwrap()
        });

        assert!(streamed);
    }

    #[test]
    fn iterates_readable_streams_and_event_emitters_asynchronously() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const streams = __canaryoBuiltins.stream;
                    const events = __canaryoBuiltins.events;
                    globalThis.asyncStreamOutput = "";
                    globalThis.asyncStreamDone = false;
                    (async () => {
                        for await (const chunk of streams.Readable.from(["can", "aryo"])) {
                            asyncStreamOutput += chunk.toString();
                        }
                        asyncStreamDone = true;
                    })();

                    const earlyStream = new streams.Readable();
                    const earlyIterator = earlyStream.iterator();
                    globalThis.earlyIteratorDone = false;
                    earlyIterator.return().then(result => {
                        earlyIteratorDone = result.done && earlyStream.destroyed;
                    });

                    const emitter = new events.EventEmitter();
                    const controller = new AbortController();
                    const eventIterator = events.on(emitter, "value", { signal: controller.signal });
                    globalThis.asyncEventValue = "";
                    globalThis.asyncEventAborted = false;
                    eventIterator.next().then(result => {
                        asyncEventValue = result.value.join(":");
                    });
                    emitter.emit("value", 42, "answer");
                    eventIterator.next().catch(error => {
                        asyncEventAborted = error.name === "AbortError" && error.cause === "stop";
                    });
                    controller.abort("stop");
                    "#,
                )
                .unwrap();

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..6 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("asyncStreamOutput")
                    .unwrap(),
                "canaryo"
            );
            for name in [
                "asyncStreamDone",
                "earlyIteratorDone",
                "asyncEventAborted",
            ] {
                assert!(context.globals().get::<_, bool>(name).unwrap(), "{name}");
            }
            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("asyncEventValue")
                    .unwrap(),
                "42:answer"
            );
        });
    }

    #[test]
    fn consumes_streams_into_common_value_types() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const { Readable } = __canaryoBuiltins.stream;
                    const consumers = __canaryoBuiltins["stream/consumers"];
                    globalThis.streamConsumersWorked = false;
                    Promise.all([
                        consumers.text(Readable.from(["can", "aryo"])),
                        consumers.json(Readable.from(['{"answer":', "42}"])),
                        consumers.buffer(Readable.from([new Uint8Array([1, 2]), Buffer.from([3])])),
                        consumers.arrayBuffer(Readable.from([Buffer.from("bytes")])),
                        consumers.blob(Readable.from(["blob", Buffer.from(" data")]))
                    ]).then(async ([text, json, buffer, arrayBuffer, blob]) => {
                        streamConsumersWorked = text === "canaryo" && json.answer === 42 &&
                            buffer.equals(Buffer.from([1, 2, 3])) &&
                            Buffer.from(arrayBuffer).toString() === "bytes" &&
                            blob instanceof Blob && blob.size === 9 &&
                            await blob.text() === "blob data";
                    });
                    "#,
                )
                .unwrap();

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..12 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .globals()
                    .get::<_, bool>("streamConsumersWorked")
                    .unwrap()
            );
        });
    }

    #[test]
    fn inspects_stream_state_and_configures_watermarks() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let supported = context
                .eval::<bool, _>(
                    r#"
                    const streams = __canaryoBuiltins.stream;
                    const originalBytes = streams.getDefaultHighWaterMark(false);
                    const originalObjects = streams.getDefaultHighWaterMark(true);
                    streams.setDefaultHighWaterMark(false, 1234);
                    streams.setDefaultHighWaterMark(true, 7);
                    const readable = new streams.Readable();
                    const objects = new streams.Readable({ objectMode: true });
                    const writable = new streams.Writable();
                    const byteDefaultsWorked = originalBytes === 65536 &&
                        originalObjects === 16 && readable.readableHighWaterMark === 1234 &&
                        objects.readableHighWaterMark === 7 && writable.writableHighWaterMark === 1234;
                    const initiallyReady = streams.isReadable(readable) &&
                        streams.isWritable(writable) && !streams.isDisturbed(readable) &&
                        !streams.isDestroyed(readable) && !streams.isErrored(readable);
                    readable.push("value");
                    readable.read();
                    const disturbed = streams.isDisturbed(readable);
                    const failure = new Error("destroyed");
                    readable.on("error", () => {});
                    readable.destroy(failure);
                    streams.setDefaultHighWaterMark(false, originalBytes);
                    streams.setDefaultHighWaterMark(true, originalObjects);
                    byteDefaultsWorked && initiallyReady && disturbed &&
                        streams.isDestroyed(readable) && streams.isErrored(readable) &&
                        !streams.isReadable(readable) &&
                        streams.isReadable({}) === null &&
                        streams._isUint8Array(Buffer.from([1])) &&
                        streams._uint8ArrayToBuffer(new Uint8Array([42]))[0] === 42
                    "#,
                )
                .unwrap();

            assert!(supported);
        });
    }

    #[test]
    fn applies_backpressure_to_piped_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let paused = context
                .eval::<bool, _>(
                    r#"
                    const { Readable, Writable, pipeline } = __canaryoBuiltins.stream;
                    const source = new Readable({ highWaterMark: 2 });
                    const writes = [];
                    let drains = 0;
                    let backpressureCompleted = false;
                    const destination = new Writable({
                        highWaterMark: 3,
                        write(chunk, _encoding, callback) {
                            writes.push(Buffer.from(chunk).toString());
                            setImmediate(callback);
                        }
                    });
                    destination.on("drain", () => drains++);
                    pipeline(source, destination, error => {
                        if (error) throw error;
                        backpressureCompleted = true;
                    });
                    source.push("ab");
                    source.push("cd");
                    source.push("ef");
                    source.push(null);
                    globalThis.backpressureState = () => ({
                        writes: writes.join(""),
                        drains,
                        completed: backpressureCompleted,
                        finished: destination.writableFinished,
                        length: destination.writableLength
                    });
                    source._paused && destination.writableNeedDrain && writes.join("") === "ab"
                    "#,
                )
                .unwrap();
            assert!(paused);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..6 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .eval::<bool, _>(
                        r#"
                        const state = backpressureState();
                        state.writes === "abcdef" && state.drains >= 1 &&
                            state.completed && state.finished && state.length === 0
                        "#,
                    )
                    .unwrap()
            );
        });
    }

    #[test]
    fn exposes_main_thread_worker_shims() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let initialized = context
                .eval::<bool, _>(
                    r#"
                    const workers = __canaryoBuiltins.worker_threads;
                    workers.setEnvironmentData("canaryo", 42);
                    const channel = new workers.MessageChannel();
                    globalThis.workerMessage = null;
                    channel.port2.on("message", value => { workerMessage = value; });
                    channel.port1.postMessage({ runtime: "canaryo" });
                    let unsupportedWorker = false;
                    try { new workers.Worker("worker.js"); }
                    catch (error) { unsupportedWorker = error.code === "ERR_WORKER_UNSUPPORTED_OPERATION"; }
                    workers.isMainThread && workers.threadId === 0 && workers.parentPort === null &&
                        workers.getEnvironmentData("canaryo") === 42 && unsupportedWorker
                    "#,
                )
                .unwrap();
            assert!(initialized);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            run_timers.call::<_, i64>(()).unwrap();

            assert!(
                context
                    .eval::<bool, _>("workerMessage.runtime === 'canaryo'")
                    .unwrap()
            );
        });
    }

    #[test]
    fn supports_web_and_node_url_apis() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const url = new URL("/users?id=1&id=2", "https://example.com/base");
                    const relative = new URL("../teams", "https://example.com/api/users/");
                    url.searchParams.append("active", "true");
                    const nodeUrl = __canaryoBuiltins.url;
                    url.origin === "https://example.com" &&
                        url.pathname === "/users" &&
                        url.searchParams.getAll("id").join(",") === "1,2" &&
                        url.href === "https://example.com/users?id=1&id=2&active=true" &&
                        relative.href === "https://example.com/api/teams" &&
                        nodeUrl.fileURLToPath(nodeUrl.pathToFileURL(".")) === process.cwd()
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_common_node_process_metadata() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &["argument".into()]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    let warned = false;
                    process.once("warning", warning => { warned = warning.message === "careful"; });
                    process.emitWarning("careful");
                    const elapsed = process.hrtime();
                    process.pid > 0 && process.arch === __canaryoOsInfo.arch &&
                        process.platform === __canaryoOsInfo.platform &&
                        process.argv[2] === "argument" && typeof process.execPath === "string" &&
                        process.uptime() >= 0 && elapsed.length === 2 &&
                        typeof process.hrtime.bigint() === "bigint" && warned &&
                        process.getBuiltinModule("node:path") === __canaryoBuiltins.path
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn resolves_dns_and_identifies_ip_addresses() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.callbackLookup = false;
                    globalThis.allLookup = false;
                    globalThis.promiseLookup = false;
                    const dns = __canaryoBuiltins.dns;
                    dns.lookup("127.0.0.1", (error, address, family) => {
                        callbackLookup = !error && address === "127.0.0.1" && family === 4;
                    });
                    dns.lookup("::1", { family: 6, all: true }, (error, addresses) => {
                        allLookup = !error && addresses.length === 1 &&
                            addresses[0].address === "::1" && addresses[0].family === 6;
                    });
                    __canaryoBuiltins["dns/promises"].lookup("127.0.0.1").then(result => {
                        promiseLookup = result.address === "127.0.0.1" && result.family === 4;
                    });
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}
            context
                .eval::<bool, _>(
                    r#"
                    callbackLookup && allLookup && promiseLookup &&
                        __canaryoBuiltins.net.isIP("127.0.0.1") === 4 &&
                        __canaryoBuiltins.net.isIPv4("127.0.0.1") &&
                        __canaryoBuiltins.net.isIPv6("2001:db8::1") &&
                        __canaryoBuiltins.net.isIP("not-an-address") === 0
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_node_module_helpers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(BOOTSTRAP).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const Module = __canaryoBuiltins.module;
                    const localRequire = Module.createRequire(__filename);
                    Module === Module.Module && Module.isBuiltin("node:http") &&
                        !Module.isBuiltin("left-pad") &&
                        Module.builtinModules.includes("fs/promises") &&
                        localRequire("node:path") === __canaryoBuiltins.path &&
                        Module._cache === localRequire.cache
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_host_operating_system_metadata() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const os = __canaryoBuiltins.os;
                    os.platform() === process.platform && os.arch() === process.arch &&
                        typeof os.type() === "string" && os.type().length > 0 &&
                        typeof os.tmpdir() === "string" && os.tmpdir().length > 0 &&
                        typeof os.homedir() === "string" && typeof os.hostname() === "string" &&
                        ["LE", "BE"].includes(os.endianness()) &&
                        os.availableParallelism() >= 1 &&
                        os.cpus().length === os.availableParallelism() &&
                        os.uptime() >= 0 && typeof os.userInfo().username === "string"
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn uses_platform_specific_path_semantics() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const path = __canaryoBuiltins.path;
                    const shared = path.win32.normalize("C:\\one\\..\\two") === "C:\\two" &&
                        path.win32.join("C:\\one", "two", "..", "file.js") === "C:\\one\\file.js" &&
                        path.win32.dirname("C:\\one\\file.js") === "C:\\one" &&
                        path.win32.parse("C:\\one\\file.js").root === "C:\\" &&
                        path.posix.normalize("/one/../two") === "/two" &&
                        path.posix.join("/one", "two") === "/one/two" &&
                        path.posix.delimiter === ":";
                    shared && (process.platform === "win32" ? path.sep === "\\" : path.sep === "/")
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn converts_between_callback_and_promise_apis() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.promisifiedResult = 0;
                    globalThis.callbackifiedResult = 0;
                    const util = __canaryoBuiltins.util;
                    util.promisify((left, right, callback) => callback(null, left + right))(20, 22)
                        .then(value => { promisifiedResult = value; });
                    util.callbackify(async value => value * 2)(21, (error, value) => {
                        if (!error) callbackifiedResult = value;
                    });
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}
            context
                .eval::<bool, _>(
                    r#"
                    promisifiedResult === 42 && callbackifiedResult === 42 &&
                        __canaryoBuiltins.util.promisify.custom === Symbol.for("nodejs.util.promisify.custom") &&
                        __canaryoBuiltins.util.stripVTControlCharacters("\u001b[31mred\u001b[0m") === "red"
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }
}
