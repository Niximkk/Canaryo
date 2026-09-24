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

    class Performance {
        constructor() {
            const error = new TypeError("Illegal constructor");
            error.code = "ERR_ILLEGAL_CONSTRUCTOR";
            throw error;
        }
    }

    const resourceTimingToken = {};
    class PerformanceResourceTiming extends PerformanceEntry {
        constructor(token, timingInfo = {}, requestedUrl = "", initiatorType = "fetch") {
            if (token !== resourceTimingToken) {
                const error = new TypeError("Illegal constructor");
                error.code = "ERR_ILLEGAL_CONSTRUCTOR";
                throw error;
            }
            const startTime = Number(timingInfo.startTime) || 0;
            const responseEnd = Number(timingInfo.responseEnd) || startTime;
            super(requestedUrl, "resource", startTime, Math.max(0, responseEnd - startTime));
            this.initiatorType = String(initiatorType);
            for (const name of [
                "workerStart", "redirectStart", "redirectEnd", "fetchStart", "domainLookupStart",
                "domainLookupEnd", "connectStart", "connectEnd", "secureConnectionStart",
                "requestStart", "responseStart", "responseEnd", "encodedBodySize",
                "decodedBodySize", "transferSize", "responseStatus"
            ]) this[name] = Number(timingInfo[name]) || 0;
            this.nextHopProtocol = String(timingInfo.nextHopProtocol || "");
            this.deliveryType = String(timingInfo.deliveryType || "");
        }
        toJSON() {
            return Object.assign(super.toJSON(), {
                initiatorType: this.initiatorType,
                workerStart: this.workerStart,
                redirectStart: this.redirectStart,
                redirectEnd: this.redirectEnd,
                fetchStart: this.fetchStart,
                domainLookupStart: this.domainLookupStart,
                domainLookupEnd: this.domainLookupEnd,
                connectStart: this.connectStart,
                connectEnd: this.connectEnd,
                secureConnectionStart: this.secureConnectionStart,
                nextHopProtocol: this.nextHopProtocol,
                requestStart: this.requestStart,
                responseStart: this.responseStart,
                responseEnd: this.responseEnd,
                encodedBodySize: this.encodedBodySize,
                decodedBodySize: this.decodedBodySize,
                transferSize: this.transferSize,
                deliveryType: this.deliveryType,
                responseStatus: this.responseStatus
            });
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
            return publishPerformanceEntry(new PerformanceResourceTiming(
                resourceTimingToken,
                timingInfo,
                requestedUrl,
                initiatorType
            ));
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
    for (const name of Object.keys(performance)) {
        if (typeof performance[name] !== "function") continue;
        Object.defineProperty(Performance.prototype, name, {
            value: performance[name],
            configurable: true,
            writable: true
        });
        delete performance[name];
    }
    Performance.prototype.toJSON = function () {
        return {
            nodeTiming: this.nodeTiming,
            timeOrigin: this.timeOrigin,
            eventLoopUtilization: this.eventLoopUtilization()
        };
    };
    Object.setPrototypeOf(performance, Performance.prototype);

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
        Performance,
        PerformanceEntry,
        PerformanceMark,
        PerformanceMeasure,
        PerformanceObserver,
        PerformanceObserverEntryList,
        PerformanceResourceTiming,
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
        __canaryoMarkServerBusy();
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
    function enroll() {
        const item = arguments[0];
        const delay = Number(arguments[1]);
        if (!item || (typeof item !== "object" && typeof item !== "function")) throw new TypeError("item must be an object");
        if (item._idleHandle) clearTimer(item._idleHandle);
        item._idleTimeout = Number.isFinite(delay) && delay >= 0 ? delay : -1;
        item._idleNext = item;
        item._idlePrev = item;
    }
    function unenroll() {
        const item = arguments[0];
        if (!item || (typeof item !== "object" && typeof item !== "function")) return;
        const hadHandle = Boolean(item._idleHandle);
        const pending = hadHandle && scheduledTimers.has(Number(item._idleHandle));
        if (item._idleHandle) clearTimer(item._idleHandle);
        item._idleHandle = undefined;
        if (!hadHandle || pending) item._idleTimeout = -1;
    }
    function active() {
        const item = arguments[0];
        if (!item || typeof item._onTimeout !== "function" || item._idleTimeout < 0) return;
        if (item._idleHandle) clearTimer(item._idleHandle);
        item._idleHandle = setTimeout(() => item._onTimeout(), item._idleTimeout);
    }
    function unrefActive() {
        const item = arguments[0];
        active(item);
        item?._idleHandle?.unref();
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
