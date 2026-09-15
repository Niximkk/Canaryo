const fs = require("node:fs");
const path = require("node:path");

const originalDirectory = process.cwd();
const targetDirectory = path.join(originalDirectory, "compat", "cases");
process.chdir(targetDirectory);
const changedDirectory = path.basename(process.cwd());
process.chdir(originalDirectory);

const environmentPath = path.join(targetDirectory, ".canaryo-process-env");
fs.writeFileSync(environmentPath, "CANARYO_ENV_ONE=first\nCANARYO_ENV_TWO=\"second value\"\n");
delete process.env.CANARYO_ENV_ONE;
delete process.env.CANARYO_ENV_TWO;
const loadResult = process.loadEnvFile(environmentPath);
fs.unlinkSync(environmentPath);

console.log(JSON.stringify({
    changedDirectory,
    restoredDirectory: process.cwd() === originalDirectory,
    environment: [process.env.CANARYO_ENV_ONE, process.env.CANARYO_ENV_TWO],
    loadResult: loadResult === undefined ? "undefined" : String(loadResult)
}));
