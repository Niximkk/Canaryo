use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    io::{self, IsTerminal, Read, Write},
    net::{IpAddr, Shutdown, TcpStream, ToSocketAddrs},
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use base64::Engine;
use ring::rand::SecureRandom;
use rquickjs::{
    Array, CatchResultExt, Context, Function, Module, Object, Promise, Runtime, TypedArray,
};
use subtle::ConstantTimeEq;

use crate::{esm, http, modules};

#[cfg(test)]
const BOOTSTRAP: &str = r#"
globalThis.global = globalThis;
Object.defineProperties(globalThis, {
    __canaryoActiveServer: { value: null, writable: true, configurable: true },
    __canaryoPendingServerStart: { value: null, writable: true, configurable: true }
});
(() => {
function Console(stdout, stderr = stdout) {
    if (stdout && stdout.stdout) {
        stderr = stdout.stderr || stdout.stdout;
        stdout = stdout.stdout;
    }
    this._stdout = stdout;
    this._stderr = stderr;
    this._times = new Map();
    this._counts = new Map();
    this._groupIndent = "";
}
Console.prototype.log = Console.prototype.info = Console.prototype.debug = function (...values) {
    this._stdout.write(this._groupIndent + values.map(formatValue).join(" ") + "\n");
};
Console.prototype.error = Console.prototype.warn = function (...values) {
    this._stderr.write(this._groupIndent + values.map(formatValue).join(" ") + "\n");
};
Console.prototype.assert = function (condition, ...values) {
    if (!condition) this.error(`Assertion failed${values.length ? `: ${values.map(formatValue).join(" ")}` : ""}`);
};
Console.prototype.clear = function () {};
Console.prototype.count = function (label = "default") {
    label = String(label);
    const value = (this._counts.get(label) || 0) + 1;
    this._counts.set(label, value);
    this.log(`${label}: ${value}`);
};
Console.prototype.countReset = function (label = "default") { this._counts.delete(String(label)); };
Console.prototype.dir = function (value) { this.log(formatValue(value)); };
Console.prototype.dirxml = Console.prototype.dir;
Console.prototype.group = Console.prototype.groupCollapsed = function (...label) {
    if (label.length) this.log(...label);
    this._groupIndent += "  ";
};
Console.prototype.groupEnd = function () { this._groupIndent = this._groupIndent.slice(0, -2); };
Console.prototype.table = function (value) { this.dir(value); };
Console.prototype.time = function (label = "default") { this._times.set(String(label), __canaryoPerformanceNow()); };
Console.prototype.timeLog = function (label = "default", ...values) {
    label = String(label);
    if (!this._times.has(label)) return;
    this.log(`${label}: ${(__canaryoPerformanceNow() - this._times.get(label)).toFixed(3)}ms`, ...values);
};
Console.prototype.timeEnd = function (label = "default") {
    label = String(label);
    this.timeLog(label);
    this._times.delete(label);
};
Console.prototype.timeStamp = Console.prototype.profile = Console.prototype.profileEnd = function () {};
Console.prototype.trace = function (...values) { this.error("Trace:", ...values); };
Console.prototype.createTask = function () { return { run: callback => callback() }; };
Console.prototype.context = function () { return this; };

const hostConsole = new Console(
    { write: value => __canaryoWrite(String(value)) },
    { write: value => __canaryoWriteError(String(value)) }
);
for (const name of [
    "log", "info", "debug", "error", "warn", "assert", "clear", "count", "countReset",
    "dir", "dirxml", "group", "groupCollapsed", "groupEnd", "table", "time", "timeLog",
    "timeEnd", "timeStamp", "trace", "profile", "profileEnd", "createTask", "context"
]) {
    hostConsole[name] = Console.prototype[name].bind(hostConsole);
}
hostConsole.Console = Console;
globalThis.console = Object.freeze(hostConsole);

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
function OutgoingMessage() {}
function ServerResponse() {}
function Socket() {}
function Server() {}
function HttpsServer() {}

function lazyMessageValue(message, name, create) {
    const value = create();
    Object.defineProperty(message, name, {
        value,
        writable: true,
        enumerable: true,
        configurable: true
    });
    return value;
}
Object.defineProperties(IncomingMessage.prototype, {
    rawHeaders: {
        configurable: true,
        get() {
            return lazyMessageValue(this, "rawHeaders", () =>
                Object.entries(this.headers || {}).flatMap(([name, value]) => [name, value])
            );
        },
        set(value) { lazyMessageValue(this, "rawHeaders", () => value); }
    },
    trailers: {
        configurable: true,
        get() { return lazyMessageValue(this, "trailers", () => Object.create(null)); },
        set(value) { lazyMessageValue(this, "trailers", () => value); }
    },
    rawTrailers: {
        configurable: true,
        get() {
            return lazyMessageValue(this, "rawTrailers", () =>
                Object.entries(this.trailers || {}).flatMap(([name, value]) => [name, value])
            );
        },
        set(value) { lazyMessageValue(this, "rawTrailers", () => value); }
    }
});

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

function validateHeaderName(name, label = "Header name") {
    if (typeof name !== "string" || !/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(name)) {
        const error = new TypeError(`${label} must be a valid HTTP token [${name}]`);
        error.code = "ERR_INVALID_HTTP_TOKEN";
        throw error;
    }
}
function validateHeaderValue(name, value) {
    if (value === undefined) {
        const error = new TypeError(`Invalid value \"undefined\" for header \"${name}\"`);
        error.code = "ERR_HTTP_INVALID_HEADER_VALUE";
        throw error;
    }
    const values = Array.isArray(value) ? value : [value];
    for (const item of values) {
        if (/[\0-\x08\x0a-\x1f\x7f]/.test(String(item))) {
            const error = new TypeError(`Invalid character in header content [${name}]`);
            error.code = "ERR_INVALID_CHAR";
            throw error;
        }
    }
}

let nextResponseHeaderState = 1;
Object.defineProperty(ServerResponse.prototype, "__canaryoHeaderState", {
    value: 0,
    writable: true
});
ServerResponse.prototype.setHeader = function(name, value) {
    validateHeaderName(name);
    validateHeaderValue(name, value);
    const key = String(name).toLowerCase();
    this.__canaryoHeaders[key] = Array.isArray(value)
        ? value.map(String)
        : String(value);
    this.__canaryoHeaderState = nextResponseHeaderState++;
    return this;
};
ServerResponse.prototype.appendHeader = function(name, value) {
    validateHeaderName(name);
    validateHeaderValue(name, value);
    const key = String(name).toLowerCase();
    const incoming = Array.isArray(value) ? value.map(String) : [String(value)];
    const current = this.__canaryoHeaders[key];
    this.__canaryoHeaders[key] = current === undefined
        ? incoming
        : (Array.isArray(current) ? current : [current]).concat(incoming);
    this.__canaryoHeaderState = nextResponseHeaderState++;
    return this;
};
ServerResponse.prototype.setHeaders = function(headers) {
    if (!headers || typeof headers.entries !== "function") {
        throw new TypeError("headers must be a Map or Headers object");
    }
    for (const [name, value] of headers.entries()) this.setHeader(name, value);
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
    const key = String(name).toLowerCase();
    if (Object.prototype.hasOwnProperty.call(this.__canaryoHeaders, key)) {
        delete this.__canaryoHeaders[key];
        this.__canaryoHeaderState = nextResponseHeaderState++;
    }
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
    if (typeof chunk === "string" && (!response.__canaryoBody || response.__canaryoBody.length === 0)) {
        response.__canaryoTextBody += chunk;
        return;
    }
    if (response.__canaryoTextBody.length > 0) {
        response.__canaryoBody ||= [];
        response.__canaryoBody.push(Buffer.from(response.__canaryoTextBody));
        response.__canaryoTextBody = "";
    }
    response.__canaryoBody ||= [];
    response.__canaryoBody.push(Buffer.from(chunk));
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
    if (this._events && this._events.finish && this._events.finish.length) {
        this.emit("finish");
    }
    return this;
};

function ensureHttpEventPrototypes() {
    const EventEmitter = __canaryoBuiltins.events.EventEmitter;
    const Stream = __canaryoBuiltins.stream;
    if (!(OutgoingMessage.prototype instanceof EventEmitter)) {
        Object.setPrototypeOf(OutgoingMessage.prototype, EventEmitter.prototype);
    }
    for (const constructor of [ServerResponse, ClientRequest]) {
        if (!(constructor.prototype instanceof OutgoingMessage)) {
            Object.setPrototypeOf(constructor.prototype, OutgoingMessage.prototype);
        }
    }
    for (const constructor of [IncomingMessage, Socket, Server, HttpsServer]) {
        if (!(constructor.prototype instanceof EventEmitter)) {
            Object.setPrototypeOf(constructor.prototype, EventEmitter.prototype);
        }
    }
    if (!IncomingMessage.prototype.pipe) IncomingMessage.prototype.pipe = Stream.prototype.pipe;
    if (!IncomingMessage.prototype.unpipe) IncomingMessage.prototype.unpipe = Stream.prototype.unpipe;
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
    validateHeaderName(name);
    validateHeaderValue(name, value);
    this._headers[String(name).toLowerCase()] = Array.isArray(value) ? value.join(", ") : String(value);
};
ClientRequest.prototype.appendHeader = function(name, value) {
    if (this.headersSent) throw new Error("Cannot append headers after they are sent");
    validateHeaderName(name);
    validateHeaderValue(name, value);
    const key = String(name).toLowerCase();
    const incoming = Array.isArray(value) ? value.join(", ") : String(value);
    this._headers[key] = this._headers[key] === undefined
        ? incoming
        : `${this._headers[key]}, ${incoming}`;
    return this;
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
    __canaryoMarkServerBusy();
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
    __canaryoHttpRequestWrite(this._requestId, bytes);
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
ClientRequest.prototype.setNoDelay = function() { return this; };
ClientRequest.prototype.setSocketKeepAlive = function() { return this; };

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
        const event = __canaryoHttpRequestPoll();
        if (event === undefined || event === null) break;
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
                const key = name.toLowerCase();
                if (key === "set-cookie") {
                    if (!response.headers[key]) response.headers[key] = [];
                    response.headers[key].push(value);
                } else if (response.headers[key] !== undefined) {
                    response.headers[key] += `, ${value}`;
                } else {
                    response.headers[key] = value;
                }
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
                request._response.__canaryoDeliverChunk(Buffer.from(event.body));
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
let maxIdleHttpParsers = 1000;

function optimizeExpressResponse(listener) {
    const responsePrototype = listener?.response;
    const settings = listener?.settings;
    if (!responsePrototype || !settings
        || Object.prototype.hasOwnProperty.call(responsePrototype, "json")
        || Object.prototype.hasOwnProperty.call(responsePrototype, "send")) return listener;

    const originalJson = responsePrototype.json;
    const originalSend = responsePrototype.send;
    if (typeof originalJson !== "function" || originalJson.name !== "json"
        || typeof originalSend !== "function" || originalSend.name !== "send") return listener;

    const jsonEscape = settings["json escape"];
    const jsonReplacer = settings["json replacer"];
    const jsonSpaces = settings["json spaces"];
    const etagSetting = settings.etag;
    const etagFunction = settings["etag fn"];
    if (jsonEscape || jsonReplacer !== undefined || jsonSpaces !== undefined) return listener;
    const cacheDefaultEtag = etagSetting === "weak" && etagFunction?.name === "generateETag";
    const jsonMetadata = new Map();
    const nativeSetHeader = ServerResponse.prototype.setHeader;
    const nativeGetHeader = ServerResponse.prototype.getHeader;
    const nativeRemoveHeader = ServerResponse.prototype.removeHeader;
    const nativeEnd = ServerResponse.prototype.end;
    const cachedJsonEnvelope = value => {
        if (!value || Array.isArray(value)
            || Object.getPrototypeOf(value) !== Object.prototype
            || typeof value.toJSON === "function") return undefined;
        const keys = Object.keys(value);
        if (keys.length !== 1 || keys[0] !== "body") return undefined;
        const inner = value.body;
        if (!inner || Array.isArray(inner)
            || Object.getPrototypeOf(inner) !== Object.prototype
            || typeof inner.toJSON === "function") return undefined;
        const snapshot = inner.__canaryoCanonicalFlatJsonSnapshot;
        if (!Array.isArray(snapshot)) return undefined;
        const innerKeys = Object.keys(inner);
        if (innerKeys.length * 2 !== snapshot.length) return undefined;
        for (let index = 0; index < innerKeys.length; index++) {
            if (innerKeys[index] !== snapshot[index * 2]
                || inner[innerKeys[index]] !== snapshot[index * 2 + 1]) return undefined;
        }
        const encodedId = inner.__canaryoCanonicalEnvelopeId;
        const length = inner.__canaryoCanonicalEnvelopeLength;
        const etag = inner.__canaryoCanonicalEnvelopeEtag;
        if (typeof encodedId === "number" && typeof length === "number"
            && typeof etag === "string") return { encodedId, length, etag };
        return undefined;
    };
    const directHeaders = responsePrototype.setHeader === nativeSetHeader
        && responsePrototype.getHeader === nativeGetHeader
        && responsePrototype.removeHeader === nativeRemoveHeader;
    const originalSet = responsePrototype.set;
    if (directHeaders && typeof originalSet === "function" && originalSet.name === "header") {
        const normalizedHeaders = new Map();
        responsePrototype.set = responsePrototype.header = function canaryoExpressSet(field, value) {
            if (arguments.length !== 2 || this.setHeader !== nativeSetHeader) {
                return originalSet.apply(this, arguments);
            }
            const key = String(field);
            const normalized = normalizedHeaders.get(key);
            if (normalized && normalized.input === value && value !== Object(value)) {
                this.__canaryoHeaders[normalized.name] = normalized.value;
                let state = normalized.transitions.get(this.__canaryoHeaderState);
                if (state === undefined) {
                    state = nextResponseHeaderState++;
                    normalized.transitions.set(this.__canaryoHeaderState, state);
                }
                this.__canaryoHeaderState = state;
                return this;
            }
            const previousState = this.__canaryoHeaderState;
            originalSet.call(this, field, value);
            if (value !== Object(value) && normalizedHeaders.size < 128) {
                const name = String(field).toLowerCase();
                const transitions = new Map();
                transitions.set(previousState, this.__canaryoHeaderState);
                normalizedHeaders.set(key, {
                    input: value,
                    name,
                    value: this.__canaryoHeaders[name],
                    transitions
                });
            }
            return this;
        };
    }

    const finishJson = (response, output, encodedId) => {
        const finishListeners = response._events && response._events.finish;
        if (response.end !== nativeEnd || (finishListeners && finishListeners.length)) {
            response.end(output);
            return;
        }
        if (output !== undefined && output !== null) response.__canaryoTextBody = String(output);
        if (encodedId !== undefined) response.__canaryoCachedBodyId = encodedId;
        response.headersSent = true;
        response.writableEnded = true;
        response.writableFinished = true;
        response.finished = true;
    };

    responsePrototype.json = function canaryoExpressJson(value) {
        if (settings["json escape"] !== jsonEscape
            || settings["json replacer"] !== jsonReplacer
            || settings["json spaces"] !== jsonSpaces
            || settings.etag !== etagSetting
            || settings["etag fn"] !== etagFunction
            || this.setHeader !== nativeSetHeader
            || this.getHeader !== nativeGetHeader
            || this.removeHeader !== nativeRemoveHeader
            || (directHeaders
                ? this.__canaryoHeaders["content-type"] !== undefined
                    || this.__canaryoHeaders.etag !== undefined
                : this.getHeader("content-type") !== undefined
                    || this.getHeader("etag") !== undefined)) {
            return originalJson.call(this, value);
        }

        const envelope = cachedJsonEnvelope(value);
        const body = envelope ? null : JSON.stringify(value);
        const headers = this.__canaryoHeaders;
        if (directHeaders) headers["content-type"] = "application/json; charset=utf-8";
        else this.setHeader("content-type", "application/json; charset=utf-8");
        if (!envelope && body === undefined) {
            finishJson(this);
            return this;
        }

        const cacheable = !envelope && cacheDefaultEtag && body.length <= 65536;
        let metadata = envelope || (cacheable ? jsonMetadata.get(body) : undefined);
        if (!metadata) {
            if (cacheDefaultEtag) {
                const generated = __canaryoExpressWeakEtag(body);
                metadata = {
                    length: generated[0],
                    etag: generated[1],
                    encodedId: cacheable ? __canaryoCacheResponseBody(body) : undefined
                };
            } else {
                const bytes = typeof etagFunction === "function" ? Buffer.from(body) : null;
                metadata = {
                    length: bytes ? bytes.length : Buffer.byteLength(body, "utf8"),
                    etag: bytes ? etagFunction(bytes) : undefined,
                    encodedId: cacheable ? __canaryoCacheResponseBody(body) : undefined
                };
            }
            if (cacheable) {
                if (jsonMetadata.size >= 32) {
                    const oldestKey = jsonMetadata.keys().next().value;
                    const oldest = jsonMetadata.get(oldestKey);
                    if (oldest?.encodedId !== undefined) {
                        __canaryoReleaseResponseBody(oldest.encodedId);
                    }
                    jsonMetadata.delete(oldestKey);
                }
                jsonMetadata.set(body, metadata);
            }
        }
        if (directHeaders) {
            headers["content-length"] = String(metadata.length);
            if (metadata.etag) headers.etag = metadata.etag;
        } else {
            this.setHeader("content-length", metadata.length);
            if (metadata.etag) this.setHeader("etag", metadata.etag);
        }
        const requestHeaders = this.req.headers;
        if ((requestHeaders["if-none-match"] !== undefined
            || requestHeaders["if-modified-since"] !== undefined)
            && this.req.fresh) this.statusCode = 304;

        let output = body;
        if (this.statusCode === 204 || this.statusCode === 304) {
            if (directHeaders) {
                delete headers["content-type"];
                delete headers["content-length"];
                delete headers["transfer-encoding"];
            } else {
                this.removeHeader("content-type");
                this.removeHeader("content-length");
                this.removeHeader("transfer-encoding");
            }
            output = "";
        } else if (this.statusCode === 205) {
            if (directHeaders) {
                headers["content-length"] = "0";
                delete headers["transfer-encoding"];
            } else {
                this.setHeader("content-length", "0");
                this.removeHeader("transfer-encoding");
            }
            output = "";
        }

        if (this.req.method === "HEAD") finishJson(this);
        else finishJson(
            this,
            output === null ? "" : output,
            (envelope ? output === null : output === body) ? metadata.encodedId : undefined
        );
        return this;
    };
    return listener;
}

function fastDefaultJsonParser(original) {
    if (!original.__canaryoDefaultJsonParser) return original;
    function canaryoDefaultJsonParser(request, response, next) {
        if (request.complete) return next();
        const contentType = String(request.headers["content-type"] || "").toLowerCase();
        const encoding = String(request.headers["content-encoding"] || "identity").toLowerCase();
        if (!/^application\/(?:json|[^;]+\+json)(?:\s*;|$)/.test(contentType)
            || encoding !== "identity"
            || (/charset\s*=/.test(contentType) && !/charset\s*=\s*"?utf-?8"?(?:\s*;|\s*$)/.test(contentType))) {
            return original(request, response, next);
        }

        const declaredLength = Number(request.headers["content-length"] || 0);
        if (declaredLength > 102400) return original(request, response, next);
        const events = request._events;
        if (request.__canaryoPaused || (events && (
            events.data || events.end || events.aborted || events.error || events.close
        ))) return original(request, response, next);
        request.__canaryoDirectJsonNext = next;
    }
    Object.defineProperty(canaryoDefaultJsonParser, "__canaryoDirectJsonParser", {
        value: true
    });
    return canaryoDefaultJsonParser;
}

function optimizeExpressRouter(listener) {
    const router = listener?.router;
    if (!router || !Array.isArray(router.stack) || router.__canaryoOptimized) return listener;

    const originalHandle = router.handle;
    if (typeof originalHandle !== "function") return listener;
    const directResponseHeaders = listener.response?.setHeader === ServerResponse.prototype.setHeader;
    const stack = router.stack;
    const stackLength = stack.length;
    const normalizePath = value => {
        let path = String(value || "/");
        if (!router.strict && path.length > 1) path = path.replace(/\/+$/, "");
        return router.caseSensitive ? path : path.toLowerCase();
    };
    const plans = new Map();
    const candidates = new Map();
    const hasTerminalNext = handle => {
        if (handle.length !== 3) return false;
        const source = Function.prototype.toString.call(handle).trim();
        return !source.startsWith("async ")
            && /(?:^|[^\w$])next\s*\(\s*\)\s*;?\s*}$/.test(source);
    };

    for (const layer of stack) {
        const route = layer.route;
        if (!route || typeof route.path !== "string" || /[:*?()[\]]/.test(route.path)) continue;
        for (const method of Object.keys(route.methods || {})) {
            if (method === "_all") continue;
            const normalizedMethod = method.toUpperCase();
            const key = `${normalizedMethod}\0${normalizePath(route.path)}`;
            candidates.set(key, { method: normalizedMethod, path: route.path });
            if (normalizedMethod === "GET" && !route.methods.head) {
                candidates.set(`HEAD\0${normalizePath(route.path)}`, {
                    method: "HEAD",
                    path: route.path
                });
            }
        }
    }

    for (const [key, candidate] of candidates) {
        const steps = [];
        let safe = true;
        let matchedRoute = false;

        for (const layer of stack) {
            if (layer.route) {
                if (normalizePath(layer.route.path) !== normalizePath(candidate.path)
                    || !layer.route._handlesMethod(candidate.method)) continue;
                const method = candidate.method === "HEAD" && !layer.route.methods.head
                    ? "get"
                    : candidate.method.toLowerCase();
                const handlers = layer.route.stack.filter(item => !item.method || item.method === method);
                const terminal = handlers[handlers.length - 1]?.handle;
                if (typeof terminal !== "function" || terminal.length >= 3) {
                    safe = false;
                    break;
                }
                matchedRoute = true;
                for (let index = 0; index < handlers.length; index++) {
                    const handle = fastDefaultJsonParser(handlers[index].handle);
                    steps.push({
                        handle,
                        expectsError: handle.length === 4,
                        terminalNext: hasTerminalNext(handle),
                        directJsonParser: Boolean(handle.__canaryoDirectJsonParser),
                        route: layer.route,
                        routeStart: index === 0
                    });
                }
                continue;
            }

            if (layer.slash) {
                const handle = fastDefaultJsonParser(layer.handle);
                steps.push({
                    handle,
                    expectsError: handle.length === 4,
                    terminalNext: hasTerminalNext(handle),
                    directJsonParser: Boolean(handle.__canaryoDirectJsonParser),
                    route: null,
                    routeStart: false
                });
                continue;
            }

            let matches = false;
            try { matches = layer.match(candidate.path); }
            finally {
                layer.params = undefined;
                layer.path = undefined;
            }
            if (matches) {
                safe = false;
                break;
            }
        }

        if (safe && matchedRoute) {
            const direct = steps.length === 1
                && steps[0].route
                && steps[0].handle.length < 3
                ? steps[0]
                : null;
            Object.defineProperty(steps, "direct", { value: direct });
            plans.set(key, steps);
        }
    }

    const materializeBridgeDefaults = (request, response) => {
        Object.assign(request, {
            method: request.method,
            url: request.url,
            httpVersion: request.httpVersion,
            httpVersionMajor: request.httpVersionMajor,
            httpVersionMinor: request.httpVersionMinor,
            aborted: request.aborted,
            complete: request.complete,
            destroyed: request.destroyed,
            readable: request.readable,
            readableEnded: request.readableEnded,
            __canaryoPaused: request.__canaryoPaused
        });
        Object.assign(response, {
            statusCode: response.statusCode,
            statusMessage: response.statusMessage,
            headersSent: response.headersSent,
            writableEnded: response.writableEnded,
            writableFinished: response.writableFinished,
            finished: response.finished,
            destroyed: response.destroyed,
            __canaryoTextBody: response.__canaryoTextBody
        });
    };
    if (plans.size === 0) {
        function canaryoExpressFallback(request, response) {
            materializeBridgeDefaults(request, response);
            return listener(request, response);
        }
        Object.defineProperties(canaryoExpressFallback, {
            __canaryoRequestPrototype: { value: listener.request },
            __canaryoResponsePrototype: { value: listener.response }
        });
        return canaryoExpressFallback;
    }
    Object.defineProperty(router, "__canaryoOptimized", { value: true });
    let cachedMethod;
    let cachedUrl;
    let cachedPlan;
    const planFor = request => {
        if (router.stack.length !== stackLength) return null;
        const method = request.method;
        const url = request.url;
        if (method === cachedMethod && url === cachedUrl) return cachedPlan;
        const queryIndex = url.indexOf("?");
        const pathname = normalizePath(queryIndex < 0 ? url : url.slice(0, queryIndex));
        cachedMethod = method;
        cachedUrl = url;
        cachedPlan = plans.get(`${method}\0${pathname}`) || null;
        return cachedPlan;
    };

    const runPlan = (plan, request, response, done) => {
        let index = 0;
        let activeRoute = null;
        let skippedRoute = null;
        let deferNext = false;
        let deferred = false;
        let deferredError;
        request.next = next;
        request.baseUrl = request.baseUrl || "";
        request.originalUrl = request.originalUrl || request.url;
        next();

        function next(error) {
            if (deferNext) {
                deferred = true;
                deferredError = error;
                return;
            }
            dispatch: while (true) {
                if (error === "router") return setImmediate(done, null);
                if (error === "route") {
                    skippedRoute = activeRoute;
                    error = null;
                }

                while (index < plan.length) {
                    const step = plan[index++];
                    if (skippedRoute && step.route === skippedRoute) continue;
                    skippedRoute = null;
                    if (step.routeStart) {
                        request.params = {};
                        request.route = step.route;
                    }
                    activeRoute = step.route || null;
                    const handle = step.handle;
                    if (Boolean(error) !== step.expectsError) continue;
                    try {
                        deferNext = step.terminalNext && !error;
                        const returned = error
                            ? handle(error, request, response, next)
                            : handle(request, response, next);
                        deferNext = false;
                        if (deferred) {
                            deferred = false;
                            error = deferredError;
                            deferredError = undefined;
                            continue dispatch;
                        }
                        if (returned && typeof returned.then === "function") {
                            returned.then(undefined, rejection => next(
                                rejection || new Error("Rejected promise")
                            ));
                        }
                        return;
                    } catch (thrown) {
                        deferNext = false;
                        deferred = false;
                        deferredError = undefined;
                        error = thrown;
                    }
                }
                return done(error);
            }
        }
    };
    const errorStatus = error => {
        const status = Number(error && (error.statusCode || error.status));
        return Number.isInteger(status) && status >= 400 && status <= 599 ? status : 500;
    };

    // Rust can dispatch exact, synchronous Express chains without paying for the
    // JavaScript router loop on every request.  Keep the eligibility rules strict:
    // every middleware must end in next(), one route handler must terminate the
    // chain, and promise-producing handlers remain on Express' regular path.
    const nativePlans = [];
    for (const [key, plan] of plans) {
        let terminal = false;
        let safe = true;
        for (const step of plan) {
            const source = Function.prototype.toString.call(step.handle).trim();
            const asynchronous = step.handle.constructor?.name === "AsyncFunction"
                || /\bPromise\b|\.then\s*\(/.test(source);
            if (asynchronous) {
                safe = false;
                break;
            }
            if (step.expectsError) continue;
            if (terminal) {
                safe = false;
                break;
            }
            if (step.directJsonParser) continue;
            if (step.handle.length < 3) terminal = true;
            else if (!step.terminalNext) {
                safe = false;
                break;
            }
        }
        if (safe && terminal) nativePlans.push({ key, steps: plan });
    }
    const nativeNext = () => {};
    if (nativePlans.length > 0) {
        for (const method of [
            "copyWithin", "fill", "pop", "push", "reverse", "shift", "sort", "splice", "unshift"
        ]) {
            const mutate = stack[method];
            Object.defineProperty(stack, method, {
                configurable: true,
                writable: true,
                value(...arguments_) {
                    __canaryoInvalidateNativeExpressPlans();
                    return Reflect.apply(mutate, this, arguments_);
                }
            });
        }
    }

    router.handle = function canaryoExpressHandle(request, response, done) {
        const plan = planFor(request);
        if (!plan) return originalHandle.call(router, request, response, done);
        return runPlan(plan, request, response, done);
    };

    function canaryoExpressApplication(request, response) {
        const plan = planFor(request);
        if (!plan) {
            // Express replaces the prototypes in app.handle. Materialize the defaults that the
            // native bridge normally inherits from its per-server templates before that happens.
            materializeBridgeDefaults(request, response);
            return listener(request, response);
        }

        if (listener.settings["x-powered-by"] !== false) {
            if (directResponseHeaders) response.__canaryoHeaders["x-powered-by"] = "Express";
            else response.setHeader("X-Powered-By", "Express");
        }
        response.locals = Object.create(null);
        if (plan.direct) {
            request.next = undefined;
            request.baseUrl = request.baseUrl || "";
            request.originalUrl = request.originalUrl || request.url;
            request.params = {};
            request.route = plan.direct.route;
            try {
                const returned = plan.direct.handle(request, response);
                if (returned && typeof returned.then === "function") {
                    returned.then(undefined, error => {
                        if (response.writableEnded) return;
                        response.statusCode = errorStatus(error);
                        response.setHeader("content-type", "text/plain; charset=utf-8");
                        response.end(error ? "Internal Server Error" : "Rejected promise");
                    });
                }
            } catch (error) {
                if (!response.writableEnded) {
                    response.statusCode = errorStatus(error);
                    response.setHeader("content-type", "text/plain; charset=utf-8");
                    response.end("Internal Server Error");
                }
            }
            return;
        }
        return runPlan(plan, request, response, error => {
            if (response.writableEnded) return;
            response.statusCode = error ? errorStatus(error) : 404;
            response.setHeader("content-type", "text/plain; charset=utf-8");
            response.end(error ? "Internal Server Error" : `Cannot ${request.method} ${request.url}`);
        });
    }
    Object.defineProperties(canaryoExpressApplication, {
        __canaryoRequestPrototype: { value: listener.request },
        __canaryoResponsePrototype: { value: listener.response },
        __canaryoNativePlans: { value: nativePlans },
        __canaryoNativeNext: { value: nativeNext },
        __canaryoNativeDirectHeaders: { value: directResponseHeaders },
        __canaryoNativeXPoweredBy: { value: listener.settings["x-powered-by"] !== false }
    });
    return canaryoExpressApplication;
}

const httpModule = Object.freeze({
    IncomingMessage,
    OutgoingMessage,
    ServerResponse,
    ClientRequest,
    Server,
    Agent,
    globalAgent,
    METHODS: ["GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH"],
    STATUS_CODES: {
        100: "Continue", 101: "Switching Protocols", 102: "Processing", 103: "Early Hints",
        200: "OK", 201: "Created", 202: "Accepted", 203: "Non-Authoritative Information",
        204: "No Content", 205: "Reset Content", 206: "Partial Content", 207: "Multi-Status",
        208: "Already Reported", 226: "IM Used", 300: "Multiple Choices", 301: "Moved Permanently",
        302: "Found", 303: "See Other", 304: "Not Modified", 305: "Use Proxy",
        307: "Temporary Redirect", 308: "Permanent Redirect", 400: "Bad Request", 401: "Unauthorized",
        402: "Payment Required", 403: "Forbidden", 404: "Not Found", 405: "Method Not Allowed",
        406: "Not Acceptable", 407: "Proxy Authentication Required", 408: "Request Timeout",
        409: "Conflict", 410: "Gone", 411: "Length Required", 412: "Precondition Failed",
        413: "Payload Too Large", 414: "URI Too Long", 415: "Unsupported Media Type",
        416: "Range Not Satisfiable", 417: "Expectation Failed", 418: "I'm a Teapot",
        421: "Misdirected Request", 422: "Unprocessable Entity", 423: "Locked",
        424: "Failed Dependency", 425: "Too Early", 426: "Upgrade Required",
        428: "Precondition Required", 429: "Too Many Requests", 431: "Request Header Fields Too Large",
        451: "Unavailable For Legal Reasons", 500: "Internal Server Error", 501: "Not Implemented",
        502: "Bad Gateway", 503: "Service Unavailable", 504: "Gateway Timeout",
        505: "HTTP Version Not Supported", 506: "Variant Also Negotiates", 507: "Insufficient Storage",
        508: "Loop Detected", 509: "Bandwidth Limit Exceeded", 510: "Not Extended",
        511: "Network Authentication Required"
    },
    maxHeaderSize: 16 * 1024,
    validateHeaderName,
    validateHeaderValue,
    setMaxIdleHTTPParsers(value) {
        if (typeof value !== "number") {
            const error = new TypeError("max must be a number");
            error.code = "ERR_INVALID_ARG_TYPE";
            throw error;
        }
        if (!Number.isInteger(value) || value < 1) {
            const error = new RangeError("max must be an integer greater than or equal to 1");
            error.code = "ERR_OUT_OF_RANGE";
            throw error;
        }
        maxIdleHttpParsers = value;
    },
    request(input, options, callback) { return clientRequest("http:", globalAgent, input, options, callback); },
    get(input, options, callback) {
        const request = clientRequest("http:", globalAgent, input, options, callback);
        request.end();
        return request;
    },
    createServer(optionsOrListener, listener, tlsOptions) {
        const rawRequestListener = typeof optionsOrListener === "function"
            ? optionsOrListener
            : listener;
        if (typeof rawRequestListener !== "function") {
            throw new TypeError("createServer requer uma função");
        }
        const requestListener = optimizeExpressRouter(optimizeExpressResponse(rawRequestListener));

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
                    listenArguments[3] = requestListener.__canaryoRequestPrototype
                        || listenArguments[3];
                    listenArguments[4] = requestListener.__canaryoResponsePrototype
                        || listenArguments[4];
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
                __canaryoMarkServerBusy();
                return this;
            },
            closeAllConnections() {
                this.__canaryoCloseAllConnectionsRequested = true;
                __canaryoMarkServerBusy();
            },
            closeIdleConnections() {
                this.__canaryoCloseIdleConnectionsRequested = true;
                __canaryoMarkServerBusy();
            }
        };
        const EventEmitter = ensureHttpEventPrototypes();
        EventEmitter.call(server);
        Object.setPrototypeOf(server, tlsOptions ? HttpsServer.prototype : Server.prototype);
        return server;
    }
});
const httpsModule = Object.freeze(Object.assign({}, httpModule, {
    Agent: HttpsAgent,
    Server: HttpsServer,
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
        let source = __canaryoReadFile(filename);
        // Node removes a Unix shebang before compiling CommonJS modules. CLI
        // packages such as Mocha rely on this even when launched on Windows.
        if (source.charCodeAt(0) === 35 && source.charCodeAt(1) === 33) {
            source = source.replace(/^#![^\r\n]*/, "");
        }
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
        if (/[\\\/]body-parser[\\\/]lib[\\\/]types[\\\/]json\.js$/.test(filename)
            && typeof module.exports === "function") {
            const createJsonParser = module.exports;
            module.exports = function canaryoJsonParserFactory(options) {
                const middleware = createJsonParser(options);
                if (options === undefined
                    || (options && Object.keys(options).length === 0)) {
                    Object.defineProperty(middleware, "__canaryoDefaultJsonParser", {
                        value: true
                    });
                }
                return middleware;
            };
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
globalThis.__canaryoServerTick = () => {
    const pending = globalThis.__canaryoPollHttpRequests();
    const timer = Math.max(-2147483648, Math.min(2147483647, globalThis.__canaryoRunTimers()));
    const flags = globalThis.__canaryoServerControl() | (pending > 0 ? 8 : 0);
    return (timer + 2147483648) * 16 + flags;
};
})();
"#;

#[cfg(test)]
const POLYFILLS: &str = concat!(
    include_str!("polyfills/00_web_events.part.js"),
    include_str!("polyfills/10_buffer_text.part.js"),
    include_str!("polyfills/20_util_path_url.part.js"),
    include_str!("polyfills/30_streams.part.js"),
    include_str!("polyfills/40_fetch.part.js"),
    include_str!("polyfills/50_async_process.part.js"),
    include_str!("polyfills/60_performance_timers.part.js"),
    include_str!("polyfills/70_filesystem.part.js"),
    include_str!("polyfills/80_dns_workers_net.part.js"),
    include_str!("polyfills/90_crypto_compression_registry.part.js"),
);

const RUNTIME_BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.qbc"));

pub fn execute(path: &str, arguments: &[String]) -> Result<u8, String> {
    let startup_started = Instant::now();
    let profile_startup = |phase: &str| {
        if env::var_os("CANARYO_PROFILE_STARTUP").is_some() {
            eprintln!("canaryo startup {phase}: {:.2?}", startup_started.elapsed());
        }
    };
    fs::metadata(path).map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    let entry = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("não foi possível resolver {path}: {error}"))?;
    let runtime = Runtime::new().map_err(|error| format!("erro ao criar runtime: {error}"))?;
    let gc_threshold = env::var("CANARYO_GC_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(32 * 1024 * 1024);
    runtime.set_gc_threshold(gc_threshold);
    runtime.set_loader(esm::NodeResolver, esm::NodeLoader);
    let context =
        Context::full(&runtime).map_err(|error| format!("erro ao criar contexto: {error}"))?;
    profile_startup("context");

    context.with(|context| {
        install_host_globals(&context, path, arguments)?;
        profile_startup("host-globals");
        // The bytecode is generated by build.rs with the exact same QuickJS version and is
        // embedded in the executable, so the unsafe loader never receives untrusted bytes.
        let (_, initialization) = unsafe { Module::load(context.clone(), RUNTIME_BYTECODE) }
            .catch(&context)
            .map_err(|error| error.to_string())?
            .eval()
            .catch(&context)
            .map_err(|error| error.to_string())?;
        initialization
            .finish::<()>()
            .catch(&context)
            .map_err(|error| error.to_string())?;
        profile_startup("runtime-bytecode");
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
            profile_startup("entry-module");
            None
        };

        let should_exit: Function = context
            .globals()
            .get("__canaryoProcessShouldExit")
            .map_err(|error| error.to_string())?;
        if !should_exit
            .call::<_, bool>(())
            .catch(&context)
            .map_err(|error| error.to_string())?
        {
            while context.execute_pending_job() {}
        }
        let start_server: Function = context
            .globals()
            .get("__canaryoStartPendingServer")
            .map_err(|error| error.to_string())?;
        if !should_exit
            .call::<_, bool>(())
            .catch(&context)
            .map_err(|error| error.to_string())?
        {
            start_server
                .call::<_, ()>(())
                .catch(&context)
                .map_err(|error| error.to_string())?;
        }
        if let Some(module_evaluation) = module_evaluation {
            module_evaluation
                .finish::<()>()
                .catch(&context)
                .map_err(|error| error.to_string())?;
        }
        while context.execute_pending_job() {}
        if !should_exit
            .call::<_, bool>(())
            .catch(&context)
            .map_err(|error| error.to_string())?
        {
            drain_event_loop(&context)?;
        }
        let exit_code: Function = context
            .globals()
            .get("__canaryoProcessExitCode")
            .map_err(|error| error.to_string())?;
        exit_code
            .call::<_, u8>(())
            .catch(&context)
            .map_err(|error| error.to_string())
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
    let should_exit: Function = context
        .globals()
        .get("__canaryoProcessShouldExit")
        .map_err(|error| error.to_string())?;
    loop {
        if should_exit
            .call::<_, bool>(())
            .catch(context)
            .map_err(|error| error.to_string())?
        {
            return Ok(());
        }
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
    let mark_server_busy = Function::new(context.clone(), http::mark_server_busy)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoMarkServerBusy", mark_server_busy)
        .map_err(|error| error.to_string())?;
    let invalidate_native_express_plans =
        Function::new(context.clone(), http::invalidate_native_express_plans)
            .map_err(|error| error.to_string())?;
    globals
        .set(
            "__canaryoInvalidateNativeExpressPlans",
            invalidate_native_express_plans,
        )
        .map_err(|error| error.to_string())?;
    let performance_started = Instant::now();
    let performance_now = Function::new(context.clone(), move || {
        performance_started.elapsed().as_secs_f64() * 1000.0
    })
    .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPerformanceNow", performance_now)
        .map_err(|error| error.to_string())?;
    let cwd = Function::new(context.clone(), cwd).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoCwd", cwd)
        .map_err(|error| error.to_string())?;
    let chdir = Function::new(context.clone(), change_dir).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoChdir", chdir)
        .map_err(|error| error.to_string())?;
    let hash = Function::new(context.clone(), crypto_digest).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHash", hash)
        .map_err(|error| error.to_string())?;
    let express_weak_etag =
        Function::new(context.clone(), express_weak_etag).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoExpressWeakEtag", express_weak_etag)
        .map_err(|error| error.to_string())?;
    let cache_response_body = Function::new(context.clone(), http::cache_response_body)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoCacheResponseBody", cache_response_body)
        .map_err(|error| error.to_string())?;
    let release_response_body = Function::new(context.clone(), http::release_response_body)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoReleaseResponseBody", release_response_body)
        .map_err(|error| error.to_string())?;
    let hmac = Function::new(context.clone(), crypto_hmac).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHmac", hmac)
        .map_err(|error| error.to_string())?;
    let pbkdf2 =
        Function::new(context.clone(), crypto_pbkdf2).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPbkdf2", pbkdf2)
        .map_err(|error| error.to_string())?;
    let hkdf = Function::new(context.clone(), crypto_hkdf).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHkdf", hkdf)
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
    let encode_utf8 =
        Function::new(context.clone(), encode_utf8).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoEncodeUtf8", encode_utf8)
        .map_err(|error| error.to_string())?;
    let decode_utf8 =
        Function::new(context.clone(), decode_utf8).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoDecodeUtf8", decode_utf8)
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
        Function::new(context.clone(), move |context| {
            poll_outbound_http_event(context, &receiver)
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
    let net_poll = Function::new(context.clone(), move |context| {
        poll_net_client_event(context, &net_receiver)
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
    os_info
        .set("stdinIsTerminal", io::stdin().is_terminal())
        .map_err(|error| error.to_string())?;
    os_info
        .set("stdoutIsTerminal", io::stdout().is_terminal())
        .map_err(|error| error.to_string())?;
    os_info
        .set("stderrIsTerminal", io::stderr().is_terminal())
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
    let fs_lstat = Function::new(context.clone(), fs_lstat).map_err(|error| error.to_string())?;
    let fs_readlink =
        Function::new(context.clone(), fs_readlink).map_err(|error| error.to_string())?;
    let fs_link = Function::new(context.clone(), fs_link).map_err(|error| error.to_string())?;
    let fs_symlink =
        Function::new(context.clone(), fs_symlink).map_err(|error| error.to_string())?;
    let fs_chmod = Function::new(context.clone(), fs_chmod).map_err(|error| error.to_string())?;
    let fs_utimes = Function::new(context.clone(), fs_utimes).map_err(|error| error.to_string())?;
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
    globals
        .set("__canaryoFsLstat", fs_lstat)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsReadlink", fs_readlink)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsLink", fs_link)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsSymlink", fs_symlink)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsChmod", fs_chmod)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsUtimes", fs_utimes)
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
        body: Vec<u8>,
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
    fn into_js<'js>(self, context: rquickjs::Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        let value = Object::new(context.clone())?;
        match self {
            Self::Connected {
                id,
                local_address,
                local_port,
                remote_address,
                remote_port,
                family,
            } => {
                value.set("type", "connected")?;
                value.set("id", id)?;
                value.set("localAddress", local_address)?;
                value.set("localPort", local_port)?;
                value.set("remoteAddress", remote_address)?;
                value.set("remotePort", remote_port)?;
                value.set("family", family)?;
            }
            Self::Data { id, body } => {
                value.set("type", "data")?;
                value.set("id", id)?;
                value.set("body", TypedArray::new(context, body)?)?;
            }
            Self::End { id } => {
                value.set("type", "end")?;
                value.set("id", id)?;
            }
            Self::Error { id, message } => {
                value.set("type", "error")?;
                value.set("id", id)?;
                value.set("message", message)?;
            }
            Self::Close { id } => {
                value.set("type", "close")?;
                value.set("id", id)?;
            }
        }
        Ok(value)
    }
}

fn poll_net_client_event<'js>(
    context: rquickjs::Ctx<'js>,
    receiver: &Mutex<Receiver<NetClientEvent>>,
) -> rquickjs::Result<Option<Object<'js>>> {
    receiver
        .lock()
        .ok()
        .and_then(|receiver| receiver.try_recv().ok())
        .map(|event| event.into_js(context))
        .transpose()
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
    body: TypedArray<'js, u8>,
    runtime: &NetClientRuntime,
) -> rquickjs::Result<()> {
    let body = typed_array_bytes(&context, &body)?.to_vec();
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
                        body: input[..length].to_vec(),
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
        body: Vec<u8>,
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
    fn into_js<'js>(self, context: rquickjs::Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        let value = Object::new(context.clone())?;
        match self {
            Self::Headers {
                id,
                status,
                status_text,
                headers,
            } => {
                let js_headers = Array::new(context.clone())?;
                for (index, (name, header_value)) in headers.into_iter().enumerate() {
                    let pair = Array::new(context.clone())?;
                    pair.set(0, name)?;
                    pair.set(1, header_value)?;
                    js_headers.set(index, pair)?;
                }
                value.set("type", "headers")?;
                value.set("id", id)?;
                value.set("status", status)?;
                value.set("statusText", status_text)?;
                value.set("headers", js_headers)?;
            }
            Self::Data { id, body } => {
                value.set("type", "data")?;
                value.set("id", id)?;
                value.set("body", TypedArray::new(context, body)?)?;
            }
            Self::End { id } => {
                value.set("type", "end")?;
                value.set("id", id)?;
            }
            Self::Error { id, message } => {
                value.set("type", "error")?;
                value.set("id", id)?;
                value.set("message", message)?;
            }
        }
        Ok(value)
    }
}

fn poll_outbound_http_event<'js>(
    context: rquickjs::Ctx<'js>,
    receiver: &Mutex<Receiver<OutboundHttpEvent>>,
) -> rquickjs::Result<Option<Object<'js>>> {
    receiver
        .lock()
        .ok()
        .and_then(|receiver| receiver.try_recv().ok())
        .map(|event| event.into_js(context))
        .transpose()
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
    body: TypedArray<'js, u8>,
    runtime: &OutboundHttpRuntime,
) -> rquickjs::Result<()> {
    let body = typed_array_bytes(&context, &body)?.to_vec();
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
                        body: chunk[..read].to_vec(),
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

fn fs_read<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let bytes = fs::read(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    TypedArray::new(context, bytes)
}

fn fs_write<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    bytes: TypedArray<'js, u8>,
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
    let bytes = typed_array_bytes(&context, &bytes)?;
    file.write_all(bytes)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_stat<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Object<'js>> {
    let metadata = fs::metadata(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    fs_metadata_object(context, metadata)
}

fn fs_lstat<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Object<'js>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    fs_metadata_object(context, metadata)
}

fn fs_metadata_object<'js>(
    context: rquickjs::Ctx<'js>,
    metadata: fs::Metadata,
) -> rquickjs::Result<Object<'js>> {
    use std::time::UNIX_EPOCH;

    let result = Object::new(context.clone())?;
    result.set("size", metadata.len() as f64)?;
    result.set("file", metadata.is_file())?;
    result.set("directory", metadata.is_dir())?;
    result.set("symlink", metadata.file_type().is_symlink())?;
    let timestamp = |value: io::Result<std::time::SystemTime>| {
        value
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    };
    let mtime = timestamp(metadata.modified());
    let atime = timestamp(metadata.accessed());
    let birthtime = timestamp(metadata.created());
    result.set("mtimeMs", mtime)?;
    result.set("atimeMs", atime)?;
    result.set("ctimeMs", mtime)?;
    result.set("birthtimeMs", birthtime)?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::MetadataExt;
        metadata.mode()
    };
    #[cfg(not(unix))]
    let mode = if metadata.permissions().readonly() {
        0o444
    } else {
        0o666
    };
    result.set("mode", mode)?;
    result.set("blocks", metadata.len().div_ceil(512) as f64)?;
    result.set("blksize", 4096)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        result.set("dev", metadata.dev() as f64)?;
        result.set("ino", metadata.ino() as f64)?;
        result.set("nlink", metadata.nlink() as f64)?;
        result.set("uid", metadata.uid())?;
        result.set("gid", metadata.gid())?;
        result.set("rdev", metadata.rdev() as f64)?;
    }
    #[cfg(not(unix))]
    {
        result.set("dev", 0)?;
        result.set("ino", 0)?;
        result.set("nlink", 1)?;
        result.set("uid", 0)?;
        result.set("gid", 0)?;
        result.set("rdev", 0)?;
    }
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

