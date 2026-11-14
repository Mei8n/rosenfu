// A workspace-scoped Vite server owned by the IDE debug task. No process-name killing.
import {unlink} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createConnection, createServer} from 'node:net';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export function controlPath(root) {
  const id = createHash('sha256').update(resolve(root)).digest('hex').slice(0, 24);
  return process.platform === 'win32'
    ? `\\\\.\\pipe\\rosenfu-debug-${id}`
    : join(tmpdir(), `rosenfu-debug-${id}.sock`);
}

export async function stopServer(path) {
  await new Promise((resolve, reject) => {
    const socket = createConnection(path);
    socket.setTimeout(5000, () => socket.destroy(new Error('Debug server did not stop')));
    socket.on('connect', () => socket.end('stop\n'));
    socket.on('data', () => {});
    socket.on('end', resolve);
    socket.on('error', async error => {
      if (error.code === 'ECONNREFUSED' && process.platform !== 'win32') {
        // A killed IDE can leave its Unix socket behind. Never unlink a live endpoint.
        try { await unlink(path); } catch (cleanup) { if (cleanup.code !== 'ENOENT') return reject(cleanup); }
        resolve();
      } else if (error.code === 'ENOENT' || error.code === 'ECONNREFUSED') resolve();
      else reject(error);
    });
  });
}

export async function startServer(path, createVite) {
  let vite;
  let closing;
  let startup;
  const sockets = new Set();
  const control = createServer({allowHalfOpen: true}, socket => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    socket.on('error', () => socket.destroy());
    socket.setTimeout(5000, () => socket.destroy());
    let command = '';
    socket.on('data', data => {
      command += data.toString();
      if (command === 'stop\n') void close().catch(error => console.error(error));
      else if (command.length > 16) socket.destroy();
    });
  });
  const close = () => closing ??= (async () => {
    await startup?.catch(() => {});
    try {
      await vite?.close();
    } finally {
      const stopped = new Promise(resolve => control.close(resolve));
      for (const socket of sockets) socket.end();
      await stopped;
    }
  })();
  await new Promise((resolve, reject) => {
    control.once('error', reject);
    control.listen(path, resolve);
  });
  startup = (async () => {
    vite = await createVite();
    await vite.listen();
  })();
  try {
    await startup;
  } catch (error) {
    await close();
    throw error;
  }
  return close;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const path = controlPath(root);
  try {
    if (process.argv[2] === 'stop') {
      await stopServer(path);
    } else if (process.argv[2] === 'start') {
      console.log('ROSENFU_DEV_STARTING');
      const {createServer: createVite} = await import('vite');
      const close = await startServer(path, () => createVite({
        root, server: {host: '127.0.0.1', port: 1420, strictPort: true},
      }));
      for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => void close());
      console.log('ROSENFU_DEV_READY');
    } else {
      throw new Error('Usage: node scripts/debug-dev-server.mjs start|stop');
    }
  } catch (error) {
    console.error('Rosenfu debug server:', error.message);
    process.exitCode = 1;
  }
}
