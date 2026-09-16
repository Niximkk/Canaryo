const stream = require("node:stream");

const [first, second] = stream.duplexPair();
const received = { first: [], second: [] };
const ended = { first: false, second: false };

first.on("data", chunk => received.first.push(chunk.toString()));
second.on("data", chunk => received.second.push(chunk.toString()));
first.on("end", () => { ended.first = true; });
second.on("end", () => { ended.second = true; });

first.write("from-first");
second.write("from-second");
first.end();
second.end();

const target = new stream.PassThrough();
let destructionError;
target.on("error", error => {
    destructionError = { name: error.name, code: error.code, message: error.message };
});
const destructionResult = stream.destroy(target);

const upper = new stream.Transform({
    transform(chunk, _encoding, callback) {
        callback(null, chunk.toString().toUpperCase());
    }
});
const suffix = new stream.Transform({
    transform(chunk, _encoding, callback) {
        callback(null, `${chunk.toString()}!`);
    }
});
const composed = stream.compose(upper, suffix);
const composedOutput = [];
composed.on("data", chunk => composedOutput.push(chunk.toString()));
composed.end("canaryo");

const single = new stream.PassThrough();
const singleCompositionIsIdentity = stream.compose(single) === single;

setImmediate(() => {
    console.log(JSON.stringify({
        arrayBufferViews: [
            stream._isArrayBufferView(new Uint8Array(1)),
            stream._isArrayBufferView(new DataView(new ArrayBuffer(1))),
            stream._isArrayBufferView(new ArrayBuffer(1))
        ],
        received,
        ended,
        composedOutput,
        singleCompositionIsIdentity,
        destruction: {
            returnedUndefined: destructionResult === undefined,
            destroyed: target.destroyed,
            error: destructionError,
            errored: target.errored && {
                name: target.errored.name,
                code: target.errored.code,
                message: target.errored.message
            }
        }
    }));
});