fn fs_readlink<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<String> {
    fs::read_link(path)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_link<'js>(context: rquickjs::Ctx<'js>, from: String, to: String) -> rquickjs::Result<()> {
    fs::hard_link(from, to)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_symlink<'js>(
    context: rquickjs::Ctx<'js>,
    target: String,
    path: String,
    directory: bool,
) -> rquickjs::Result<()> {
    #[cfg(unix)]
    let result = {
        let _ = directory;
        std::os::unix::fs::symlink(target, path)
    };
    #[cfg(windows)]
    let result = if directory {
        std::os::windows::fs::symlink_dir(target, path)
    } else {
        std::os::windows::fs::symlink_file(target, path)
    };
    #[cfg(not(any(unix, windows)))]
    let result = {
        let _ = (target, path, directory);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are unsupported on this platform",
        ))
    };
    result.map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_chmod<'js>(context: rquickjs::Ctx<'js>, path: String, mode: u32) -> rquickjs::Result<()> {
    let metadata = fs::metadata(&path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(mode);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(mode & 0o222 == 0);
    fs::set_permissions(path, permissions)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_utimes<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    accessed_seconds: f64,
    modified_seconds: f64,
) -> rquickjs::Result<()> {
    use std::time::{Duration, UNIX_EPOCH};

    if !accessed_seconds.is_finite()
        || !modified_seconds.is_finite()
        || accessed_seconds < 0.0
        || modified_seconds < 0.0
    {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "timestamps must be finite non-negative values",
        ));
    }
    let times = fs::FileTimes::new()
        .set_accessed(UNIX_EPOCH + Duration::from_secs_f64(accessed_seconds))
        .set_modified(UNIX_EPOCH + Duration::from_secs_f64(modified_seconds));
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|file| file.set_times(times))
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

