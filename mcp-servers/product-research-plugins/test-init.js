const { spawn } = require('child_process');
const path = require('path');

const serverPath = path.join(__dirname, 'server.js');
const child = spawn('node', [serverPath], {
  env: { ...process.env, BRIGHTDATA_API_KEY: '7505c9bf-f1b7-4c37-9df1-c4265421eba1' }
});

let buf = '';
child.stdout.on('data', (d) => {
  buf += d.toString();
  const lines = buf.split('\n');
  buf = lines.pop();
  lines.forEach(l => {
    if (!l.trim()) return;
    try {
      const msg = JSON.parse(l.trim());
      if (msg.result && msg.result.tools) {
        console.log('\n=== exposed tools ===');
        msg.result.tools.forEach(t => console.log(' -', t.name));
        console.log('=====================\n');
        child.kill();
      } else if (msg.result && msg.result.protocolVersion) {
        console.log('[init ok] protocol:', msg.result.protocolVersion);
        child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }) + '\n');
      }
    } catch (e) {
      if (!l.includes('Starting server') && !l.includes('wrapper')) {
        console.log('[raw]', l.trim());
      }
    }
  });
});

child.stderr.on('data', (d) => {
  const s = d.toString();
  if (!s.includes('FastMCP warning')) console.error('[stderr]', s.trim());
});

setTimeout(() => {
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'test', version: '1.0' } } }) + '\n');
}, 500);

setTimeout(() => child.kill(), 10000);
