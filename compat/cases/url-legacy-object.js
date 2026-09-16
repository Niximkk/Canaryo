const url = require("node:url");

const parsed = url.parse("https://user:pass@example.com:8080/a?x=1#h", true);
const resolved = url.resolveObject("https://example.com/x/y", "../z?q=1");
const empty = new url.Url();

function select(value) {
    return {
        protocol: value.protocol,
        slashes: value.slashes,
        auth: value.auth,
        host: value.host,
        port: value.port,
        hostname: value.hostname,
        hash: value.hash,
        search: value.search,
        query: value.query,
        pathname: value.pathname,
        path: value.path,
        href: value.href
    };
}

console.log(JSON.stringify({
    parsedInstance: parsed instanceof url.Url,
    parsed: select(parsed),
    resolvedInstance: resolved instanceof url.Url,
    resolved: select(resolved),
    empty: select(empty),
    methods: [typeof empty.parse, typeof empty.format, typeof empty.resolve, typeof empty.resolveObject]
}));
