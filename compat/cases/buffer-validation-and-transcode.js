const buffer = require("node:buffer");

const storage = new Uint8Array([255, 65, 66, 255]);
const middle = storage.subarray(1, 3);
const utf16 = buffer.transcode(buffer.Buffer.from("Canário"), "utf8", "utf16le");

console.log(JSON.stringify({
    ascii: [
        buffer.isAscii(buffer.Buffer.from("plain text")),
        buffer.isAscii(buffer.Buffer.from("Canário")),
        buffer.isAscii(middle),
        buffer.isAscii(new ArrayBuffer(0))
    ],
    utf8: [
        buffer.isUtf8(buffer.Buffer.from("Canário 🐤")),
        buffer.isUtf8(buffer.Buffer.from([0xc0, 0xaf])),
        buffer.isUtf8(buffer.Buffer.from([0xf0, 0x9f, 0x90])),
        buffer.isUtf8(middle)
    ],
    transcoded: {
        hex: utf16.toString("hex"),
        roundTrip: buffer.transcode(utf16, "utf16le", "utf8").toString()
    },
    kStringMaxLength: buffer.kStringMaxLength
}));
