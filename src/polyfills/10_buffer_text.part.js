    function encodeUtf8(value) {
        const bytes = [];
        for (const character of String(value)) {
            let code = character.codePointAt(0);
            if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd;
            if (code <= 0x7f) bytes.push(code);
            else if (code <= 0x7ff) bytes.push(0xc0 | code >> 6, 0x80 | code & 0x3f);
            else if (code <= 0xffff) bytes.push(0xe0 | code >> 12, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
            else bytes.push(0xf0 | code >> 18, 0x80 | code >> 12 & 0x3f, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
        }
        return bytes;
    }
    function decodeUtf8(bytes) {
        let result = "";
        for (let index = 0; index < bytes.length;) {
            const first = bytes[index++];
            if (first < 0x80) result += String.fromCodePoint(first);
            else if (first < 0xe0) result += String.fromCodePoint((first & 0x1f) << 6 | bytes[index++] & 0x3f);
            else if (first < 0xf0) result += String.fromCodePoint((first & 0x0f) << 12 | (bytes[index++] & 0x3f) << 6 | bytes[index++] & 0x3f);
            else result += String.fromCodePoint((first & 0x07) << 18 | (bytes[index++] & 0x3f) << 12 | (bytes[index++] & 0x3f) << 6 | bytes[index++] & 0x3f);
        }
        return result;
    }
    function normalizeEncoding(encoding = "utf8") {
        const value = String(encoding).toLowerCase().replace(/[-_]/g, "");
        if (value === "utf8" || value === "utf") return "utf8";
        if (value === "utf16le" || value === "ucs2" || value === "ucs2le") return "utf16le";
        if (value === "latin1" || value === "binary") return "latin1";
        if (value === "ascii" || value === "hex" || value === "base64" || value === "base64url") return value;
        throw new TypeError(`Unknown encoding: ${encoding}`);
    }
    function decodeBase64(value) {
        const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        const input = String(value).replace(/-/g, "+").replace(/_/g, "/").replace(/[^A-Za-z0-9+/]/g, "");
        const output = [];
        let accumulator = 0;
        let bits = 0;
        for (const character of input) {
            const digit = alphabet.indexOf(character);
            if (digit < 0) continue;
            accumulator = accumulator << 6 | digit;
            bits += 6;
            if (bits >= 8) {
                bits -= 8;
                output.push(accumulator >> bits & 0xff);
            }
        }
        return output;
    }
    function encodeBase64(bytes, urlSafe = false) {
        const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let output = "";
        for (let index = 0; index < bytes.length; index += 3) {
            const remaining = bytes.length - index;
            const value = bytes[index] << 16 | (bytes[index + 1] || 0) << 8 | (bytes[index + 2] || 0);
            output += alphabet[value >> 18 & 63] + alphabet[value >> 12 & 63];
            output += remaining > 1 ? alphabet[value >> 6 & 63] : "=";
            output += remaining > 2 ? alphabet[value & 63] : "=";
        }
        return urlSafe ? output.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "") : output;
    }
    function encodeString(value, encoding) {
        const normalized = normalizeEncoding(encoding);
        if (normalized === "utf8") return __canaryoEncodeUtf8(String(value));
        if (normalized === "hex") {
            const input = String(value).match(/^[0-9a-fA-F]*/)[0];
            const bytes = [];
            for (let index = 0; index + 1 < input.length; index += 2) bytes.push(parseInt(input.slice(index, index + 2), 16));
            return bytes;
        }
        if (normalized === "base64" || normalized === "base64url") return decodeBase64(value);
        if (normalized === "utf16le") {
            const bytes = [];
            const input = String(value);
            for (let index = 0; index < input.length; index++) {
                const code = input.charCodeAt(index);
                bytes.push(code & 0xff, code >> 8);
            }
            return bytes;
        }
        const input = String(value);
        const bytes = [];
        for (let index = 0; index < input.length; index++) bytes.push(input.charCodeAt(index) & (normalized === "ascii" ? 0x7f : 0xff));
        return bytes;
    }
    function decodeBytes(bytes, encoding) {
        const normalized = normalizeEncoding(encoding);
        if (normalized === "utf8") return __canaryoDecodeUtf8(bytes);
        if (normalized === "hex") return [...bytes].map(byte => byte.toString(16).padStart(2, "0")).join("");
        if (normalized === "base64" || normalized === "base64url") return encodeBase64(bytes, normalized === "base64url");
        if (normalized === "utf16le") {
            let output = "";
            for (let index = 0; index + 1 < bytes.length; index += 2) output += String.fromCharCode(bytes[index] | bytes[index + 1] << 8);
            return output;
        }
        return [...bytes].map(byte => String.fromCharCode(normalized === "ascii" ? byte & 0x7f : byte)).join("");
    }
    function bufferSourceBytes(value) {
        if (value instanceof ArrayBuffer) return new Uint8Array(value);
        if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
        throw new TypeError("input must be an ArrayBuffer or ArrayBufferView");
    }
    function isAscii(value) {
        return bufferSourceBytes(value).every(byte => byte <= 0x7f);
    }
    function isUtf8(value) {
        const bytes = bufferSourceBytes(value);
        for (let index = 0; index < bytes.length;) {
            const first = bytes[index++];
            if (first <= 0x7f) continue;
            let remaining;
            let code;
            let minimum;
            if (first >= 0xc2 && first <= 0xdf) {
                remaining = 1;
                code = first & 0x1f;
                minimum = 0x80;
            } else if (first >= 0xe0 && first <= 0xef) {
                remaining = 2;
                code = first & 0x0f;
                minimum = 0x800;
            } else if (first >= 0xf0 && first <= 0xf4) {
                remaining = 3;
                code = first & 0x07;
                minimum = 0x10000;
            } else {
                return false;
            }
            if (index + remaining > bytes.length) return false;
            for (let offset = 0; offset < remaining; offset++) {
                const next = bytes[index++];
                if ((next & 0xc0) !== 0x80) return false;
                code = code << 6 | next & 0x3f;
            }
            if (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return false;
        }
        return true;
    }
    function transcode(source, fromEncoding, toEncoding) {
        const bytes = bufferSourceBytes(source);
        return Buffer.from(decodeBytes(bytes, fromEncoding), toEncoding);
    }
    class Buffer extends Uint8Array {
        constructor(...args) {
            super(0);
            const bytes = new Uint8Array(...args);
            Object.setPrototypeOf(bytes, new.target.prototype);
            return bytes;
        }
        static from(value, encodingOrOffset, length) {
            if (typeof value === "string") return new Buffer(encodeString(value, encodingOrOffset));
            if (ArrayBuffer.isView(value)) return new Buffer(value);
            if (value instanceof ArrayBuffer) return new Buffer(value, encodingOrOffset, length);
            if (value && value.type === "Buffer" && Array.isArray(value.data)) return new Buffer(value.data);
            return new Buffer(value);
        }
        static alloc(size, fill = 0, encoding) {
            const buffer = new Buffer(size);
            if (typeof fill === "string") {
                const pattern = Buffer.from(fill, encoding);
                if (pattern.length) {
                    for (let index = 0; index < buffer.length; index++) buffer[index] = pattern[index % pattern.length];
                }
            } else buffer.fill(fill);
            return buffer;
        }
        static allocUnsafe(size) { return new Buffer(size); }
        static allocUnsafeSlow(size) { return new Buffer(size); }
        static isBuffer(value) { return value instanceof Buffer; }
        static isEncoding(value) { try { normalizeEncoding(value); return true; } catch { return false; } }
        static byteLength(value, encoding) {
            if (typeof value === "string") return !encoding || normalizeEncoding(encoding) === "utf8" ? __canaryoByteLength(value) : encodeString(value, encoding).length;
            if (ArrayBuffer.isView(value)) return value.byteLength;
            if (value instanceof ArrayBuffer) return value.byteLength;
            return Buffer.from(value).length;
        }
        static compare(left, right) { return Buffer.from(left).compare(right); }
        static concat(list, totalLength) {
            if (!Array.isArray(list)) throw new TypeError("list must be an Array of Uint8Array instances");
            const length = totalLength === undefined
                ? list.reduce((sum, item) => sum + item.length, 0)
                : Number(totalLength);
            const result = new Buffer(length);
            let offset = 0;
            for (const item of list) {
                const count = Math.min(item.length, length - offset);
                if (count <= 0) break;
                result.set(item.subarray(0, count), offset);
                offset += count;
            }
            return result;
        }
        toString(encoding = "utf8", start = 0, end = this.length) { return decodeBytes(this.subarray(start, end), encoding); }
        equals(other) { return this.compare(other) === 0; }
        compare(other) {
            const right = Buffer.from(other);
            const length = Math.min(this.length, right.length);
            for (let index = 0; index < length; index++) if (this[index] !== right[index]) return this[index] < right[index] ? -1 : 1;
            return this.length === right.length ? 0 : this.length < right.length ? -1 : 1;
        }
        copy(target, targetStart = 0, sourceStart = 0, sourceEnd = this.length) {
            const source = this.subarray(sourceStart, sourceEnd);
            const count = Math.min(source.length, target.length - targetStart);
            if (count > 0) target.set(source.subarray(0, count), targetStart);
            return Math.max(0, count);
        }
        slice(start, end) { return this.subarray(start, end); }
        indexOf(value, byteOffset = 0, encoding) {
            const needle = typeof value === "number" ? Buffer.from([value & 0xff]) : Buffer.from(value, encoding);
            let start = Number(byteOffset) || 0;
            if (start < 0) start = Math.max(0, this.length + start);
            for (let index = start; index <= this.length - needle.length; index++) {
                let matches = true;
                for (let offset = 0; offset < needle.length; offset++) {
                    if (this[index + offset] !== needle[offset]) { matches = false; break; }
                }
                if (matches) return index;
            }
            return -1;
        }
        includes(value, byteOffset, encoding) { return this.indexOf(value, byteOffset, encoding) !== -1; }
        lastIndexOf(value, byteOffset = this.length - 1, encoding) {
            const needle = typeof value === "number" ? Buffer.from([value & 0xff]) : Buffer.from(value, encoding);
            let start = Number(byteOffset);
            if (!Number.isFinite(start)) start = this.length - 1;
            if (start < 0) start = this.length + start;
            start = Math.min(Math.trunc(start), this.length - needle.length);
            for (let index = start; index >= 0; index--) {
                let matches = true;
                for (let offset = 0; offset < needle.length; offset++) {
                    if (this[index + offset] !== needle[offset]) { matches = false; break; }
                }
                if (matches) return index;
            }
            return -1;
        }
        write(value, offset = 0, length = this.length - offset, encoding = "utf8") {
            if (typeof length === "string") { encoding = length; length = this.length - offset; }
            const source = Buffer.from(value, encoding);
            const count = Math.min(source.length, length, this.length - offset);
            this.set(source.subarray(0, count), offset);
            return count;
        }
        readUInt8(offset = 0) { return this[offset]; }
        readInt8(offset = 0) { const value = this.readUInt8(offset); return value & 0x80 ? value - 0x100 : value; }
        readUInt16LE(offset = 0) { return this[offset] | this[offset + 1] << 8; }
        readUInt16BE(offset = 0) { return this[offset] << 8 | this[offset + 1]; }
        readInt16LE(offset = 0) { const value = this.readUInt16LE(offset); return value & 0x8000 ? value - 0x10000 : value; }
        readInt16BE(offset = 0) { const value = this.readUInt16BE(offset); return value & 0x8000 ? value - 0x10000 : value; }
        readUInt32LE(offset = 0) { return (this[offset] | this[offset + 1] << 8 | this[offset + 2] << 16 | this[offset + 3] << 24) >>> 0; }
        readUInt32BE(offset = 0) { return (this[offset] << 24 | this[offset + 1] << 16 | this[offset + 2] << 8 | this[offset + 3]) >>> 0; }
        readInt32LE(offset = 0) { return this.readUInt32LE(offset) | 0; }
        readInt32BE(offset = 0) { return this.readUInt32BE(offset) | 0; }
        readUIntLE(offset, byteLength) {
            let value = 0;
            let multiplier = 1;
            for (let index = 0; index < byteLength; index++) { value += this[offset + index] * multiplier; multiplier *= 256; }
            return value;
        }
        readUIntBE(offset, byteLength) {
            let value = 0;
            for (let index = 0; index < byteLength; index++) value = value * 256 + this[offset + index];
            return value;
        }
        readIntLE(offset, byteLength) { const value = this.readUIntLE(offset, byteLength); const limit = 2 ** (byteLength * 8 - 1); return value >= limit ? value - 2 ** (byteLength * 8) : value; }
        readIntBE(offset, byteLength) { const value = this.readUIntBE(offset, byteLength); const limit = 2 ** (byteLength * 8 - 1); return value >= limit ? value - 2 ** (byteLength * 8) : value; }
        _dataView() { return new DataView(this.buffer, this.byteOffset, this.byteLength); }
        readFloatLE(offset = 0) { return this._dataView().getFloat32(offset, true); }
        readFloatBE(offset = 0) { return this._dataView().getFloat32(offset, false); }
        readDoubleLE(offset = 0) { return this._dataView().getFloat64(offset, true); }
        readDoubleBE(offset = 0) { return this._dataView().getFloat64(offset, false); }
        readBigUInt64LE(offset = 0) { return this._dataView().getBigUint64(offset, true); }
        readBigUInt64BE(offset = 0) { return this._dataView().getBigUint64(offset, false); }
        readBigInt64LE(offset = 0) { return this._dataView().getBigInt64(offset, true); }
        readBigInt64BE(offset = 0) { return this._dataView().getBigInt64(offset, false); }
        writeUInt8(value, offset = 0) { this[offset] = value; return offset + 1; }
        writeInt8(value, offset = 0) { return this.writeUInt8(value, offset); }
        writeUInt16LE(value, offset = 0) { this[offset] = value; this[offset + 1] = value >> 8; return offset + 2; }
        writeUInt16BE(value, offset = 0) { this[offset] = value >> 8; this[offset + 1] = value; return offset + 2; }
        writeInt16LE(value, offset = 0) { return this.writeUInt16LE(value, offset); }
        writeInt16BE(value, offset = 0) { return this.writeUInt16BE(value, offset); }
        writeUInt32LE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> index * 8; return offset + 4; }
        writeUInt32BE(value, offset = 0) { for (let index = 0; index < 4; index++) this[offset + index] = value >>> (3 - index) * 8; return offset + 4; }
        writeInt32LE(value, offset = 0) { return this.writeUInt32LE(value, offset); }
        writeInt32BE(value, offset = 0) { return this.writeUInt32BE(value, offset); }
        writeUIntLE(value, offset, byteLength) {
            let remaining = Number(value);
            for (let index = 0; index < byteLength; index++) { this[offset + index] = remaining % 256; remaining = Math.floor(remaining / 256); }
            return offset + byteLength;
        }
        writeUIntBE(value, offset, byteLength) {
            let remaining = Number(value);
            for (let index = byteLength - 1; index >= 0; index--) { this[offset + index] = remaining % 256; remaining = Math.floor(remaining / 256); }
            return offset + byteLength;
        }
        writeIntLE(value, offset, byteLength) { return this.writeUIntLE(value < 0 ? value + 2 ** (byteLength * 8) : value, offset, byteLength); }
        writeIntBE(value, offset, byteLength) { return this.writeUIntBE(value < 0 ? value + 2 ** (byteLength * 8) : value, offset, byteLength); }
        writeFloatLE(value, offset = 0) { this._dataView().setFloat32(offset, Number(value), true); return offset + 4; }
        writeFloatBE(value, offset = 0) { this._dataView().setFloat32(offset, Number(value), false); return offset + 4; }
        writeDoubleLE(value, offset = 0) { this._dataView().setFloat64(offset, Number(value), true); return offset + 8; }
        writeDoubleBE(value, offset = 0) { this._dataView().setFloat64(offset, Number(value), false); return offset + 8; }
        writeBigUInt64LE(value, offset = 0) { this._dataView().setBigUint64(offset, BigInt(value), true); return offset + 8; }
        writeBigUInt64BE(value, offset = 0) { this._dataView().setBigUint64(offset, BigInt(value), false); return offset + 8; }
        writeBigInt64LE(value, offset = 0) { this._dataView().setBigInt64(offset, BigInt(value), true); return offset + 8; }
        writeBigInt64BE(value, offset = 0) { this._dataView().setBigInt64(offset, BigInt(value), false); return offset + 8; }
        swap16() { return this._swap(2); }
        swap32() { return this._swap(4); }
        swap64() { return this._swap(8); }
        _swap(width) {
            if (this.length % width !== 0) throw new RangeError(`Buffer size must be a multiple of ${width * 8}-bits`);
            for (let offset = 0; offset < this.length; offset += width) {
                for (let left = 0, right = width - 1; left < right; left++, right--) {
                    const value = this[offset + left];
                    this[offset + left] = this[offset + right];
                    this[offset + right] = value;
                }
            }
            return this;
        }
        toJSON() { return { type: "Buffer", data: [...this] }; }
    }
    Buffer.poolSize = 8192;
    for (const method of ["from", "alloc", "allocUnsafe", "allocUnsafeSlow", "isBuffer", "isEncoding", "byteLength", "compare", "concat"]) {
        Object.defineProperty(Buffer, method, { enumerable: true });
    }

    class Blob {
        constructor(sources = [], options = {}) {
            const chunks = [];
            for (const source of sources) {
                if (source instanceof Blob) chunks.push(source._buffer);
                else if (typeof source === "string") chunks.push(Buffer.from(source));
                else if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
                    chunks.push(Buffer.from(source));
                } else chunks.push(Buffer.from(String(source)));
            }
            this._buffer = Buffer.concat(chunks);
            const type = String(options.type || "").toLowerCase();
            this.type = /^[\x20-\x7e]*$/.test(type) ? type : "";
        }
        get size() { return this._buffer.length; }
        arrayBuffer() {
            const bytes = new Uint8Array(this._buffer);
            return Promise.resolve(bytes.buffer);
        }
        bytes() { return Promise.resolve(new Uint8Array(this._buffer)); }
        text() { return Promise.resolve(this._buffer.toString()); }
        slice(start = 0, end = this.size, type = "") {
            const normalize = value => value < 0
                ? Math.max(this.size + Math.trunc(value), 0)
                : Math.min(Math.trunc(value), this.size);
            const first = normalize(Number(start) || 0);
            const last = normalize(end === undefined ? this.size : Number(end) || 0);
            return new Blob([this._buffer.subarray(first, Math.max(first, last))], { type });
        }
    }
    class File extends Blob {
        constructor(sources, name, options = {}) {
            super(sources, options);
            this.name = String(name);
            this.lastModified = options.lastModified === undefined
                ? Date.now()
                : Number(options.lastModified);
        }
    }
    function atob(value) {
        return decodeBase64(String(value)).map(byte => String.fromCharCode(byte)).join("");
    }
    function btoa(value) {
        const bytes = [];
        for (const character of String(value)) {
            const code = character.charCodeAt(0);
            if (code > 0xff) throw new DOMException("Invalid character", "InvalidCharacterError");
            bytes.push(code);
        }
        return encodeBase64(bytes);
    }

    class TextEncoder {
        encode(value) { return new Uint8Array(encodeUtf8(value)); }
    }

    class TextDecoder {
        decode(value = new Uint8Array()) { return decodeUtf8(value); }
    }

    class StringDecoder {
        constructor(encoding = "utf8") {
            this.encoding = normalizeEncoding(encoding);
            this._pending = [];
            this.lastNeed = 0;
            this.lastTotal = 0;
            this.lastChar = Buffer.alloc(4);
        }
        _split(bytes) {
            let complete = bytes.length;
            if (this.encoding === "utf8" && complete > 0) {
                let lead = complete - 1;
                while (lead >= 0 && (bytes[lead] & 0xc0) === 0x80) lead--;
                if (lead >= 0) {
                    const first = bytes[lead];
                    const expected = first >= 0xf0 && first <= 0xf4 ? 4
                        : first >= 0xe0 ? 3
                            : first >= 0xc2 ? 2
                                : 1;
                    if (complete - lead < expected) complete = lead;
                }
            } else if (this.encoding === "utf16le") {
                complete -= complete % 2;
                if (complete >= 2) {
                    const code = bytes[complete - 2] | bytes[complete - 1] << 8;
                    if (code >= 0xd800 && code <= 0xdbff) complete -= 2;
                }
            } else if (this.encoding === "base64" || this.encoding === "base64url") {
                complete -= complete % 3;
            }
            return complete;
        }
        write(value) {
            const bytes = this._pending.concat([...value]);
            const complete = this._split(bytes);
            this._pending = bytes.slice(complete);
            this.lastNeed = this._pending.length;
            this.lastTotal = this.lastNeed ? complete + this.lastNeed : 0;
            return decodeBytes(new Uint8Array(bytes.slice(0, complete)), this.encoding);
        }
        end(value) {
            let output = value === undefined ? "" : this.write(value);
            if (this._pending.length) {
                output += this.encoding === "utf8" || this.encoding === "utf16le"
                    ? "\ufffd"
                    : decodeBytes(new Uint8Array(this._pending), this.encoding);
            }
            this._pending = [];
            this.lastNeed = 0;
            this.lastTotal = 0;
            return output;
        }
        text(value, offset = 0) {
            this._pending = [];
            return this.write([...value].slice(offset));
        }
    }

    function queryUnescape(value, decode = decodeURIComponent) {
        const source = String(value).replace(/\+/g, " ");
        try { return decode(source); }
        catch {
            return source.replace(/(?:%[0-9a-fA-F]{2})+/g, sequence => {
                try { return decodeURIComponent(sequence); }
                catch { return sequence; }
            });
        }
    }
    function queryParse(value, separator = "&", equals = "=", options = {}) {
        const output = Object.create(null);
        const source = String(value);
        if (!source) return output;
        const maxKeys = options.maxKeys === undefined ? 1000 : Number(options.maxKeys);
        const parts = source.split(String(separator));
        const limit = maxKeys > 0 ? Math.min(parts.length, maxKeys) : parts.length;
        const decode = typeof options.decodeURIComponent === "function" ? options.decodeURIComponent : decodeURIComponent;
        for (let index = 0; index < limit; index++) {
            const part = parts[index];
            const equalAt = part.indexOf(String(equals));
            const key = queryUnescape(equalAt < 0 ? part : part.slice(0, equalAt), decode);
            const item = queryUnescape(equalAt < 0 ? "" : part.slice(equalAt + String(equals).length), decode);
            if (!(key in output)) output[key] = item;
            else if (Array.isArray(output[key])) output[key].push(item);
            else output[key] = [output[key], item];
        }
        return output;
    }
    function queryPrimitive(value) {
        return typeof value === "string" || typeof value === "number" || typeof value === "bigint" || typeof value === "boolean"
            ? String(value)
            : "";
    }
    function queryStringify(value, separator = "&", equals = "=", options = {}) {
        if (value === null || typeof value !== "object") return "";
        const encode = typeof options.encodeURIComponent === "function" ? options.encodeURIComponent : encodeURIComponent;
        const parts = [];
        for (const key of Object.keys(value)) {
            const encodedKey = encode(key);
            const items = Array.isArray(value[key]) ? value[key] : [value[key]];
            if (!items.length) parts.push(`${encodedKey}${equals}`);
            for (const item of items) parts.push(`${encodedKey}${equals}${encode(queryPrimitive(item))}`);
        }
        return parts.join(String(separator));
    }
    const querystringModule = {
        decode: queryParse,
        parse: queryParse,
        encode: queryStringify,
        stringify: queryStringify,
        escape: encodeURIComponent,
        unescape: queryUnescape,
        unescapeBuffer(value, decodeSpaces) {
            return Buffer.from(queryUnescape(decodeSpaces ? String(value).replace(/\+/g, " ") : value));
        }
    };
