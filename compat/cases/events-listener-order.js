const { EventEmitter } = require("node:events");

const emitter = new EventEmitter();
const order = [];
emitter.on("value", value => order.push(`on:${value}`));
emitter.prependListener("value", value => order.push(`prepend:${value}`));
emitter.once("value", value => order.push(`once:${value}`));
emitter.emit("value", 1);
emitter.emit("value", 2);

console.log(JSON.stringify({
    order,
    eventNames: emitter.eventNames().map(String),
    listenerCount: emitter.listenerCount("value")
}));
