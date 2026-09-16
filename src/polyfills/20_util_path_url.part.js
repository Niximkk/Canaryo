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
    function toUSVString(value) {
        const source = String(value);
        let output = "";
        for (let index = 0; index < source.length; index++) {
            const code = source.charCodeAt(index);
            if (code >= 0xd800 && code <= 0xdbff) {
                const next = source.charCodeAt(index + 1);
                if (next >= 0xdc00 && next <= 0xdfff) {
                    output += source[index] + source[++index];
                } else {
                    output += "\ufffd";
                }
            } else if (code >= 0xdc00 && code <= 0xdfff) {
                output += "\ufffd";
            } else {
                output += source[index];
            }
        }
        return output;
    }
    function parseEnv(source) {
        const result = {};
        const lines = String(source).replace(/^\ufeff/, "").split(/\r?\n/);
        for (let index = 0; index < lines.length; index++) {
            let line = lines[index];
            line = line.trim();
            if (!line || line.startsWith("#")) continue;
            line = line.replace(/^export\s+/, "");
            const match = line.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/);
            if (!match) continue;
            let value = match[2];
            const quote = value[0] === '"' || value[0] === "'" ? value[0] : "";
            while (quote && (value.length === 1 || !value.endsWith(quote)) && index + 1 < lines.length) {
                value += `\n${lines[++index]}`;
            }
            if (quote && value.endsWith(quote)) {
                value = value.slice(1, -1);
            } else if (value.startsWith("#")) {
                value = "";
            } else {
                value = value.trimEnd();
            }
            result[match[1]] = value;
        }
        return Object.fromEntries(Object.entries(result).sort(([left], [right]) => left.localeCompare(right)));
    }
    const util = {
        _extend(target, source) { return Object.assign(target, source); },
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
        parseEnv,
        MIMEType,
        MIMEParams,
        isArray: Array.isArray,
        isBoolean: value => typeof value === "boolean",
        isBuffer: value => value instanceof Buffer,
        isDate: value => value instanceof Date,
        isDeepStrictEqual: (left, right) => deepEquals(left, right, true),
        isError: value => value instanceof Error || Object.prototype.toString.call(value) === "[object Error]",
        isFunction: value => typeof value === "function",
        isNull: value => value === null,
        isNullOrUndefined: value => value === null || value === undefined,
        isNumber: value => typeof value === "number",
        isObject: value => value !== null && typeof value === "object",
        isPrimitive: value => value === null || (typeof value !== "object" && typeof value !== "function"),
        isRegExp: value => value instanceof RegExp,
        isString: value => typeof value === "string",
        isSymbol: value => typeof value === "symbol",
        isUndefined: value => value === undefined,
        toUSVString,
        stripVTControlCharacters(value) { return String(value).replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, ""); },
        TextEncoder,
        TextDecoder,
        types: utilTypes
    };

    function createPathApi(separator, delimiter, windows) {
        const toSlashes = value => windows ? String(value).replace(/\\/g, "/") : String(value);
        function matchesGlob(pathname, pattern) {
            const source = toSlashes(pathname);
            const glob = toSlashes(pattern);
            let expression = "^";
            let segmentStart = true;
            for (let index = 0; index < glob.length;) {
                const character = glob[index];
                const next = glob[index + 1];
                if ("?+*@".includes(character) && next === "(") {
                    const end = glob.indexOf(")", index + 2);
                    if (end !== -1) {
                        const alternatives = glob.slice(index + 2, end).split("|")
                            .map(value => value.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&"));
                        if (segmentStart) expression += "(?!\\.)";
                        const quantifier = character === "?" ? "?" : character === "+" ? "+" : "*";
                        expression += `(?:${alternatives.join("|")})${quantifier}`;
                        index = end + 1;
                        segmentStart = false;
                        continue;
                    }
                }
                if (character === "*") {
                    if (segmentStart) expression += "(?!\\.)";
                    if (next === "*") {
                        index += 2;
                        if (glob[index] === "/") {
                            expression += "(?:(?!\\.)[^/]+/)*";
                            index++;
                            segmentStart = true;
                        } else {
                            expression += ".*";
                            segmentStart = false;
                        }
                    } else {
                        expression += "[^/]*";
                        index++;
                        segmentStart = false;
                    }
                    continue;
                }
                if (character === "?") {
                    if (segmentStart) expression += "(?!\\.)";
                    expression += "[^/]";
                    index++;
                    segmentStart = false;
                    continue;
                }
                if (character === "[") {
                    const end = glob.indexOf("]", index + 1);
                    if (end !== -1) {
                        if (segmentStart) expression += "(?!\\.)";
                        const contents = glob.slice(index + 1, end);
                        expression += `[${contents.startsWith("!") ? `^${contents.slice(1)}` : contents}]`;
                        index = end + 1;
                        segmentStart = false;
                        continue;
                    }
                }
                if (character === "{") {
                    const end = glob.indexOf("}", index + 1);
                    if (end !== -1) {
                        const alternatives = glob.slice(index + 1, end).split(",")
                            .map(value => value.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&"));
                        expression += `(?:${alternatives.join("|")})`;
                        index = end + 1;
                        segmentStart = false;
                        continue;
                    }
                }
                if (character === "/") {
                    expression += "/";
                    segmentStart = true;
                } else {
                    expression += character.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&");
                    segmentStart = false;
                }
                index++;
            }
            return new RegExp(`${expression}$`, windows ? "i" : "").test(source);
        }
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
        function toNamespacedPath(value) {
            const source = String(value);
            if (!windows || !isAbsolute(source) || source.startsWith("\\\\?\\") || source.startsWith("\\\\.\\")) return source;
            if (source.startsWith("\\\\")) return `\\\\?\\UNC\\${source.slice(2)}`;
            return `\\\\?\\${source}`;
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
                const parsedRoot = root && rootOf(normalized).absolute && windows ? `${root}/` : root;
                return { root: separator === "/" ? parsedRoot : parsedRoot.replace(/\//g, separator), dir, base, ext, name: base.slice(0, base.length - ext.length) };
            },
            format(value) {
                const base = value.base || `${value.name || ""}${value.ext || ""}`;
                return value.dir ? `${value.dir}${value.dir.endsWith(separator) ? "" : separator}${base}` : `${value.root || ""}${base}`;
            },
            toNamespacedPath,
            _makeLong: toNamespacedPath,
            matchesGlob
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
        static createObjectURL(blob) {
            if (!(blob instanceof Blob)) throw new TypeError("The object must be a Blob");
            const identifier = `blob:nodedata:${randomUUID()}`;
            objectUrlRegistry.set(identifier, blob);
            return identifier;
        }
        static revokeObjectURL(identifier) {
            objectUrlRegistry.delete(String(identifier));
        }
    }
    const objectUrlRegistry = new Map();
    function resolveObjectURL(identifier) { return objectUrlRegistry.get(String(identifier)); }
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
