const zlib = require("node:zlib");

const first = zlib.crc32("hello");
const incremental = zlib.crc32("llo", zlib.crc32("he"));

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
    crc32: [first, incremental],
    codes: [
        zlib.codes.Z_OK,
        zlib.codes.Z_DATA_ERROR,
        zlib.codes[0],
        zlib.codes[-3]
    ],
    values: Object.fromEntries(names.map(name => [name, zlib[name]])),
    matchConstantsObject: names.every(name => zlib[name] === zlib.constants[name])
}));
