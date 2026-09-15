const events = require("node:events");
const { EventEmitter } = events;

const emitter = new EventEmitter();
const order = [];
emitter.on("value", value => order.push(`on:${value}`));
emitter.prependListener("value", value => order.push(`prepend:${value}`));
emitter.once("value", value => order.push(`once:${value}`));
emitter.emit("value", 1);
emitter.emit("value", 2);

const originalDefault = events.defaultMaxListeners;
events.defaultMaxListeners = 13;
const inheritedMaximum = new EventEmitter().getMaxListeners();
const initialized = {};
events.init.call(initialized);
events.setMaxListeners(17);
const staticMaximum = new EventEmitter().getMaxListeners();
events.defaultMaxListeners = originalDefault;

console.log(JSON.stringify({
    order,
    eventNames: emitter.eventNames().map(String),
    listenerCount: emitter.listenerCount("value"),
    defaults: {
        inheritedMaximum,
        initialized: Object.getPrototypeOf(initialized._events) === null,
        staticMaximum,
        usingDomains: events.usingDomains
    }
}));
