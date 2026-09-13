setTimeout(() => process.stdout.write("late"), 1000).unref();
process.stdout.write("done");
