const modules = JSON.parse(process.env.CANARYO_COMPAT_MODULES || "[]");

function names(value) {
    if ((typeof value !== "object" || value === null) && typeof value !== "function") return [];
    return Reflect.ownKeys(value)
        .filter(name => typeof name === "string")
        .sort();
}

const result = {};
for (const name of modules) {
    try {
        const value = require(`node:${name}`);
        result[name] = {
            loaded: true,
            exports: names(value),
            prototype: typeof value === "function" ? names(value.prototype) : []
        };
    } catch (error) {
        result[name] = {
            loaded: false,
            error: {
                name: error && error.name || "Error",
                code: error && error.code || null
            }
        };
    }
}

console.log(JSON.stringify(result));
