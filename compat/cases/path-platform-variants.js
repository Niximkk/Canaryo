const path = require("node:path");

const globCases = [
    ["src/main.js", "src/*.js"],
    ["src/lib/main.js", "src/*.js"],
    ["src/lib/main.js", "src/**/*.js"],
    ["README.md", "*.{md,txt}"],
    ["module.js", "*.?(js|ts)"],
    [".env", "*"],
    ["a/b", "a/?"]
];

console.log(JSON.stringify({
    posix: {
        normalize: path.posix.normalize("/srv/app/../data//file.json"),
        relative: path.posix.relative("/srv/app", "/srv/data/file.json"),
        parsed: path.posix.parse("/srv/data/file.json")
    },
    win32: {
        normalize: path.win32.normalize("C:\\srv\\app\\..\\data\\file.json"),
        relative: path.win32.relative("C:\\srv\\app", "C:\\srv\\data\\file.json"),
        parsed: path.win32.parse("C:\\srv\\data\\file.json"),
        namespacedDrive: path.win32.toNamespacedPath("C:\\srv\\data"),
        namespacedUnc: path.win32.toNamespacedPath("\\\\server\\share\\data")
    },
    makeLongAliases: [
        path._makeLong === path.toNamespacedPath,
        path.posix._makeLong === path.posix.toNamespacedPath,
        path.win32._makeLong === path.win32.toNamespacedPath
    ],
    globMatches: globCases.map(([value, pattern]) => path.matchesGlob(value, pattern))
}));
