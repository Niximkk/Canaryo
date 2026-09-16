const diagnostics = require("node:diagnostics_channel");

const channel = diagnostics.channel("canaryo:test");
const messages = [];
const subscriber = (message, name) => messages.push([message.value, name]);
channel.subscribe(subscriber);
channel.publish({ value: 1 });
const removed = channel.unsubscribe(subscriber);
channel.publish({ value: 2 });
const standalone = new diagnostics.Channel("standalone");

console.log(JSON.stringify({
    instance: channel instanceof diagnostics.Channel,
    singleton: diagnostics.channel("canaryo:test") === channel,
    name: channel.name,
    messages,
    removed,
    hasSubscribers: channel.hasSubscribers,
    standalone: {
        instance: standalone instanceof diagnostics.Channel,
        name: standalone.name
    }
}));
