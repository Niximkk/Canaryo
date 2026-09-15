const fs = require("node:fs");

console.log(JSON.stringify({
    access: [fs.F_OK, fs.R_OK, fs.W_OK, fs.X_OK],
    constants: [fs.constants.F_OK, fs.constants.R_OK, fs.constants.W_OK, fs.constants.X_OK],
    streamAliases: [
        fs.FileReadStream === fs.ReadStream,
        fs.FileWriteStream === fs.WriteStream
    ]
}));
