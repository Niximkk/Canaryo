const timers = require("node:timers");

const item = {
    fired: 0,
    _onTimeout() { this.fired++; }
};

timers.enroll(item, 5);
const enrolled = {
    idleTimeout: item._idleTimeout,
    linked: "_idleNext" in item && "_idlePrev" in item
};
timers.active(item);

setTimeout(() => {
    const fired = item.fired;
    timers.unenroll(item);
    console.log(JSON.stringify({
        functions: ["active", "_unrefActive", "enroll", "unenroll"].map(name => typeof timers[name]),
        enrolled,
        fired,
        idleTimeoutAfterUnenroll: item._idleTimeout
    }));
}, 25);
