import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { parseNativeFaces, pinnedNativeFaces, pinnedPublicSnapshot, publicIndexSnapshot, mergeNativeFaces, SOURCE_PATH, SNAPSHOT_PATH } from './sync-native-faces.mjs';

function fixture(records) {
  const literal = JSON.stringify(records).replace(/\\/g, '\\\\').replace(/'/g, "\\'");
  return `const data=JSON.parse('${literal}');throw new Error('The source must not execute');`;
}

test('parses only catalog data, deduplicates exact labels and sorts numeric IDs', () => {
  assert.deepEqual(parseNativeFaces(fixture([
    { QSid: '10', QDes: '/Synthetic ten' }, { QSid: '2', QDes: '/Synthetic two' },
    { QSid: '2', QDes: '/Synthetic two' }, { QSid: '0', QDes: '/Owner\'s "quoted" label' },
  ])), [{ id: '0', name: 'Owner\'s "quoted" label' }, { id: '2', name: 'Synthetic two' }, { id: '10', name: 'Synthetic ten' }]);
});

test('rejects conflicting IDs, malformed records and executable arguments', () => {
  assert.throws(() => parseNativeFaces(fixture([{ QSid: '2', QDes: '/One' }, { QSid: '2', QDes: '/Other' }])), /Conflicting/);
  assert.throws(() => parseNativeFaces(fixture([{ QSid: '02', QDes: '/Invalid ID' }])), /Invalid/);
  assert.throws(() => parseNativeFaces(fixture([{ QSid: '2' }])), /Invalid/);
  assert.throws(() => parseNativeFaces('JSON.parse(process.exit())'), /No QSid/);
  assert.throws(() => parseNativeFaces(`JSON.parse('${JSON.stringify([{ QSid: '2', QDes: '/One' }])}' + process.exit())`), /one literal/);
  assert.throws(() => pinnedNativeFaces(Buffer.from(fixture([{ QSid: '2', QDes: '/One' }]))), /SHA256 mismatch/);
});

test('generated catalog merges both pinned public sources with explicit current labels and media', () => {
  const root = new URL('../../', import.meta.url);
  const classic = pinnedNativeFaces(fs.readFileSync(new URL(SOURCE_PATH, root)));
  assert.equal(classic.length, 275);
  const faces = mergeNativeFaces(classic, pinnedPublicSnapshot(fs.readFileSync(new URL(SNAPSHOT_PATH, root))));
  assert.equal(faces.length, 407);
  assert.deepEqual(JSON.parse(fs.readFileSync(new URL('src/data/qq-faces.json', root), 'utf8')), faces);
  const mapping = new Map(faces.map(face => [face.id, face.name]));
  for (const [id, name] of [['264', '捂脸'], ['277', '汪汪'], ['317', '菜汪'], ['324', '吃糖'], ['338', '我想开了'], ['427', '偷感']]) assert.equal(mapping.get(id), name);
  assert.equal(classic.find(face => face.id === '317').name, '菜狗');
  assert.match(faces.find(face => face.id === '427').previewUrl, /\/427\/apng\/427\.png$/);
  assert.match(faces.find(face => face.id === '324').previewUrl, /\/324\/apng\/324\.png$/);
  assert.equal(faces.some(face => /\p{Extended_Pictographic}/u.test(face.name)), false);
});

test('public index excludes Unicode, interactive types and unproven new numeric IDs', () => {
  const entry = (id, extra = {}) => ({ emojiId: id, describe: '/Synthetic', qcid: 0, emojiType: 1,
    downloadInfo: { baseResDownloadUrl: `https://wa.qq.com/qgif-web-permanent/sysemoji/v1/singleres/${id}_base_123.zip` },
    assets: [{ type: 'apng', path: `assets/qq_emoji/${id}/apng/${id}.png` }], ...extra });
  const bytes = Buffer.from(JSON.stringify({ syncedAt: 'synthetic', emojis: {
    427: entry('427'), 999: entry('999', { downloadInfo: { baseResDownloadUrl: 'https://example.invalid/mface.zip' } }),
    358: entry('358', { emojiType: 2 }), 443: entry('443', { emojiType: 5 }),
    unicode: entry('unicode', { qcid: 100 }), mface: { type: 'mface', describe: '/Unsupported' },
  } }));
  const snapshot = publicIndexSnapshot(bytes, false);
  assert.deepEqual(snapshot.faces.map(face => face.id), ['427', '999']);
  assert.deepEqual(mergeNativeFaces([], snapshot).map(face => face.id), ['427']);
  assert.throws(() => publicIndexSnapshot(bytes), /SHA256/);
  assert.throws(() => pinnedPublicSnapshot(bytes), /SHA256/);
});
