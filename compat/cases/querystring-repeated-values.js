const querystring = require("node:querystring");

const parsed = querystring.parse("name=canaryo&tag=rust&tag=http&empty=&encoded=a%20b");
console.log(JSON.stringify({
    parsed,
    serialized: querystring.stringify(parsed),
    custom: querystring.parse("one:1;two:2", ";", ":")
}));
