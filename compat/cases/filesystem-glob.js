const fs = require("node:fs");
const path = require("node:path");

const cwd = path.join(process.cwd(), "compat", "cases");
const options = { cwd };
const direct = fs.globSync("*.js", options);
const recursive = fs.globSync("**/*metadata.js", options);
const excluded = fs.globSync("*.js", { cwd, exclude: ["*tracker.js"] });

fs.glob("*.js", options, async (error, callbackMatches) => {
    if (error) throw error;
    const promiseMatches = [];
    for await (const match of fs.promises.glob("*.js", options)) promiseMatches.push(match);

    console.log(JSON.stringify({
        direct: {
            containsSelf: direct.includes("filesystem-glob.js"),
            onlyDirect: direct.every(match => !match.includes(path.sep)),
            sorted: direct.join("|") === direct.slice().sort().join("|")
        },
        recursive,
        excludedTracker: excluded.some(match => match.endsWith("tracker.js")),
        callbackMatchesSync: callbackMatches.join("|") === direct.join("|"),
        promiseMatchesSync: promiseMatches.join("|") === direct.join("|")
    }));
});
