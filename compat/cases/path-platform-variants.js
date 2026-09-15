const path = require("node:path");

console.log(JSON.stringify({
    posix: {
        normalize: path.posix.normalize("/srv/app/../data//file.json"),
        relative: path.posix.relative("/srv/app", "/srv/data/file.json"),
        parsed: path.posix.parse("/srv/data/file.json")
    },
    win32: {
        normalize: path.win32.normalize("C:\\srv\\app\\..\\data\\file.json"),
        relative: path.win32.relative("C:\\srv\\app", "C:\\srv\\data\\file.json"),
        parsed: path.win32.parse("C:\\srv\\data\\file.json")
    }
}));
