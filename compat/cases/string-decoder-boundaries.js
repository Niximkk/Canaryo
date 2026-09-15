const { StringDecoder } = require("node:string_decoder");

const decoder = new StringDecoder("utf8");
const bytes = Buffer.from("A🐤B");
const chunks = [bytes.subarray(0, 3), bytes.subarray(3, 5), bytes.subarray(5)];
console.log(JSON.stringify({
    decoded: chunks.map(chunk => decoder.write(chunk)).join("") + decoder.end(),
    lastNeed: decoder.lastNeed,
    encoding: decoder.encoding
}));
