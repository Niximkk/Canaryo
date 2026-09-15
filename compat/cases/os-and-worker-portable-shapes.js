const os = require("node:os");
const workers = require("node:worker_threads");

console.log(JSON.stringify({
    devNull: os.devNull,
    loadAverageShape: os.loadavg().length === 3 && os.loadavg().every(Number.isFinite),
    internalThread: workers.isInternalThread
}));
