const assert = require("node:assert");

function passes(actual, expected) {
    try {
        assert.partialDeepStrictEqual(actual, expected);
        return true;
    } catch (error) {
        return error.code === "ERR_ASSERTION" ? false : error.name;
    }
}

console.log(JSON.stringify([
    passes({ first: 1, second: 2 }, { first: 1 }),
    passes({ nested: { first: 1, second: 2 } }, { nested: { second: 2 } }),
    passes([1, 2, 3], [2, 3]),
    passes(new Set([1, 2]), new Set([2])),
    passes(new Map([["first", 1], ["second", 2]]), new Map([["second", 2]])),
    passes({ value: 1 }, { value: "1" }),
    passes([1], [1, 2])
]));
