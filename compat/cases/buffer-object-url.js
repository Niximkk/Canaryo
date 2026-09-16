const { Blob, resolveObjectURL } = require("node:buffer");

const blob = new Blob(["canaryo"], { type: "text/plain" });
const identifier = URL.createObjectURL(blob);
const resolved = resolveObjectURL(identifier);
URL.revokeObjectURL(identifier);

console.log(JSON.stringify({
    prefix: identifier.startsWith("blob:nodedata:"),
    resolved: resolved instanceof Blob,
    size: resolved.size,
    type: resolved.type,
    revoked: resolveObjectURL(identifier) === undefined
}));
