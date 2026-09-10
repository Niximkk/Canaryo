const greeting = require("./lib/greeting");
const metadata = require("./lib/metadata.json");

console.log(greeting(metadata.name));
