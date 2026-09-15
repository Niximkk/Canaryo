const fs = require("node:fs");
const path = require("node:path");

const missing = path.join(process.cwd(), "compat", "cases", ".definitely-missing-canaryo-file");

function capture(operation) {
    try {
        operation();
        return null;
    } catch (error) {
        return {
            name: error.name,
            code: error.code,
            syscall: error.syscall,
            path: error.path
        };
    }
}

function callbackError() {
    return new Promise(resolve => {
        fs.readFile(missing, error => resolve({
            name: error.name,
            code: error.code,
            syscall: error.syscall,
            path: error.path
        }));
    });
}

(async () => {
    console.log(JSON.stringify({
        read: capture(() => fs.readFileSync(missing)),
        open: capture(() => fs.openSync(missing, "r")),
        stat: capture(() => fs.statSync(missing)),
        lstat: capture(() => fs.lstatSync(missing)),
        access: capture(() => fs.accessSync(missing)),
        unlink: capture(() => fs.unlinkSync(missing)),
        callback: await callbackError(),
        promise: await fs.promises.readFile(missing).then(
            () => null,
            error => ({ name: error.name, code: error.code, syscall: error.syscall, path: error.path })
        )
    }));
})();
