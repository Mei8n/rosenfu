import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {controlPath, startServer, stopServer} from './debug-dev-server.mjs';

async function fixture(t) {
  const dir = await mkdtemp(join(tmpdir(), 'rosenfu-dev-test-'));
  t.after(() => rm(dir, {recursive: true, force: true}));
  return controlPath(dir);
}
test('workspace control paths are distinct and stable', () => {
  assert.equal(controlPath('/a'), controlPath('/a'));
  assert.notEqual(controlPath('/a'), controlPath('/b'));
});
test('stop is harmless without a running server', async t => {
  await stopServer(await fixture(t));
});
test('stop waits for Vite cleanup; restart and repeated stop work', async t => {
  const path = await fixture(t);
  let closed = false;
  const close = await startServer(path, async () => ({
    async listen() {},
    async close() { await new Promise(r => setTimeout(r, 40)); closed = true; },
  }));
  t.after(close);
  await stopServer(path);
  assert.equal(closed, true);
  await close();
  const closeAgain = await startServer(path, async () => ({async listen() {}, async close() {}}));
  t.after(closeAgain);
  await stopServer(path);
  await closeAgain();
  await stopServer(path);
});
test('startup failure releases ownership', async t => {
  const path = await fixture(t);
  await assert.rejects(startServer(path, async () => {throw new Error('port occupied');}), /port occupied/);
  const close = await startServer(path, async () => ({async listen() {}, async close() {}}));
  await close();
});
test('duplicate start fails without stopping existing server', async t => {
  const path = await fixture(t);
  let closed = false;
  const close = await startServer(path, async () => ({async listen() {}, async close() {closed = true;}}));
  t.after(close);
  await assert.rejects(startServer(path, async () => assert.fail('must not start Vite')),
    error => error.code === 'EADDRINUSE');
  assert.equal(closed, false);
  await stopServer(path);
  await close();
});
test('stop during startup waits, then closes the newly created server', async t => {
  const path = await fixture(t);
  let release;
  const starting = new Promise(resolve => {release = resolve;});
  let closed = false;
  const startup = startServer(path, async () => {
    await starting;
    return {async listen() {}, async close() {closed = true;}};
  });
  // Wait until the control listener has been registered before requesting stop.
  await new Promise(resolve => setTimeout(resolve, 50));
  const stopping = stopServer(path);
  release();
  const close = await startup;
  await stopping;
  await close();
  assert.equal(closed, true);
});
test('an interrupted control client does not prevent later shutdown', async t => {
  const {createConnection} = await import('node:net');
  const path = await fixture(t);
  const close = await startServer(path, async () => ({async listen() {}, async close() {}}));
  t.after(close);
  await new Promise(resolve => {
    const socket = createConnection(path);
    socket.on('connect', () => {socket.write('st'); socket.destroy(); resolve();});
  });
  await stopServer(path);
  await close();
});
