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