fn encode_utf8<'js>(
    context: rquickjs::Ctx<'js>,
    value: rquickjs::String<'js>,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    TypedArray::new(context, value.to_string()?.into_bytes())
}

fn decode_utf8(bytes: TypedArray<'_, u8>) -> rquickjs::Result<String> {
    let bytes = bytes
        .as_bytes()
        .ok_or_else(|| rquickjs::Error::new_from_js("detached Uint8Array", "string"))?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn cwd() -> String {
    env::current_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".into())
}

fn change_dir<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<()> {
    env::set_current_dir(&path).map_err(|error| {
        rquickjs::Exception::throw_message(
            &context,
            &format!("cannot change directory to '{path}': {error}"),
        )
    })
}

fn typed_array_bytes<'a, 'js>(
    context: &rquickjs::Ctx<'js>,
    value: &'a TypedArray<'js, u8>,
) -> rquickjs::Result<&'a [u8]> {
    value
        .as_bytes()
        .ok_or_else(|| rquickjs::Exception::throw_message(context, "detached Uint8Array"))
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
    contents: TypedArray<'js, u8>,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let algorithm = digest_algorithm(&algorithm).ok_or_else(|| {
        rquickjs::Exception::throw_message(&context, &format!("unsupported hash: {algorithm}"))
    })?;
    let contents = typed_array_bytes(&context, &contents)?;
    let digest = ring::digest::digest(algorithm, contents);
    TypedArray::new_copy(context, digest.as_ref())
}

