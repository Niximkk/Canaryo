const zlib = require("node:zlib");

const names = [
    "Z_NO_FLUSH",
    "Z_SYNC_FLUSH",
    "Z_FULL_FLUSH",
    "Z_FINISH",
    "Z_DEFAULT_COMPRESSION",
    "Z_BEST_SPEED",
    "Z_BEST_COMPRESSION",
    "Z_DEFAULT_STRATEGY",
    "BROTLI_OPERATION_PROCESS",
    "BROTLI_OPERATION_FLUSH",
    "BROTLI_OPERATION_FINISH"
];

console.log(JSON.stringify({
    values: Object.fromEntries(names.map(name => [name, zlib[name]])),
    matchConstantsObject: names.every(name => zlib[name] === zlib.constants[name])
}));
