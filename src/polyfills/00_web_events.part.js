(() => {
    const disposeSymbol = Symbol.dispose || Symbol.for("nodejs.dispose");
    let defaultMaxListeners = 10;

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
    EventTarget.prototype.getMaxListeners = function () { return this._maxListeners ?? defaultMaxListeners; };
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
    EventEmitter.usingDomains = false;
    EventEmitter.init = function (options) { EventEmitter.call(this, options); };
    Object.defineProperty(EventEmitter, "defaultMaxListeners", {
        enumerable: true,
        configurable: true,
        get() { return defaultMaxListeners; },
        set(value) {
            const number = Number(value);
            if (!Number.isFinite(number) || number < 0) throw new RangeError("defaultMaxListeners must be a non-negative number");
            defaultMaxListeners = number;
        }
    });
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
    EventEmitter.prototype.getMaxListeners = function () { return this._maxListeners ?? defaultMaxListeners; };
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
        if (emitters.length === 0) {
            EventEmitter.defaultMaxListeners = value;
            return;
        }
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
