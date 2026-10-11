import assert from 'node:assert/strict';
import { test } from 'node:test';
import { inspectRelease, publishRelease } from './publish.mjs';

const target = { repository: 'owner/repo', tag: 'v0.6.4', installer: 'setup.exe', otherAssets: ['latest.json'], notes: 'notes.md' };
const ok = { status: 0, stdout: '', stderr: '' };
const missing = { status: 1, stderr: 'gh: Not Found (HTTP 404)' };
const release = draft => ({ ...ok, stdout: JSON.stringify({ tag_name: target.tag, draft }) });

test('rejects a public release without making any writes', () => {
  const commands = [];
  assert.throws(() => publishRelease({ ...target, run: args => { commands.push(args); return release(false); } }), /already public/);
  assert.equal(commands.length, 1);
  assert.equal(commands[0][0], 'api');
});

test('network and authorization errors cannot be treated as a missing release', () => {
  for (const stderr of ['connection reset', 'gh: Bad credentials (HTTP 401)', 'gh: Forbidden (HTTP 403)']) {
    assert.throws(() => inspectRelease({ ...target, run: () => ({ status: 1, stderr }) }), /publication stopped/);
  }
});

test('rejects malformed or mismatched release state', () => {
  for (const value of [{ tag_name: 'v9.0.0', draft: true }, { tag_name: target.tag }]) {
    assert.throws(() => inspectRelease({ ...target, run: () => ({ ...ok, stdout: JSON.stringify(value) }) }), /unexpected release/);
  }
});

test('creates a draft and uploads installer before the remaining assets', () => {
  let exists = false;
  const writes = [];
  publishRelease({ ...target, run: args => {
    if (args[0] === 'api') return exists ? release(true) : missing;
    writes.push(args);
    if (args[1] === 'create') exists = true;
    return ok;
  } });
  assert.deepEqual(writes.map(args => args[1]), ['create', 'upload', 'upload', 'edit']);
  assert.ok(writes[0].includes('--draft'));
  assert.ok(writes[1].includes(target.installer));
  assert.ok(writes[2].includes('latest.json'));
  assert.ok(writes[3].includes('--draft=false'));
});

test('resumes a partial draft without creating another release', () => {
  const writes = [];
  publishRelease({ ...target, run: args => {
    if (args[0] === 'api') return release(true);
    writes.push(args);
    return ok;
  } });
  assert.deepEqual(writes.map(args => args[1]), ['upload', 'upload', 'edit']);
});

test('stops if an external actor publishes the draft during upload', () => {
  let reads = 0;
  const writes = [];
  assert.throws(() => publishRelease({ ...target, run: args => {
    if (args[0] === 'api') return release(++reads < 3);
    writes.push(args);
    return ok;
  } }), /already public/);
  assert.deepEqual(writes.map(args => args[1]), ['upload']);
});

test('an upload failure leaves the draft unpublished', () => {
  const writes = [];
  assert.throws(() => publishRelease({ ...target, run: args => {
    if (args[0] === 'api') return release(true);
    writes.push(args);
    return { status: 1, stderr: 'upload interrupted' };
  } }), /upload interrupted/);
  assert.equal(writes.length, 1);
});
