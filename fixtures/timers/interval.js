let calls = 0;
const interval = setInterval(() => {
    calls += 1;
    if (calls === 3) {
        clearInterval(interval);
        process.stdout.write(String(calls));
    }
}, 5);
