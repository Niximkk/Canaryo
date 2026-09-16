const {
    performance,
    Performance,
    PerformanceResourceTiming
} = require("node:perf_hooks");

function constructorError(Constructor) {
    try {
        new Constructor();
        return null;
    } catch (error) {
        return { name: error.name, code: error.code, message: error.message };
    }
}

console.log(JSON.stringify({
    performanceInstance: performance instanceof Performance,
    performanceConstructor: constructorError(Performance),
    resourceConstructor: constructorError(PerformanceResourceTiming),
    methods: [
        typeof performance.now,
        typeof performance.mark,
        typeof performance.measure,
        typeof performance.toJSON
    ],
    jsonKeys: Object.keys(performance.toJSON()).sort()
}));
