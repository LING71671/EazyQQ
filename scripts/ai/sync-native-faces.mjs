import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
// The classic labels are frozen in a local snapshot so the generator never depends on the
// version-specific NapCat resource directory, which moves on every application release.
export const SOURCE_PATH = 'src-tauri/resources/protocol/0.6.5/static/assets/qface-BZLvZXJ8.js';
export const SOURCE_SHA256 = '9fee278a09655cbe79ef2e45c83487dc123b1721f1e0025fe5809edf17d80044';
export const CLASSIC_SNAPSHOT_PATH = 'scripts/ai/data/qface-classic-BZLvZXJ8.json';
export const CLASSIC_SNAPSHOT_SHA256 = '2f02df687a8fda663ba6ab9412d2a8b498a7d8c3f272d2f26c1273d83cf75ef8';
export const INDEX_COMMIT = 'c96fa14ff894147ac0442dda0fce6a4523e5fcbd';
export const INDEX_URL = `https://raw.githubusercontent.com/koishijs/QFace/${INDEX_COMMIT}/public/assets/qq_emoji/_index.v2.json`;
export const INDEX_SHA256 = 'e69d6a0f0974b3086bc351e8b0100f1e4187822c6149936848031313a587b60d';
export const SNAPSHOT_PATH = 'scripts/ai/data/qface-native-faces.c96fa14.json';
export const SNAPSHOT_SHA256 = '1e5ed7ec7cdde7f3623aed1b5b34f84b62184a28e175e710017fad9added9d00';
const ASSET_ROOT = `https://raw.githubusercontent.com/koishijs/QFace/${INDEX_COMMIT}/public/`;
const TARGET_PATH = 'src/data/qq-faces.json';
const MAX_SOURCE_BYTES = 1024 * 1024;

