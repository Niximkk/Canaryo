const util = require("node:util");

const values = [
    undefined,
    null,
    true,
    1,
    "text",
    Symbol.for("symbol"),
    () => {},
    [],
    new Date(0),
    /value/,
    new Error("value"),
    Buffer.from("value")
];

const predicates = [
    "isArray",
    "isBoolean",
    "isNull",
    "isNullOrUndefined",
    "isNumber",
    "isString",
    "isSymbol",
    "isUndefined",
    "isRegExp",
    "isObject",
    "isDate",
    "isError",
    "isFunction",
    "isPrimitive",
    "isBuffer"
];

console.log(JSON.stringify({
    predicates: predicates.map(name => [name, values.map(value => util[name](value))]),
    deep: [
        util.isDeepStrictEqual({ value: [1, NaN] }, { value: [1, NaN] }),
        util.isDeepStrictEqual({ value: 1 }, { value: "1" })
    ],
    usv: util.toUSVString("\ud800A\udc00"),
    extended: util._extend({ first: 1, replaced: false }, { second: 2, replaced: true }),
    environment: util.parseEnv("A=one\nB=\"two words\"\nexport C=three\nD=hello # comment\n"),
    environmentEdges: util.parseEnv("HASH=#value\nSINGLE='one # two'\nDOUBLE=\"line one\nline two\"\nEMPTY=\nSPACED = trimmed   \n")
}));
