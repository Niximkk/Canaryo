const net = require("node:net");

const list = new net.BlockList();
list.addAddress("10.0.0.1");
list.addRange("10.0.0.10", "10.0.0.20");
list.addSubnet("192.168.0.0", 24);
list.addAddress("2001:db8::1", "ipv6");

const address = new net.SocketAddress({ address: "127.0.0.1", port: 8080 });
const parsed = net.SocketAddress.parse("[2001:db8::1]:443");

const originalAutoSelectFamily = net.getDefaultAutoSelectFamily();
const originalAttemptTimeout = net.getDefaultAutoSelectFamilyAttemptTimeout();
net.setDefaultAutoSelectFamily(false);
net.setDefaultAutoSelectFamilyAttemptTimeout(25);
const autoSelection = [
    net.getDefaultAutoSelectFamily(),
    net.getDefaultAutoSelectFamilyAttemptTimeout()
];
net.setDefaultAutoSelectFamily(originalAutoSelectFamily);
net.setDefaultAutoSelectFamilyAttemptTimeout(originalAttemptTimeout);

console.log(JSON.stringify({
    rules: list.rules,
    checks: [
        list.check("10.0.0.1"),
        list.check("10.0.0.15"),
        list.check("10.0.0.21"),
        list.check("192.168.0.99"),
        list.check("192.168.1.1"),
        list.check("2001:db8::1", "ipv6"),
        list.check("2001:db8::2", "ipv6")
    ],
    address: address.toJSON(),
    parsed: parsed && parsed.toJSON(),
    autoSelection
}));