fn express_weak_etag<'js>(
    context: rquickjs::Ctx<'js>,
    body: String,
) -> rquickjs::Result<Array<'js>> {
    let length = body.len();
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, body.as_bytes());
    let encoded = base64::engine::general_purpose::STANDARD.encode(digest.as_ref());
    let output = Array::new(context)?;
    output.set(0, length)?;
    output.set(
        1,
        format!("W/\"{:x}-{}\"", length, encoded.trim_end_matches('=')),
    )?;
    Ok(output)
}

fn crypto_hmac<'js>(
    context: rquickjs::Ctx<'js>,
    algorithm: String,
    key: TypedArray<'js, u8>,
    contents: TypedArray<'js, u8>,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let algorithm = hmac_algorithm(&algorithm).ok_or_else(|| {
        rquickjs::Exception::throw_message(&context, &format!("unsupported hash: {algorithm}"))
    })?;
    let key = typed_array_bytes(&context, &key)?;
    let contents = typed_array_bytes(&context, &contents)?;
    let tag = ring::hmac::sign(&ring::hmac::Key::new(algorithm, key), contents);
    TypedArray::new_copy(context, tag.as_ref())
}

fn crypto_pbkdf2<'js>(
    context: rquickjs::Ctx<'js>,
    password: TypedArray<'js, u8>,
    salt: TypedArray<'js, u8>,
    iterations: u32,
    key_length: u32,
    digest: String,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let algorithm = match digest.to_ascii_lowercase().replace('-', "").as_str() {
        "sha1" => ring::pbkdf2::PBKDF2_HMAC_SHA1,
        "sha256" => ring::pbkdf2::PBKDF2_HMAC_SHA256,
        "sha384" => ring::pbkdf2::PBKDF2_HMAC_SHA384,
        "sha512" => ring::pbkdf2::PBKDF2_HMAC_SHA512,
        _ => {
            return Err(rquickjs::Exception::throw_message(
                &context,
                &format!("unsupported hash: {digest}"),
            ));
        }
    };
    let iterations = std::num::NonZeroU32::new(iterations).ok_or_else(|| {
        rquickjs::Exception::throw_message(&context, "iterations must be greater than zero")
    })?;
    let password = typed_array_bytes(&context, &password)?;
    let salt = typed_array_bytes(&context, &salt)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(key_length as usize)
        .map_err(|_| rquickjs::Exception::throw_message(&context, "derived key is too large"))?;
    output.resize(key_length as usize, 0);
    ring::pbkdf2::derive(algorithm, iterations, salt, password, &mut output);
    TypedArray::new(context, output)
}

