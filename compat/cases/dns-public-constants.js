const dns = require("node:dns");
const promises = require("node:dns/promises");

const names = [
    "ADDRCONFIG", "ALL", "V4MAPPED", "NODATA", "FORMERR", "SERVFAIL",
    "NOTFOUND", "NOTIMP", "REFUSED", "BADQUERY", "BADNAME", "BADFAMILY",
    "BADRESP", "CONNREFUSED", "TIMEOUT", "EOF", "FILE", "NOMEM",
    "DESTRUCTION", "BADSTR", "BADFLAGS", "NONAME", "BADHINTS",
    "NOTINITIALIZED", "LOADIPHLPAPI", "ADDRGETNETWORKPARAMS", "CANCELLED"
];

console.log(JSON.stringify({
    constants: Object.fromEntries(names.map(name => [name, dns[name]])),
    promisesMatch: names.every(name => promises[name] === dns[name]),
    resultOrderMethods: [
        typeof promises.getDefaultResultOrder,
        typeof promises.setDefaultResultOrder
    ]
}));
