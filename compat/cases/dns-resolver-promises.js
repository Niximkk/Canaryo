const dns = require("node:dns");
const promises = require("node:dns/promises");

const callbackResolver = new dns.Resolver({ timeout: 50, tries: 1 });
callbackResolver.setServers(["1.1.1.1", "[::1]:5353"]);

const promiseResolver = new promises.Resolver({ timeout: 50, tries: 1 });
promiseResolver.setServers(["8.8.8.8"]);

const originalServers = dns.getServers();
const originalPromiseServers = promises.getServers();
dns.setServers(["1.0.0.1", "[::1]:5353"]);
promises.setServers(["8.8.4.4"]);
const sharedServers = [dns.getServers(), promises.getServers()];
dns.setServers(originalServers);
promises.setServers(originalPromiseServers);

console.log(JSON.stringify({
    callbackResolver: {
        servers: callbackResolver.getServers(),
        resolve: typeof callbackResolver.resolve,
        resolve4: typeof callbackResolver.resolve4,
        resolve6: typeof callbackResolver.resolve6
    },
    promiseResolver: {
        servers: promiseResolver.getServers(),
        resolve: typeof promiseResolver.resolve,
        resolve4: typeof promiseResolver.resolve4,
        resolve6: typeof promiseResolver.resolve6
    },
    sharedServers,
    promiseFunctions: [typeof promises.resolve4, typeof promises.resolve6]
}));