struct HkdfLength(usize);

impl ring::hkdf::KeyType for HkdfLength {
    fn len(&self) -> usize {
        self.0
    }
}

fn crypto_hkdf<'js>(
    context: rquickjs::Ctx<'js>,
    digest: String,
    key: TypedArray<'js, u8>,
    salt: TypedArray<'js, u8>,
    info: TypedArray<'js, u8>,
    key_length: u32,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let algorithm = match digest.to_ascii_lowercase().replace('-', "").as_str() {
        "sha1" => ring::hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY,
        "sha256" => ring::hkdf::HKDF_SHA256,
        "sha384" => ring::hkdf::HKDF_SHA384,
        "sha512" => ring::hkdf::HKDF_SHA512,
        _ => {
            return Err(rquickjs::Exception::throw_message(
                &context,
                &format!("unsupported hash: {digest}"),
            ));
        }
    };
    let key = typed_array_bytes(&context, &key)?;
    let salt = typed_array_bytes(&context, &salt)?;
    let info = typed_array_bytes(&context, &info)?;
    let extracted = ring::hkdf::Salt::new(algorithm, salt).extract(key);
    let info_parts = [info];
    let expanded = extracted
        .expand(&info_parts, HkdfLength(key_length as usize))
        .map_err(|_| rquickjs::Exception::throw_message(&context, "derived key is too large"))?;
    let mut output = vec![0; key_length as usize];
    expanded
        .fill(&mut output)
        .map_err(|_| rquickjs::Exception::throw_message(&context, "could not derive key"))?;
    TypedArray::new(context, output)
}

fn crypto_random_bytes<'js>(
    context: rquickjs::Ctx<'js>,
    size: u32,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    let mut bytes = vec![0_u8; size as usize];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            rquickjs::Exception::throw_message(&context, "operating system random generator failed")
        })?;
    TypedArray::new(context, bytes)
}

fn crypto_timing_safe_equal<'js>(
    context: rquickjs::Ctx<'js>,
    left: TypedArray<'js, u8>,
    right: TypedArray<'js, u8>,
) -> rquickjs::Result<bool> {
    let left = typed_array_bytes(&context, &left)?;
    let right = typed_array_bytes(&context, &right)?;
    if left.len() != right.len() {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "Input buffers must have the same byte length",
        ));
    }
    Ok(bool::from(left.ct_eq(right)))
}

