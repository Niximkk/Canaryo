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
    const zlibCodes = {
        0: "Z_OK", 1: "Z_STREAM_END", 2: "Z_NEED_DICT",
        "-1": "Z_ERRNO", "-2": "Z_STREAM_ERROR", "-3": "Z_DATA_ERROR",
        "-4": "Z_MEM_ERROR", "-5": "Z_BUF_ERROR", "-6": "Z_VERSION_ERROR",
        Z_OK: 0, Z_STREAM_END: 1, Z_NEED_DICT: 2, Z_ERRNO: -1,
        Z_STREAM_ERROR: -2, Z_DATA_ERROR: -3, Z_MEM_ERROR: -4,
        Z_BUF_ERROR: -5, Z_VERSION_ERROR: -6
    };
    function crc32(data, value = 0) {
        const bytes = typeof data === "string" ? Buffer.from(data) : Buffer.from(data);
        let checksum = (Number(value) >>> 0) ^ 0xffffffff;
        for (const byte of bytes) {
            checksum ^= byte;
            for (let bit = 0; bit < 8; bit++) {
                checksum = (checksum >>> 1) ^ (checksum & 1 ? 0xedb88320 : 0);
            }
        }
        return (checksum ^ 0xffffffff) >>> 0;
    }
    const zlibModule = {
        codes: zlibCodes,
        crc32,
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
            Z_OK: 0,
            Z_STREAM_END: 1,
            Z_NEED_DICT: 2,
            Z_ERRNO: -1,
            Z_STREAM_ERROR: -2,
            Z_DATA_ERROR: -3,
            Z_MEM_ERROR: -4,
            Z_BUF_ERROR: -5,
            Z_VERSION_ERROR: -6,
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
    for (const name of [
        "Z_NO_FLUSH", "Z_SYNC_FLUSH", "Z_FULL_FLUSH", "Z_FINISH",
        "Z_DEFAULT_COMPRESSION", "Z_BEST_SPEED", "Z_BEST_COMPRESSION", "Z_DEFAULT_STRATEGY"
    ]) {
        zlibModule[name] = zlibModule.constants[name];
    }

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
            executionAsyncResource,
            asyncWrapProviders
        },
        buffer: {
            Buffer,
            Blob,
            File,
            SlowBuffer: Buffer,
            atob,
            btoa,
            resolveObjectURL,
            isAscii,
            isUtf8,
            transcode,
            INSPECT_MAX_BYTES: 50,
            kMaxLength: 0x7fffffff,
            kStringMaxLength: 0x1fffffe8,
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
            devNull: process.platform === "win32" ? "\\\\.\\nul" : "/dev/null",
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
            networkInterfaces: () => ({}),
            loadavg: () => [0, 0, 0]
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
        timers: {
            setImmediate,
            clearImmediate: clearTimer,
            setTimeout,
            clearTimeout: clearTimer,
            setInterval,
            clearInterval: clearTimer,
            active,
            _unrefActive: unrefActive,
            enroll,
            unenroll,
            promises: timersPromises
        },
        "timers/promises": timersPromises,
        tty: { isatty: terminalStatus, ReadStream: TTYReadStream, WriteStream: TTYWriteStream },
        url: {
            URL,
            URLSearchParams,
            Url,
            parse: parseUrl,
            format: formatLegacyUrl,
            resolve: (base, target) => resolveUrlObject(base, target).href,
            resolveObject: resolveUrlObject,
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
