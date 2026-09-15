const { Console } = require("node:console");
const { Writable } = require("node:stream");

const output = [];
const errors = [];
const stdout = new Writable({ write(value, _encoding, callback) { output.push(String(value)); callback(); } });
const stderr = new Writable({ write(value, _encoding, callback) { errors.push(String(value)); callback(); } });
const logger = new Console(stdout, stderr);

logger.log("hello", 2);
logger.warn("careful");
logger.assert(true, "hidden");
logger.assert(false, "failed", 3);
logger.count("job");
logger.count("job");
logger.countReset("job");
logger.count("job");
logger.group("scope");
logger.log("inside");
logger.groupEnd();
logger.log("outside");

const requiredMethods = [
    "assert", "clear", "count", "countReset", "dir", "dirxml", "group",
    "groupCollapsed", "groupEnd", "table", "time", "timeEnd", "timeLog",
    "timeStamp", "trace", "profile", "profileEnd"
];

console.log(JSON.stringify({
    output,
    errors,
    globalMethods: requiredMethods.every(name => typeof console[name] === "function")
}));
