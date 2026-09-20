const asyncHooks = require("node:async_hooks");
const util = require("node:util");

const errorNumber = process.platform === "win32" ? -4058 : -2;
const systemError = util._errnoException(errorNumber, "open", "fixture.txt");
const hostErrorNumber = process.platform === "win32" ? -4078 : -111;
const hostError = util._exceptionWithHostPort(hostErrorNumber, "connect", "localhost", 8080);

const controller = util.transferableAbortController();
const sameSignal = util.transferableAbortSignal(controller.signal) === controller.signal;
const providerNames = ["NONE", "PROMISE", "TCPWRAP", "VERIFYREQUEST"];
const providerValues = providerNames.map(name => asyncHooks.asyncWrapProviders[name]);

util.aborted(controller.signal, {}).then(() => {
    console.log(JSON.stringify({
        providers: {
            nullPrototype: Object.getPrototypeOf(asyncHooks.asyncWrapProviders) === null,
            namesPresent: providerNames.every(name => Object.hasOwn(asyncHooks.asyncWrapProviders, name)),
            integerValues: providerValues.every(Number.isInteger),
            uniqueValues: new Set(providerValues).size === providerValues.length
        },
        systemError: {
            name: util.getSystemErrorName(errorNumber),
            message: util.getSystemErrorMessage(errorNumber),
            map: util.getSystemErrorMap().get(errorNumber),
            exception: {
                message: systemError.message,
                code: systemError.code,
                errno: systemError.errno,
                syscall: systemError.syscall
            },
            hostException: {
                message: hostError.message,
                code: hostError.code,
                errno: hostError.errno,
                syscall: hostError.syscall,
                address: hostError.address,
                port: hostError.port
            }
        },
        styles: [
            util.styleText("red", "x", { validateStream: false }),
            util.styleText(["bold", "blue"], "x", { validateStream: false })
        ],
        abort: {
            controller: controller instanceof AbortController,
            sameSignal,
            aborted: controller.signal.aborted,
            reason: controller.signal.reason
        }
    }));
});

controller.abort("done");
