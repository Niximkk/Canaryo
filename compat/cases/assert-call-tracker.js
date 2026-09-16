const assert = require("node:assert");

const tracker = new assert.CallTracker();
const tracked = tracker.calls(function add(value) { return this.base + value; }, 2);
const result = tracked.call({ base: 3 }, 4);
const calls = tracker.getCalls(tracked).map(call => ({
    thisArg: call.thisArg,
    arguments: call.arguments
}));
const report = tracker.report().map(({ message, actual, expected, operator }) => ({
    message,
    actual,
    expected,
    operator
}));
let verification;
try {
    tracker.verify();
} catch (error) {
    verification = { name: error.name, code: error.code, message: error.message };
}
tracked.call({ base: 5 }, 6);

console.log(JSON.stringify({
    result,
    calls,
    report,
    verification,
    finalReport: tracker.report()
}));
