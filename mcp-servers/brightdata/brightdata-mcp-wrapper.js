#!/usr/bin/env node
/**
 * brightdata mcp server wrapper
 * ensures api_token is always present before starting the server
 */
const { spawn } = require('child_process');
const path = require('path');

const apiToken = process.env.API_TOKEN || process.env.BRIGHTDATA_API_KEY;
if (!apiToken) {
  console.error('[brightdata-mcp] error: api_token or brightdata_api_key environment variable is required');
  process.exit(1);
}

// use the locally installed package
const serverJs = path.join(__dirname, 'node_modules', '@brightdata', 'mcp', 'server.js');

const child = spawn('node', [serverJs], {
  stdio: ['pipe', 'pipe', 'pipe'],
  env: {
    ...process.env,
    api_token: apiToken,
    web_unlocker_zone: process.env.web_unlocker_zone || 'mcp_unlocker',
    browser_zone: process.env.browser_zone || 'mcp_browser'
  }
});

child.stdout.on('data', (d) => process.stdout.write(d));
child.stderr.on('data', (d) => process.stderr.write(d));
child.on('exit', (code) => process.exit(code || 0));

process.stdin.on('data', (d) => child.stdin.write(d));
process.stdin.on('end', () => child.stdin.end());
