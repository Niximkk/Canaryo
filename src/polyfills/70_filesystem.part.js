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
        if (Number(fd) === 1 || Number(fd) === 2) {
            let bytes;
            if (typeof value === "string") {
                bytes = Buffer.from(value, typeof lengthOrEncoding === "string" ? lengthOrEncoding : "utf8");
            } else {
                if (!ArrayBuffer.isView(value)) throw new TypeError("buffer must be an ArrayBuffer view");
                const offset = offsetOrPosition === undefined ? 0 : Number(offsetOrPosition);
                const length = lengthOrEncoding === undefined ? value.byteLength - offset : Number(lengthOrEncoding);
                if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < 0 || length < 0 || offset + length > value.byteLength) {
                    throw new RangeError("offset and length are outside the buffer");
                }
                bytes = Buffer.from(value).subarray(offset, offset + length);
            }
            if (Number(fd) === 2) __canaryoWriteError(bytes.toString());
            else __canaryoWrite(bytes.toString());
            return bytes.length;
        }
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
    Object.assign(fsModule, fsConstants, {
        FileReadStream: ReadStream,
        FileWriteStream: WriteStream
    });
    fsModule.realpath.native = fsModule.realpath;