// Decode a data-only JavaScript string without executing the bundled module.
function stringLiteral(source, start) {
  const quote = source[start];
  const escapes = { "'": "'", '"': '"', '\\': '\\', '/': '/', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t' };
  let value = '';
  for (let index = start + 1; index < source.length; index++) {
    const character = source[index];
    if (character === quote) return { value, end: index + 1 };
    if (character !== '\\') { value += character; continue; }
    const escape = source[++index];
    if (Object.hasOwn(escapes, escape)) value += escapes[escape];
    else if (escape === 'u' || escape === 'x') {
      const length = escape === 'u' ? 4 : 2;
      const hex = source.slice(index + 1, index + 1 + length);
      if (hex.length !== length || !/^[a-f\d]+$/i.test(hex)) throw new Error('Invalid escaped catalog string');
      value += String.fromCharCode(Number.parseInt(hex, 16));
      index += length;
    } else throw new Error('Unsupported escape in catalog string');
  }
  throw new Error('Truncated catalog string');
}

export function parseNativeFaces(source, { withPreviews = false } = {}) {
  if (Buffer.byteLength(source) > MAX_SOURCE_BYTES) throw new Error('Catalog source exceeds its size limit');
  const mapping = new Map();
  const calls = /\bJSON\.parse\(\s*(['"])/g;
  for (let match; (match = calls.exec(source));) {
    const literal = stringLiteral(source, calls.lastIndex - 1);
    if (!/^\s*\)/.test(source.slice(literal.end))) throw new Error('Catalog JSON.parse must use one literal argument');
    const records = JSON.parse(literal.value);
    calls.lastIndex = literal.end;
    if (!Array.isArray(records) || !records.some(record => record && typeof record === 'object' && ('QSid' in record || 'QDes' in record))) continue;
    for (const record of records) {
      if (!record || typeof record.QSid !== 'string' || !/^(?:0|[1-9]\d*)$/.test(record.QSid)
        || BigInt(record.QSid) > 0xffffffffn || typeof record.QDes !== 'string') throw new Error('Invalid QSid/QDes record');
      const id = record.QSid;
      const name = record.QDes.startsWith('/') ? record.QDes.slice(1) : record.QDes;
      if (!name || /[\u0000-\u001f\u007f]/.test(name)) throw new Error(`Invalid catalog label for ${id}`);
      if (mapping.has(id) && mapping.get(id).name !== name) throw new Error(`Conflicting catalog label for ${id}`);
      const previewUrl = `https://koishi.js.org/QFace/${record.isStatic ? 'static' : 'gif'}/s${id}.${record.isStatic ? 'png' : 'gif'}`;
      mapping.set(id, { id, name, ...(withPreviews ? { previewUrl } : {}) });
    }
  }
  if (!mapping.size) throw new Error('No QSid/QDes catalog was found');
  return [...mapping.values()].sort((left, right) => Number(left.id) - Number(right.id));
}

export function pinnedClassicFaces(bytes) {
  const actual = crypto.createHash('sha256').update(bytes).digest('hex');
  if (actual !== CLASSIC_SNAPSHOT_SHA256) throw new Error(`Pinned classic face snapshot SHA256 mismatch: ${actual}`);
  const snapshot = JSON.parse(bytes.toString('utf8'));
  if (snapshot.schemaVersion !== 1 || snapshot.upstream?.source !== SOURCE_PATH || snapshot.upstream?.sha256 !== SOURCE_SHA256 || !Array.isArray(snapshot.faces)) {
    throw new Error('Invalid pinned classic face snapshot');
  }
  for (const face of snapshot.faces) {
    if (!face || !/^(?:0|[1-9]\d*)$/.test(face.id) || typeof face.name !== 'string' || !face.name
      || typeof face.previewUrl !== 'string' || !new RegExp(`^https://koishi\\.js\\.org/QFace/(?:static|gif)/s${face.id}\\.(?:png|gif)$`).test(face.previewUrl)) {
      throw new Error(`Invalid classic face snapshot record: ${JSON.stringify(face)}`);
    }
  }
  return snapshot.faces.map(face => ({ id: face.id, name: face.name, previewUrl: face.previewUrl }));
}

export function pinnedPublicSnapshot(bytes) {
  if (crypto.createHash('sha256').update(bytes).digest('hex') !== SNAPSHOT_SHA256) throw new Error('Pinned local public face snapshot SHA256 mismatch');
  return JSON.parse(bytes.toString('utf8'));
}

export function publicIndexSnapshot(bytes, verifyHash = true) {
  if (bytes.length > MAX_SOURCE_BYTES) throw new Error('Public face index exceeds its size limit');
  if (verifyHash && crypto.createHash('sha256').update(bytes).digest('hex') !== INDEX_SHA256) throw new Error('Pinned public face index SHA256 mismatch');
  const index = JSON.parse(bytes.toString('utf8'));
  if (!index.emojis || typeof index.emojis !== 'object' || Array.isArray(index.emojis)) throw new Error('Invalid public face index');
  const faces = [];
  for (const [id, entry] of Object.entries(index.emojis)) {
    if (!/^(?:0|[1-9]\d*)$/.test(id) || BigInt(id) > 0xffffffffn || entry.emojiId !== id || entry.qcid !== 0 || ![0, 1].includes(entry.emojiType)) continue;
    if (typeof entry.describe !== 'string' || !entry.describe.startsWith('/') || !entry.describe.slice(1) || /[\p{Extended_Pictographic}\u0000-\u001f\u007f]/u.test(entry.describe)) throw new Error(`Invalid native face name: ${id}`);
    const official = entry.downloadInfo?.baseResDownloadUrl;
    const officialBaseUrl = typeof official === 'string' && new RegExp(`^https://wa\\.qq\\.com/qgif-web-permanent/sysemoji/v1/singleres/${id}_base_\\d+\\.zip$`).test(official) ? official : undefined;
    const assets = Array.isArray(entry.assets) ? entry.assets : [];
    const asset = ['apng', 'gif', 'png'].map(type => assets.find(asset => asset.type === type && asset.path === `assets/qq_emoji/${id}/${type}/${id}.${type === 'gif' ? 'gif' : 'png'}`)).find(Boolean);
    faces.push({ id, name: entry.describe.slice(1), emojiType: entry.emojiType, ...(officialBaseUrl ? { officialBaseUrl } : {}), ...(officialBaseUrl && asset ? { previewAsset: asset.path } : {}) });
  }
  faces.sort((left, right) => Number(left.id) - Number(right.id));
  return { schemaVersion: 1, upstream: { commit: INDEX_COMMIT, url: INDEX_URL, sha256: INDEX_SHA256, syncedAt: index.syncedAt }, selection: 'Numeric QQ system face IDs; qcid=0; emojiType=0/1. New IDs require matching official sysemoji ZIP metadata.', faces };
}

export function mergeNativeFaces(classic, snapshot) {
  if (snapshot.schemaVersion !== 1 || snapshot.upstream?.commit !== INDEX_COMMIT || snapshot.upstream?.sha256 !== INDEX_SHA256 || !Array.isArray(snapshot.faces)) throw new Error('Invalid pinned public face snapshot');
  const mapping = new Map(classic.map(face => [face.id, face]));
  for (const face of snapshot.faces) {
    if (!/^(?:0|[1-9]\d*)$/.test(face.id) || typeof face.name !== 'string' || !face.name || ![0, 1].includes(face.emojiType)) throw new Error('Invalid public native face record');
    const official = typeof face.officialBaseUrl === 'string' && new RegExp(`^https://wa\\.qq\\.com/qgif-web-permanent/sysemoji/v1/singleres/${face.id}_base_\\d+\\.zip$`).test(face.officialBaseUrl);
    if (!mapping.has(face.id) && !official) continue;
    const preview = official && typeof face.previewAsset === 'string' && new RegExp(`^assets/qq_emoji/${face.id}/(?:apng|gif|png)/${face.id}\\.(?:png|gif)$`).test(face.previewAsset) ? ASSET_ROOT + face.previewAsset : undefined;
    const previewUrl = preview || mapping.get(face.id)?.previewUrl;
    mapping.set(face.id, { id: face.id, name: face.name, ...(previewUrl ? { previewUrl } : {}) });
  }
  return [...mapping.values()].sort((left, right) => Number(left.id) - Number(right.id));
}

async function main() {
  const arguments_ = process.argv.slice(2);
  if (arguments_.some(argument => !['--check', '--refresh-upstream'].includes(argument))) throw new Error('Usage: node scripts/ai/sync-native-faces.mjs [--check] [--refresh-upstream]');
  if (arguments_.includes('--refresh-upstream')) {
    const response = await fetch(INDEX_URL, { signal: AbortSignal.timeout(15000) });
    if (!response.ok) throw new Error(`Cannot fetch pinned public index: ${response.status}`);
    const snapshot = publicIndexSnapshot(Buffer.from(await response.arrayBuffer()));
    fs.mkdirSync(path.dirname(path.join(ROOT, SNAPSHOT_PATH)), { recursive: true });
    fs.writeFileSync(path.join(ROOT, SNAPSHOT_PATH), JSON.stringify(snapshot, null, 2) + '\n');
  }
  const faces = mergeNativeFaces(pinnedClassicFaces(fs.readFileSync(path.join(ROOT, CLASSIC_SNAPSHOT_PATH))), pinnedPublicSnapshot(fs.readFileSync(path.join(ROOT, SNAPSHOT_PATH))));
  const content = JSON.stringify(faces, null, 2) + '\n';
  const destination = path.join(ROOT, TARGET_PATH);
  if (arguments_.includes('--check')) {
    if (fs.readFileSync(destination, 'utf8') !== content) throw new Error('QQ face catalog differs from the pinned sources; run the generator');
  } else fs.writeFileSync(destination, content, 'utf8');
  console.log(JSON.stringify({ count: faces.length, source: CLASSIC_SNAPSHOT_PATH, sha256: CLASSIC_SNAPSHOT_SHA256, publicIndexSha256: INDEX_SHA256, checked: arguments_.includes('--check') }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