fn zlib_transform<'js>(
    context: rquickjs::Ctx<'js>,
    operation: String,
    contents: TypedArray<'js, u8>,
) -> rquickjs::Result<TypedArray<'js, u8>> {
    use flate2::{
        Compression,
        read::{DeflateDecoder, GzDecoder, ZlibDecoder},
        write::{DeflateEncoder, GzEncoder, ZlibEncoder},
    };

    let input = typed_array_bytes(&context, &contents)?;
    let result = match operation.as_str() {
        "gzip" => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(input).and_then(|_| encoder.finish())
        }
        "gunzip" => read_compressed(GzDecoder::new(input)),
        "deflate" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(input).and_then(|_| encoder.finish())
        }
        "inflate" => read_compressed(ZlibDecoder::new(input)),
        "deflateRaw" => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(input).and_then(|_| encoder.finish())
        }
        "inflateRaw" => read_compressed(DeflateDecoder::new(input)),
        "unzip" if input.starts_with(&[0x1f, 0x8b]) => read_compressed(GzDecoder::new(input)),
        "unzip" => read_compressed(ZlibDecoder::new(input)),
        "brotliCompress" => {
            let mut reader = input;
            let mut output = Vec::new();
            let params = brotli::enc::BrotliEncoderParams::default();
            brotli::BrotliCompress(&mut reader, &mut output, &params).map(|_| output)
        }
        "brotliDecompress" => {
            let mut output = Vec::new();
            let mut reader = input;
            brotli::BrotliDecompress(&mut reader, &mut output).map(|_| output)
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
    TypedArray::new(context, output)
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
    fn returns_explicit_process_exit_codes() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let filename = env::temp_dir().join(format!(
            "canaryo-process-exit-{}-{id}.js",
            std::process::id()
        ));
        fs::write(
            &filename,
            "setTimeout(() => { throw new Error('timer should not run'); }, 10000); process.exit(7);",
        )
        .unwrap();

        let exit_code = execute(filename.to_string_lossy().as_ref(), &[]).unwrap();
        fs::write(&filename, "process.exitCode = 9;").unwrap();
        let passive_exit_code = execute(filename.to_string_lossy().as_ref(), &[]).unwrap();

        fs::remove_file(filename).unwrap();
        assert_eq!(exit_code, 7);
        assert_eq!(passive_exit_code, 9);
    }

    #[test]
    fn executes_commonjs_entry_points_with_a_shebang() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let filename =
            env::temp_dir().join(format!("canaryo-shebang-{}-{id}.js", std::process::id()));
        fs::write(&filename, "#!/usr/bin/env node\nprocess.exitCode = 17;\n").unwrap();

        let exit_code = execute(filename.to_string_lossy().as_ref(), &[]).unwrap();

        fs::remove_file(filename).unwrap();
        assert_eq!(exit_code, 17);
    }

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
                    Buffer.from([0x41, 0xe9]).latin1Slice() === "Aé" &&
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
                    atob(btoa("Canaryo")) === "Canaryo" &&
                    (() => {
                        const signed = Buffer.alloc(6);
                        signed.writeIntLE(-123456, 0, 3);
                        signed.writeUIntBE(0x123456, 3, 3);
                        const floating = Buffer.alloc(12);
                        floating.writeFloatLE(1.5, 0);
                        floating.writeDoubleBE(Math.PI, 4);
                        const bigint = Buffer.alloc(16);
                        bigint.writeBigUInt64LE(0x123456789abcdef0n, 0);
                        bigint.writeBigInt64BE(-42n, 8);
                        return signed.readIntLE(0, 3) === -123456 &&
                            signed.readUIntBE(3, 3) === 0x123456 &&
                            floating.readFloatLE(0) === 1.5 &&
                            Math.abs(floating.readDoubleBE(4) - Math.PI) < 1e-12 &&
                            bigint.readBigUInt64LE(0) === 0x123456789abcdef0n &&
                            bigint.readBigInt64BE(8) === -42n &&
                            Buffer.from([1, 2, 3, 4]).swap32().equals(Buffer.from([4, 3, 2, 1])) &&
                            Buffer.from("ababa").lastIndexOf("ba") === 3 &&
                            Buffer.allocUnsafeSlow(3).length === 3 && Buffer.poolSize === 8192;
                    })()
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
    fn derives_keys_with_pbkdf2() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const crypto = __canaryoBuiltins.crypto;
                    const sha1 = crypto.pbkdf2Sync("password", "salt", 1, 20, "sha1").toString("hex");
                    const sha256 = crypto.pbkdf2Sync("password", "salt", 1, 32, "sha256").toString("hex");
                    let invalidIterations = false;
                    try { crypto.pbkdf2Sync("password", "salt", 0, 20, "sha256"); }
                    catch (error) { invalidIterations = error instanceof RangeError; }
                    globalThis.pbkdf2CallbackPassed = false;
                    crypto.pbkdf2("password", "salt", 2, 20, "sha1", (error, key) => {
                        if (error) throw error;
                        pbkdf2CallbackPassed = Buffer.isBuffer(key) &&
                            key.toString("hex") === "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957";
                    });
                    sha1 === "0c60c80f961f0e71f3a9b524af6012062fe037a6" &&
                        sha256 === "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b" &&
                        invalidIterations
                    "#,
                )
                .unwrap();

            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("pbkdf2CallbackPassed")
                    .unwrap()
            );
        });
    }

    #[test]
    fn derives_keys_with_hkdf() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const crypto = __canaryoBuiltins.crypto;
                    const key = Buffer.from("0b".repeat(22), "hex");
                    const salt = Buffer.from("000102030405060708090a0b0c", "hex");
                    const info = Buffer.from("f0f1f2f3f4f5f6f7f8f9", "hex");
                    const expected = "3cb25f25faacd57a90434f64d0362f2a" +
                        "2d2d0a90cf1a5a4c5db02d56ecc4c5bf" +
                        "34007208d5b887185865";
                    const derived = Buffer.from(crypto.hkdfSync("sha256", key, salt, info, 42));
                    globalThis.hkdfCallbackPassed = false;
                    crypto.hkdf("sha256", key, salt, info, 42, (error, result) => {
                        if (error) throw error;
                        hkdfCallbackPassed = result instanceof ArrayBuffer && Buffer.from(result).toString("hex") === expected;
                    });
                    derived.toString("hex") === expected
                    "#,
                )
                .unwrap();

            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("hkdfCallbackPassed")
                    .unwrap()
            );
        });
    }

    #[test]
    fn provides_web_crypto_digests_and_hmac_keys() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.webCryptoPassed = false;
                    (async () => {
                        const nodeCrypto = __canaryoBuiltins.crypto;
                        const data = new TextEncoder().encode("hello");
                        const digest = Buffer.from(await crypto.subtle.digest("SHA-256", data));
                        const key = await crypto.subtle.importKey(
                            "raw",
                            new TextEncoder().encode("secret"),
                            { name: "HMAC", hash: "SHA-256" },
                            true,
                            ["sign", "verify"]
                        );
                        const signature = await crypto.subtle.sign("HMAC", key, data);
                        const verified = await crypto.subtle.verify("HMAC", key, signature, data);
                        const exported = Buffer.from(await crypto.subtle.exportKey("raw", key));
                        const generated = await crypto.subtle.generateKey(
                            { name: "HMAC", hash: "SHA-256", length: 128 },
                            true,
                            ["sign"]
                        );
                        const password = await crypto.subtle.importKey(
                            "raw",
                            new TextEncoder().encode("password"),
                            "PBKDF2",
                            false,
                            ["deriveBits", "deriveKey"]
                        );
                        const derivation = {
                            name: "PBKDF2",
                            hash: "SHA-256",
                            salt: new TextEncoder().encode("salt"),
                            iterations: 1
                        };
                        const derivedBits = Buffer.from(await crypto.subtle.deriveBits(derivation, password, 256));
                        const derivedKey = await crypto.subtle.deriveKey(
                            derivation,
                            password,
                            { name: "HMAC", hash: "SHA-256", length: 128 },
                            true,
                            ["sign"]
                        );
                        const hkdfKey = await crypto.subtle.importKey(
                            "raw",
                            Buffer.from("0b".repeat(22), "hex"),
                            "HKDF",
                            false,
                            ["deriveBits"]
                        );
                        const hkdfBits = Buffer.from(await crypto.subtle.deriveBits({
                            name: "HKDF",
                            hash: "SHA-256",
                            salt: Buffer.from("000102030405060708090a0b0c", "hex"),
                            info: Buffer.from("f0f1f2f3f4f5f6f7f8f9", "hex")
                        }, hkdfKey, 336));
                        const integer = nodeCrypto.randomInt(10, 20);
                        const floating = nodeCrypto.randomFloat();
                        webCryptoPassed = digest.toString("hex") ===
                            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" &&
                            verified && exported.toString() === "secret" && key instanceof CryptoKey &&
                            generated.algorithm.length === 128 && nodeCrypto.subtle === crypto.subtle &&
                            derivedBits.toString("hex") === "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b" &&
                            derivedKey.algorithm.name === "HMAC" && derivedKey.algorithm.length === 128 &&
                            hkdfBits.toString("hex") === "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865" &&
                            nodeCrypto.hash("sha256", "hello", "hex") === digest.toString("hex") &&
                            integer >= 10 && integer < 20 && floating >= 0 && floating < 1 &&
                            crypto instanceof Crypto && crypto.subtle instanceof SubtleCrypto;
                    })();
                    "#,
                )
                .unwrap();

            for _ in 0..20 {
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .globals()
                    .get::<_, bool>("webCryptoPassed")
                    .unwrap()
            );
        });
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
    fn compresses_and_decompresses_web_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.webCompressionPassed = false;
                    (async () => {
                        const source = new TextEncoder().encode("Canaryo web compression".repeat(20));
                        const compression = new CompressionStream("gzip");
                        const decompression = new DecompressionStream("gzip");
                        const piping = compression.readable.pipeTo(decompression.writable);
                        const writer = compression.writable.getWriter();
                        const reader = decompression.readable.getReader();
                        await writer.write(source.subarray(0, 37));
                        await writer.write(source.subarray(37));
                        await writer.close();
                        const chunks = [];
                        while (true) {
                            const { value, done } = await reader.read();
                            if (done) break;
                            chunks.push(value);
                        }
                        await piping;
                        const decoded = new TextDecoder().decode(Buffer.concat(chunks.map(Buffer.from)));
                        webCompressionPassed = decoded === "Canaryo web compression".repeat(20) &&
                            compression.readable instanceof ReadableStream &&
                            decompression.writable instanceof WritableStream &&
                            __canaryoBuiltins["stream/web"].CompressionStream === CompressionStream;
                    })();
                    "#,
                )
                .unwrap();

            for _ in 0..40 {
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .globals()
                    .get::<_, bool>("webCompressionPassed")
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
    fn copies_directory_trees_and_returns_dirents() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            env::temp_dir().join(format!("canaryo-fs-copy-{}-{id}", std::process::id()));
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
                    const source = path.join(fixtureDirectory, "source");
                    const nested = path.join(source, "nested");
                    const copied = path.join(fixtureDirectory, "copied");
                    fs.mkdirSync(nested, { recursive: true });
                    fs.writeFileSync(path.join(source, "root.txt"), "root");
                    fs.writeFileSync(path.join(nested, "child.txt"), "child");
                    fs.writeFileSync(path.join(nested, "skip.txt"), "skip");
                    const entries = fs.readdirSync(source, { withFileTypes: true });
                    const buffered = fs.readdirSync(source, { encoding: "buffer" });
                    fs.cpSync(source, copied, {
                        recursive: true,
                        filter(from) { return !from.endsWith("skip.txt"); }
                    });
                    fs.truncateSync(path.join(copied, "root.txt"), 2);
                    globalThis.fsCopyCallback = false;
                    fs.access(path.join(copied, "root.txt"), error => {
                        if (error) throw error;
                        fs.exists(path.join(copied, "nested", "child.txt"), exists => { fsCopyCallback = exists; });
                    });
                    globalThis.fsCopyPromise = false;
                    const promised = path.join(fixtureDirectory, "promised");
                    fs.promises.cp(source, promised, { recursive: true }).then(async () => {
                        await fs.promises.truncate(path.join(promised, "nested", "child.txt"), 3);
                        fsCopyPromise = fs.readFileSync(path.join(promised, "nested", "child.txt"), "utf8") === "chi";
                    });

                    entries.some(entry => entry instanceof fs.Dirent && entry.name === "nested" && entry.isDirectory()) &&
                        entries.some(entry => entry.name === "root.txt" && entry.isFile()) &&
                        buffered.every(Buffer.isBuffer) &&
                        fs.readFileSync(path.join(copied, "root.txt"), "utf8") === "ro" &&
                        fs.readFileSync(path.join(copied, "nested", "child.txt"), "utf8") === "child" &&
                        !fs.existsSync(path.join(copied, "nested", "skip.txt"))
                    "#,
                )
                .unwrap();

            for _ in 0..30 {
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert!(context.globals().get::<_, bool>("fsCopyCallback").unwrap());
            assert!(context.globals().get::<_, bool>("fsCopyPromise").unwrap());
        });

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn iterates_open_directories_and_recursive_listings() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!("canaryo-fs-dir-{}-{id}", std::process::id()));
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
                    fs.mkdirSync(nested, { recursive: true });
                    fs.writeFileSync(path.join(fixtureDirectory, "root.txt"), "root");
                    fs.writeFileSync(path.join(nested, "child.txt"), "child");

                    const recursiveNames = fs.readdirSync(fixtureDirectory, { recursive: true });
                    const recursiveEntries = fs.readdirSync(fixtureDirectory, { recursive: true, withFileTypes: true });
                    const directory = fs.opendirSync(fixtureDirectory);
                    const opened = [];
                    for (let entry = directory.readSync(); entry !== null; entry = directory.readSync()) opened.push(entry);
                    directory.closeSync();
                    let closedError = false;
                    try { directory.readSync(); } catch { closedError = true; }

                    globalThis.fsDirCallback = false;
                    fs.opendir(fixtureDirectory, (error, handle) => {
                        if (error) throw error;
                        handle.read((readError, entry) => {
                            if (readError) throw readError;
                            handle.close(closeError => {
                                if (closeError) throw closeError;
                                fsDirCallback = entry instanceof fs.Dirent;
                            });
                        });
                    });

                    globalThis.fsDirPromise = false;
                    (async () => {
                        const handle = await fs.promises.opendir(fixtureDirectory, { recursive: true });
                        const entries = [];
                        for await (const entry of handle) entries.push(entry);
                        let automaticallyClosed = false;
                        try { await handle.read(); } catch { automaticallyClosed = true; }
                        fsDirPromise = entries.length === 3 && automaticallyClosed;
                    })();

                    recursiveNames.includes("root.txt") &&
                        recursiveNames.includes(path.join("nested", "child.txt")) &&
                        recursiveEntries.some(entry => entry.name === "child.txt" && entry.parentPath === nested) &&
                        opened.length === 2 && opened.every(entry => entry instanceof fs.Dirent) &&
                        directory instanceof fs.Dir && closedError
                    "#,
                )
                .unwrap();

            for _ in 0..50 {
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert!(context.globals().get::<_, bool>("fsDirCallback").unwrap());
            assert!(context.globals().get::<_, bool>("fsDirPromise").unwrap());
        });

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reads_and_writes_through_file_descriptors() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let filename = env::temp_dir().join(format!("canaryo-fd-{}-{id}.txt", std::process::id()));
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context
                .globals()
                .set("fixturePath", filename.to_string_lossy().as_ref())
                .unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const fs = __canaryoBuiltins.fs;
                    const fd = fs.openSync(fixturePath, "w+");
                    const first = fs.writeSync(fd, Buffer.from("hello"), 0, 5, 0);
                    const second = fs.writeSync(fd, "!", 5, "utf8");
                    const output = Buffer.alloc(6);
                    const read = fs.readSync(fd, output, 0, output.length, 0);
                    fs.ftruncateSync(fd, 5);
                    const vectoredWrite = fs.writevSync(fd, [Buffer.from(" "), Buffer.from("world")], 5);
                    const vectorBuffers = [Buffer.alloc(5), Buffer.alloc(6)];
                    const vectoredRead = fs.readvSync(fd, vectorBuffers, 0);
                    fs.fsyncSync(fd);
                    const size = fs.fstatSync(fd).size;
                    const bigintStats = fs.fstatSync(fd, { bigint: true });
                    const missing = fs.statSync(`${fixturePath}.missing`, { throwIfNoEntry: false });
                    fs.closeSync(fd);
                    let badDescriptor = false;
                    try { fs.fstatSync(fd); }
                    catch (error) { badDescriptor = /EBADF/.test(error.message); }
                    globalThis.fdCallbackPassed = false;
                    fs.open(fixturePath, "r", (openError, callbackFd) => {
                        if (openError) throw openError;
                        const buffers = [Buffer.alloc(2), Buffer.alloc(3)];
                        fs.readv(callbackFd, buffers, 0, (readError, bytesRead, returned) => {
                            if (readError) throw readError;
                            fs.close(callbackFd, closeError => {
                                if (closeError) throw closeError;
                                fdCallbackPassed = bytesRead === 5 && returned === buffers && Buffer.concat(buffers).toString() === "hello";
                            });
                        });
                    });
                    globalThis.fdPromisePassed = false;
                    fs.promises.open(fixturePath, "r+").then(async handle => {
                        const writeBuffers = [Buffer.from("Y"), Buffer.from("Z")];
                        const written = await handle.writev(writeBuffers, 4);
                        const buffers = [Buffer.alloc(5), Buffer.alloc(6)];
                        const result = await handle.readv(buffers, 0);
                        await handle.truncate(4);
                        await handle.datasync();
                        const stat = await handle.stat({ bigint: true });
                        await handle.close();
                        fdPromisePassed = written.bytesWritten === 2 && written.buffers === writeBuffers &&
                            result.bytesRead === 11 && result.buffers === buffers && Buffer.concat(buffers).toString() === "hellYZworld" &&
                            stat.size === 4n && fs.promises.constants.O_RDWR === 2 && handle.fd === -1;
                    });

                    first === 5 && second === 1 && read === 6 && output.toString() === "hello!" &&
                        vectoredWrite === 6 && vectoredRead === 11 && Buffer.concat(vectorBuffers).toString() === "hello world" &&
                        size === 11 && typeof bigintStats.size === "bigint" && bigintStats.size === 11n &&
                        typeof bigintStats.mtimeNs === "bigint" && bigintStats.isFile() && missing === undefined &&
                        badDescriptor && fs.constants.O_RDWR === 2
                    "#,
                )
                .unwrap();

            for _ in 0..20 {
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert!(context.globals().get::<_, bool>("fdCallbackPassed").unwrap());
            assert!(context.globals().get::<_, bool>("fdPromisePassed").unwrap());
        });

        if filename.exists() {
            fs::remove_file(filename).unwrap();
        }
    }

    #[test]
    fn manages_links_permissions_and_file_times() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            env::temp_dir().join(format!("canaryo-fs-meta-{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
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
                    const original = path.join(fixtureDirectory, "original.txt");
                    const hardLink = path.join(fixtureDirectory, "hard.txt");
                    const symbolicLink = path.join(fixtureDirectory, "symbolic.txt");
                    fs.writeFileSync(original, "metadata");
                    fs.linkSync(original, hardLink);
                    fs.utimesSync(original, new Date(1577934245000), 1577934245);
                    const timestamp = fs.statSync(original).mtimeMs;
                    fs.chmodSync(original, 0o444);
                    const readonly = (fs.statSync(original).mode & 0o222) === 0;
                    fs.chmodSync(original, 0o666);
                    let symlinkReady = false;
                    try {
                        fs.symlinkSync(original, symbolicLink, "file");
                        symlinkReady = fs.lstatSync(symbolicLink).isSymbolicLink() &&
                            fs.readlinkSync(symbolicLink) === original;
                    } catch {}
                    const temporary = fs.mkdtempSync(path.join(fixtureDirectory, "temp-"));
                    globalThis.fsMetadataPromise = false;
                    const promisedLink = path.join(fixtureDirectory, "promised.txt");
                    fs.promises.link(original, promisedLink).then(async () => {
                        const metadata = await fs.promises.lstat(promisedLink);
                        await fs.promises.chmod(promisedLink, 0o666);
                        await fs.promises.utimes(promisedLink, 1577934245, 1577934245);
                        await fs.promises.unlink(promisedLink);
                        const promisedTemp = await fs.promises.mkdtemp(path.join(fixtureDirectory, "promise-"));
                        await fs.promises.rmdir(promisedTemp);
                        fsMetadataPromise = metadata.isFile() && !fs.existsSync(promisedLink);
                    });

                    fs.readFileSync(hardLink, "utf8") === "metadata" &&
                        fs.lstatSync(original).isFile() && readonly &&
                        Math.abs(timestamp - 1577934245000) < 2000 &&
                        fs.statSync(temporary).isDirectory() &&
                        (!fs.existsSync(symbolicLink) || symlinkReady)
                    "#,
                )
                .unwrap();

            for _ in 0..30 {
                while context.execute_pending_job() {}
            }

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("fsMetadataPromise")
                    .unwrap()
            );
        });

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn watches_file_and_directory_changes() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            env::temp_dir().join(format!("canaryo-fs-watch-{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context
                .globals()
                .set("fixtureDirectory", directory.to_string_lossy().as_ref())
                .unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let initialized = context
                .eval::<bool, _>(
                    r#"
                    const fs = __canaryoBuiltins.fs;
                    const path = __canaryoBuiltins.path;
                    const file = path.join(fixtureDirectory, "watched.txt");
                    const added = path.join(fixtureDirectory, "added.txt");
                    fs.writeFileSync(file, "before");
                    globalThis.watchEvents = [];
                    globalThis.watchFileEvents = 0;
                    const directoryWatcher = fs.watch(fixtureDirectory, { interval: 0 }, (type, name) => {
                        watchEvents.push(`${type}:${name}`);
                        if (watchEvents.some(value => value === "change:watched.txt") &&
                            watchEvents.some(value => value === "rename:added.txt")) directoryWatcher.close();
                    });
                    const statListener = (current, previous) => {
                        if (current.size !== previous.size) {
                            watchFileEvents++;
                            fs.unwatchFile(file, statListener);
                        }
                    };
                    const statWatcher = fs.watchFile(file, { interval: 0 }, statListener);
                    const controller = new AbortController();
                    const abortedWatcher = fs.watch(file, { interval: 0, signal: controller.signal });
                    controller.abort();
                    setImmediate(() => {
                        fs.writeFileSync(file, "after-change");
                        fs.writeFileSync(added, "new");
                    });
                    directoryWatcher instanceof fs.FSWatcher && statWatcher instanceof fs.StatWatcher &&
                        directoryWatcher.hasRef() && !abortedWatcher.hasRef()
                    "#,
                )
                .unwrap();
            assert!(initialized);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            for _ in 0..8 {
                run_timers.call::<_, i64>(()).unwrap();
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .eval::<bool, _>(
                        "watchFileEvents === 1 && watchEvents.includes('change:watched.txt') && watchEvents.includes('rename:added.txt')",
                    )
                    .unwrap()
            );
        });

        fs::remove_dir_all(directory).unwrap();
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
    fn pipes_and_transforms_web_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const web = __canaryoBuiltins["stream/web"];
                    globalThis.webStreamOutput = "";
                    globalThis.webStreamDone = false;
                    const source = new web.ReadableStream({
                        start(controller) {
                            controller.enqueue("can");
                            controller.enqueue("aryo");
                            controller.close();
                        }
                    });
                    const upper = new web.TransformStream({
                        transform(chunk, controller) { controller.enqueue(chunk.toUpperCase()); }
                    });
                    const destination = new web.WritableStream({
                        write(chunk) { webStreamOutput += chunk; }
                    });
                    source.pipeThrough(upper).pipeTo(destination).then(() => {
                        webStreamDone = webStreamOutput === "CANARYO" &&
                            !source.locked && !upper.readable.locked && !destination.locked &&
                            globalThis.ReadableStream === web.ReadableStream &&
                            new web.CountQueuingStrategy({ highWaterMark: 2 }).size() === 1 &&
                            new web.ByteLengthQueuingStrategy({ highWaterMark: 4 }).size(new Uint8Array(3)) === 3;
                    });
                    "#,
                )
                .unwrap();

            for _ in 0..20 {
                while context.execute_pending_job() {}
            }

            assert_eq!(
                context
                    .globals()
                    .get::<_, String>("webStreamOutput")
                    .unwrap(),
                "CANARYO"
            );
            assert!(context.globals().get::<_, bool>("webStreamDone").unwrap());
        });
    }

    #[test]
    fn converts_between_node_and_web_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const { Readable } = __canaryoBuiltins.stream;
                    globalThis.nodeToWebOutput = "";
                    globalThis.webToNodeOutput = "";
                    globalThis.streamConversionsDone = false;

                    const nodeSource = new Readable();
                    const reader = Readable.toWeb(nodeSource).getReader();
                    nodeSource.push("node-");
                    nodeSource.push("web");
                    nodeSource.push(null);
                    (async () => {
                        while (true) {
                            const result = await reader.read();
                            if (result.done) break;
                            nodeToWebOutput += Buffer.from(result.value).toString();
                        }
                        reader.releaseLock();
                    })();

                    const webSource = new ReadableStream({
                        start(controller) {
                            controller.enqueue(new TextEncoder().encode("web-"));
                            controller.enqueue(Buffer.from("node"));
                            controller.close();
                        }
                    });
                    const converted = Readable.fromWeb(webSource);
                    converted.on("data", chunk => { webToNodeOutput += Buffer.from(chunk).toString(); });
                    converted.on("end", () => {
                        streamConversionsDone = nodeToWebOutput === "node-web" && webToNodeOutput === "web-node";
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
                    .get::<_, bool>("streamConversionsDone")
                    .unwrap()
            );
        });
    }

    #[test]
    fn provides_fetch_request_response_and_headers_values() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.fetchValuesWorked = false;
                    const headers = new Headers([["X-Test", "one"], ["x-test", "two"]]);
                    headers.append("set-cookie", "first=1");
                    headers.append("set-cookie", "second=2");
                    const response = Response.json({ answer: 42 }, {
                        status: 201,
                        headers: { "x-response": "yes" }
                    });
                    const responseClone = response.clone();
                    const request = new Request("https://example.com/items", {
                        method: "POST",
                        body: new URLSearchParams({ page: "2" })
                    });
                    const requestClone = request.clone();
                    Promise.all([
                        response.json(),
                        responseClone.text(),
                        request.text(),
                        requestClone.arrayBuffer()
                    ]).then(([json, responseText, requestText, requestBytes]) => {
                        fetchValuesWorked = json.answer === 42 &&
                            responseText === '{"answer":42}' && response.bodyUsed &&
                            response.status === 201 && response.ok &&
                            response.headers.get("x-response") === "yes" &&
                            headers.get("X-Test") === "one, two" &&
                            headers.getSetCookie().join(";") === "first=1;second=2" &&
                            requestText === "page=2" && request.bodyUsed &&
                            Buffer.from(requestBytes).toString() === "page=2" &&
                            request.headers.get("content-type") === "application/x-www-form-urlencoded;charset=UTF-8" &&
                            Object.prototype.toString.call(response) === "[object Response]";
                    });
                    "#,
                )
                .unwrap();

            for _ in 0..30 {
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .globals()
                    .get::<_, bool>("fetchValuesWorked")
                    .unwrap()
            );
        });
    }

    #[test]
    fn decodes_split_text_and_node_query_strings() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let result = context
                .eval::<String, _>(
                    r#"
                    const { StringDecoder } = __canaryoBuiltins.string_decoder;
                    const querystring = __canaryoBuiltins.querystring;
                    const utf8 = new StringDecoder("utf8");
                    const encoded = Buffer.from("Canário 🐦");
                    const decoded = utf8.write(encoded.subarray(0, 4)) +
                        utf8.write(encoded.subarray(4, 10)) + utf8.end(encoded.subarray(10));
                    const utf16 = new StringDecoder("utf16le");
                    const utf16Bytes = Buffer.from("A🐦B", "utf16le");
                    const decoded16 = utf16.write(utf16Bytes.subarray(0, 3)) + utf16.end(utf16Bytes.subarray(3));
                    const base64 = new StringDecoder("base64");
                    const base64Value = base64.write(Buffer.from([1, 2])) + base64.end(Buffer.from([3, 4]));
                    const parsed = querystring.parse("tag=rust&tag=js&message=hello+world&empty");
                    const custom = querystring.parse("a:1;a:2", ";", ":");
                    JSON.stringify({
                        decoded,
                        decoded16,
                        base64Value,
                        tags: parsed.tag.join(","),
                        message: parsed.message,
                        empty: parsed.empty,
                        nullPrototype: Object.getPrototypeOf(parsed) === null,
                        custom: custom.a.join(","),
                        stringified: querystring.stringify({ tag: ["rust", "js"], enabled: true }),
                        aliases: querystring.decode === querystring.parse && querystring.encode === querystring.stringify
                    })
                    "#,
                )
                .unwrap();

            assert_eq!(
                result,
                r#"{"decoded":"Canário 🐦","decoded16":"A🐦B","base64Value":"AQIDBA==","tags":"rust,js","message":"hello world","empty":"","nullPrototype":true,"custom":"1,2","stringified":"tag=rust&tag=js&enabled=true","aliases":true}"#
            );
        });
    }

    #[test]
    fn supports_node_assertion_families() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    const looseAssert = __canaryoBuiltins.assert;
                    const strictAssert = __canaryoBuiltins["assert/strict"];
                    globalThis.assertionsPassed = false;
                    (async () => {
                        looseAssert.equal("1", 1);
                        let strictRejected = false;
                        try { strictAssert.equal("1", 1); }
                        catch (error) {
                            strictRejected = error instanceof looseAssert.AssertionError &&
                                error.code === "ERR_ASSERTION" && error.actual === "1";
                        }
                        const left = { values: [1, { runtime: "canaryo" }], map: new Map([["ready", true]]) };
                        left.self = left;
                        const right = { values: [1, { runtime: "canaryo" }], map: new Map([["ready", true]]) };
                        right.self = right;
                        strictAssert.deepEqual(left, right);
                        const thrown = looseAssert.throws(
                            () => { throw Object.assign(new Error("boom"), { code: "EBOOM" }); },
                            { code: "EBOOM", message: /boom/ }
                        );
                        await looseAssert.rejects(Promise.reject(new TypeError("async boom")), TypeError);
                        await looseAssert.doesNotReject(Promise.resolve(42));
                        looseAssert.match("canaryo", /naryo/);
                        looseAssert.doesNotMatch("canaryo", /node/);
                        looseAssert.ifError(null);
                        assertionsPassed = strictRejected && thrown.code === "EBOOM" &&
                            strictAssert.strict === strictAssert && looseAssert.strict === strictAssert;
                    })();
                    "#,
                )
                .unwrap();

            for _ in 0..20 {
                while context.execute_pending_job() {}
            }

            assert!(
                context
                    .globals()
                    .get::<_, bool>("assertionsPassed")
                    .unwrap()
            );
        });
    }

    #[test]
    fn records_performance_entries_and_histograms() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let result = context
                .eval::<String, _>(
                    r#"
                    const hooks = __canaryoBuiltins.perf_hooks;
                    const observer = new hooks.PerformanceObserver(() => {});
                    observer.observe({ entryTypes: ["mark", "measure", "function"] });
                    performance.mark("start", { startTime: 10, detail: { phase: 1 } });
                    performance.mark("end", { startTime: 25 });
                    const measure = performance.measure("work", "start", "end");
                    const histogram = hooks.createHistogram();
                    const timed = performance.timerify(value => value * 2, { histogram });
                    const timedResult = timed(21);
                    const observed = observer.takeRecords();
                    performance.clearMarks("end");
                    const delay = hooks.monitorEventLoopDelay();
                    const value = JSON.stringify({
                        relativeNow: performance.now() >= 0 && performance.now() < Date.now(),
                        measure: [measure.name, measure.startTime, measure.duration],
                        detail: performance.getEntriesByName("start", "mark")[0].detail.phase,
                        marksAfterClear: performance.getEntriesByType("mark").map(entry => entry.name),
                        observedTypes: observed.map(entry => entry.entryType).join(","),
                        timedResult,
                        histogramCount: histogram.count,
                        histogramPercentile: histogram.percentile(50) >= 1,
                        delayShape: delay.enable() && delay.disable() && delay.hasRef() === false,
                        constructors: hooks.PerformanceEntry === PerformanceEntry &&
                            hooks.PerformanceObserver === PerformanceObserver
                    });
                    observer.disconnect();
                    value
                    "#,
                )
                .unwrap();

            assert_eq!(
                result,
                r#"{"relativeNow":true,"measure":["work",10,15],"detail":1,"marksAfterClear":["start"],"observedTypes":"mark,mark,measure,function","timedResult":42,"histogramCount":1,"histogramPercentile":true,"delayShape":true,"constructors":true}"#
            );
        });
    }

    #[test]
    fn serializes_and_parses_form_data_values() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.formDataWorked = false;
                    const source = new FormData();
                    source.append("name", "canaryo");
                    source.append("tag", "rust");
                    source.append("tag", "javascript");
                    source.append("file", new File([new Uint8Array([0, 1, 2, 255])], "data.bin", {
                        type: "application/octet-stream"
                    }));
                    const request = new Request("https://example.com/upload", {
                        method: "POST",
                        body: source
                    });
                    const urlEncoded = new Response("item=one&item=two", {
                        headers: { "content-type": "application/x-www-form-urlencoded" }
                    });
                    Promise.all([request.formData(), urlEncoded.formData()]).then(([multipart, encoded]) => {
                        const file = multipart.get("file");
                        formDataWorked = request.headers.get("content-type").startsWith("multipart/form-data; boundary=") &&
                            multipart.get("name") === "canaryo" &&
                            multipart.getAll("tag").join(",") === "rust,javascript" &&
                            file instanceof File && file.name === "data.bin" &&
                            file.type === "application/octet-stream" && file.size === 4 &&
                            encoded.getAll("item").join(",") === "one,two";
                    });
                    "#,
                )
                .unwrap();

            for _ in 0..30 {
                while context.execute_pending_job() {}
            }

            assert!(context.globals().get::<_, bool>("formDataWorked").unwrap());
        });
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
                    const sent = { runtime: "canaryo", bytes: new Uint8Array([1, 2, 3]) };
                    sent.self = sent;
                    channel.port1.postMessage(sent);
                    sent.runtime = "changed";
                    const broadcast1 = new workers.BroadcastChannel("canaryo");
                    const broadcast2 = new BroadcastChannel("canaryo");
                    globalThis.broadcastMessage = null;
                    broadcast2.onmessage = event => { broadcastMessage = event.data; };
                    broadcast1.postMessage({ kind: "broadcast" });
                    const marked = {};
                    workers.markAsUntransferable(marked);
                    const uncloneable = {};
                    workers.markAsUncloneable(uncloneable);
                    let cloneRejected = false;
                    try { structuredClone(uncloneable); }
                    catch (error) { cloneRejected = error.name === "DataCloneError"; }
                    let unsupportedWorker = false;
                    try { new workers.Worker("worker.js"); }
                    catch (error) { unsupportedWorker = error.code === "ERR_WORKER_UNSUPPORTED_OPERATION"; }
                    workers.isMainThread && workers.threadId === 0 && workers.parentPort === null &&
                        workers.getEnvironmentData("canaryo") === 42 && unsupportedWorker && cloneRejected &&
                        workers.isMarkedAsUntransferable(marked) && MessageEvent.prototype instanceof Event
                    "#,
                )
                .unwrap();
            assert!(initialized);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            run_timers.call::<_, i64>(()).unwrap();

            assert!(
                context
                    .eval::<bool, _>(
                        "workerMessage.runtime === 'canaryo' && workerMessage.self === workerMessage && workerMessage.bytes[2] === 3 && broadcastMessage.kind === 'broadcast'",
                    )
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
                        Module.isBuiltin("node:constants") && Module.isBuiltin("_stream_readable") &&
                        !Module.isBuiltin("left-pad") &&
                        Module.builtinModules.includes("fs/promises") &&
                        localRequire("node:path") === __canaryoBuiltins.path &&
                        localRequire("node:constants") === __canaryoBuiltins.fs.constants &&
                        localRequire("sys") === __canaryoBuiltins.util &&
                        localRequire("_stream_readable") === __canaryoBuiltins.stream.Readable &&
                        localRequire("node:_stream_writable") === __canaryoBuiltins.stream.Writable &&
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
                    const tty = __canaryoBuiltins.tty;
                    os.platform() === process.platform && os.arch() === process.arch &&
                        typeof os.type() === "string" && os.type().length > 0 &&
                        typeof os.tmpdir() === "string" && os.tmpdir().length > 0 &&
                        typeof os.homedir() === "string" && typeof os.hostname() === "string" &&
                        ["LE", "BE"].includes(os.endianness()) &&
                        os.availableParallelism() >= 1 &&
                        os.cpus().length === os.availableParallelism() &&
                        os.uptime() >= 0 && typeof os.userInfo().username === "string" &&
                        process.stdin instanceof tty.ReadStream && process.stdout instanceof tty.WriteStream &&
                        process.stderr instanceof tty.WriteStream && process.stdin.fd === 0 &&
                        process.stdout.fd === 1 && process.stderr.fd === 2 &&
                        tty.isatty(0) === process.stdin.isTTY && tty.isatty(1) === process.stdout.isTTY &&
                        tty.isatty(2) === process.stderr.isTTY && process.stdin.setRawMode(true).isRaw &&
                        process.stdout.getWindowSize().length === 2 &&
                        !process.stdout.hasColors(16, { FORCE_COLOR: "0" }) &&
                        typeof process.stdout.on === "function"
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
                        path.posix.parse("/one/file.js").root === "/" &&
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

    #[test]
    fn parses_structured_command_line_arguments() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const { parseArgs } = __canaryoBuiltins.util;
                    const parsed = parseArgs({
                        args: ["--port=3000", "-vv", "--name", "canaryo", "server.js", "--no-color", "--", "--literal"],
                        options: {
                            port: { type: "string" },
                            verbose: { type: "boolean", short: "v", multiple: true },
                            name: { type: "string", short: "n", default: "default" },
                            color: { type: "boolean", default: true }
                        },
                        strict: true,
                        allowPositionals: true,
                        allowNegative: true,
                        tokens: true
                    });
                    let strictError = false;
                    try { parseArgs({ args: ["--unknown"], options: {} }); }
                    catch (error) { strictError = error instanceof TypeError; }
                    Object.getPrototypeOf(parsed.values) === null &&
                        parsed.values.port === "3000" && parsed.values.name === "canaryo" &&
                        parsed.values.color === false && parsed.values.verbose.length === 2 &&
                        parsed.values.verbose.every(Boolean) &&
                        parsed.positionals.join(",") === "server.js,--literal" &&
                        parsed.tokens.length === 8 && parsed.tokens[0].inlineValue === true &&
                        parsed.tokens.some(token => token.kind === "option-terminator") && strictError
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn parses_and_serializes_mime_types() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const { MIMEType, MIMEParams } = __canaryoBuiltins.util;
                    const mime = new MIMEType('Text/HTML; Charset="utf-8"; boundary="a;b"; charset=ignored');
                    mime.type = "application";
                    mime.subtype = "json";
                    mime.params.set("Foo", "a b");
                    const entries = [...mime.params];
                    const standalone = new MIMEParams([["version", "1"]]);
                    let invalid = false;
                    try { new MIMEType("invalid"); } catch (error) { invalid = error instanceof TypeError; }
                    mime.essence === "application/json" && mime.params.get("CHARSET") === "utf-8" &&
                        mime.params.get("boundary") === "a;b" && mime.params.has("foo") &&
                        entries.length === 3 && mime.toString() === 'application/json;charset=utf-8;boundary="a;b";foo="a b"' &&
                        JSON.stringify(mime) === '"application/json;charset=utf-8;boundary=\\"a;b\\";foo=\\"a b\\""' &&
                        standalone.toString() === "version=1" && mime instanceof MIMEType &&
                        mime.params instanceof MIMEParams && invalid
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }
}
