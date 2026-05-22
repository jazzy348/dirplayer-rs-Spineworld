/**
 * WebSocket-to-TCP Proxy for Director Multiuser Xtra
 * Runs WebSocket listeners that forward Director Multiuser traffic to TCP.
 *
 * Usage:
 *   node ws-tcp-proxy-all.cjs
 *
 * Default mappings:
 *   ws://0.0.0.0:1627 -> tcp://127.0.0.1:1626 (Whirlpool)
 *   ws://0.0.0.0:3091 -> tcp://127.0.0.1:3090 (legacy game server)
 *   ws://0.0.0.0:3081 -> tcp://127.0.0.1:3080 (legacy multiuser server)
 *
 * Override with DIRPLAYER_PROXY_TARGETS:
 *   [{"name":"Whirlpool","wsPort":1627,"tcpHost":"127.0.0.1","tcpPort":1626}]
 */

const WebSocket = require('ws');
const net = require('net');

const listenHost = process.env.DIRPLAYER_WS_HOST || '0.0.0.0';

function createProxy({ wsPort, tcpHost, tcpPort, name }) {
  const wss = new WebSocket.Server({ host: listenHost, port: wsPort });

  console.log(`[${name}] WebSocket ws://${listenHost}:${wsPort} -> TCP ${tcpHost}:${tcpPort}`);

  wss.on('connection', (ws, req) => {
    const clientIp = req.socket.remoteAddress;
    console.log(`[${name}] New WS connection from ${clientIp}`);

    const tcp = net.createConnection({ host: tcpHost, port: tcpPort }, () => {
      console.log(`[${name}] Connected to TCP ${tcpHost}:${tcpPort}`);
    });

    tcp.on('data', (data) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(data);
      }
    });

    tcp.on('close', () => {
      console.log(`[${name}] TCP closed`);
      ws.close();
    });

    tcp.on('error', (err) => {
      console.error(`[${name}] TCP error: ${err.message}`);
      ws.close();
    });

    ws.on('message', (data) => {
      if (tcp.writable) {
        tcp.write(Buffer.from(data));
      }
    });

    ws.on('close', () => {
      console.log(`[${name}] WS closed`);
      tcp.end();
    });

    ws.on('error', (err) => {
      console.error(`[${name}] WS error: ${err.message}`);
      tcp.end();
    });
  });

  wss.on('error', (err) => {
    console.error(`[${name}] Server error: ${err.message}`);
  });

  return wss;
}

function loadTargets() {
  if (!process.env.DIRPLAYER_PROXY_TARGETS) {
    return [
      { name: 'Whirlpool', wsPort: 1627, tcpHost: '127.0.0.1', tcpPort: 1626 },
      { name: 'Game', wsPort: 3091, tcpHost: '127.0.0.1', tcpPort: 3090 },
      { name: 'Multiuser', wsPort: 3081, tcpHost: '127.0.0.1', tcpPort: 3080 },
    ];
  }

  const targets = JSON.parse(process.env.DIRPLAYER_PROXY_TARGETS);
  if (!Array.isArray(targets) || targets.length === 0) {
    throw new Error('DIRPLAYER_PROXY_TARGETS must be a non-empty JSON array');
  }
  return targets.map((target) => ({
    name: String(target.name || `${target.tcpHost}:${target.tcpPort}`),
    wsPort: Number(target.wsPort),
    tcpHost: String(target.tcpHost),
    tcpPort: Number(target.tcpPort),
  }));
}

console.log('WebSocket-to-TCP Proxy for Director');
console.log('====================================\n');

for (const target of loadTargets()) {
  createProxy(target);
}

console.log('\nReady for connections!\n');
