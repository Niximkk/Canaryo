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

