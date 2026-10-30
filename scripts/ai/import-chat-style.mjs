import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const MANIFEST = 'window.__QCE_MANIFEST__ && window.__QCE_MANIFEST__(';
const CHUNK = 'window.__QCE_CHUNK__ && window.__QCE_CHUNK__(';
const MAX_HTML_BYTES = 256 * 1024 * 1024;
const FORBIDDEN_CLASSES = /(?:^|\s)(?:system-message(?:-container)?|recalled-message|reply-content(?:-[\w-]+)?|forward-card(?:-[\w-]+)?|json-card|native-card)(?:\s|$)/;
const GENERIC_UTTERANCE = /^(?:嗯+|哦+|啊+|好+|哈+|嘿+|呃+|额+|唉+|哎+|em+|ok+|[?？!！.。…~～]+|(?:好|行|可以|没事|不用|不知道|确实|有点|怎么|什么|真的假的|原来如此|懂了|收到|等等|再说|算了|也行|不是|对|不对|是的|我也是|不至于|笑死|离谱|绷不住)(?:了|啊|吧|呢|吗|哦|呀|啦|的|嘛|诶|欸|哈)?)[?？!！.。…~～]*$/iu;

// Read data literals without executing any exported JavaScript or HTML.
export function jsonLiteral(source, start) {
  if (!['[', '{'].includes(source[start])) throw new Error('Export data literal is missing');
  let depth = 0, quoted = false, escaped = false;
  for (let i = start; i < source.length; i++) {
    const character = source[i];
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === '"') quoted = false;
    } else if (character === '"') quoted = true;
    else if (character === '[' || character === '{') depth++;
    else if ((character === ']' || character === '}') && --depth === 0) {
      try { return { value: JSON.parse(source.slice(start, i + 1)), end: i + 1 }; }
      catch { throw new Error('Export data is not valid JSON'); }
    }
  }
  throw new Error('Export data is truncated');
}

export function parseExport(source) {
  const manifestAt = source.indexOf(MANIFEST);
  if (manifestAt < 0) throw new Error('Unsupported export: expected QCE modern chunked HTML');
  const manifest = jsonLiteral(source, source.indexOf('{', manifestAt + MANIFEST.length)).value;
  if (manifest.format !== 'qce-modern-html-chunked' || manifest.version !== 1) throw new Error('Unsupported QCE schema version');
  function* messages() {
    let cursor = 0;
    while ((cursor = source.indexOf(CHUNK, cursor)) >= 0) {
      const field = source.indexOf('messages:', cursor + CHUNK.length);
      const boundary = source.indexOf('</script>', cursor + CHUNK.length);
      if (field < 0 || (boundary >= 0 && field > boundary)) throw new Error('Export chunk messages are missing');
      const literal = jsonLiteral(source, source.indexOf('[', field + 9));
      if (!Array.isArray(literal.value)) throw new Error('Export messages must be an array');
      cursor = literal.end;
      yield* literal.value;
    }
  }
  return { manifest, messages };
}

function classes(html) { return [...html.matchAll(/class="([^"]*)"/g)].map(match => match[1]); }
function faceIds(html) {
  return [...html.matchAll(/<img\b[^>]*class="[^"]*\bface-emoji\b[^>]*>/g)]
    .map(match => match[0].match(/src="[^"<>]*\/(?:apng|gif|png)\/(\d+)\.(?:png|gif|webp)(?:\?[^"<>]*)?"/i)?.[1]).filter(Boolean);
}
function ownerUid(manifest, uin) {
  // UIN-to-UID mapping is explicit exporter metadata, never display name or isSelf.
  if (String(manifest.chat?.selfUin) === uin && typeof manifest.chat.selfUid === 'string') return manifest.chat.selfUid;
  const mapped = (manifest.senders || []).filter(sender => String(sender.uin ?? sender.senderUin ?? '') === uin);
  if (mapped.length === 1 && typeof mapped[0].uid === 'string') return mapped[0].uid;
  return null;
}
export function eligibleMessage(message) {
  const html = String(message.html || '');
  if (classes(html).some(value => FORBIDDEN_CLASSES.test(value))) return null;
  if (message.isSystem || message.forwarded || message.quoted || message.aiGenerated || (message.ai_reply_status && message.ai_reply_status !== 'none')) return null;
  if (message.textTruncated) return null;
  const text = String(message.text || '').trim();
  if (/^(?:作为(?:一个)?(?:AI|人工智能)|我是(?:一个)?(?:AI|人工智能)|系统提示|system\s*:)/iu.test(text)) return null;
  if (text.length > 2000 || /https?:\/\/|(?:^|\s)[\w.+-]+@[\w.-]+\.[a-z]{2,}/iu.test(text)) return null;
  const faces = faceIds(html);
  const plain = text.replace(/\[CQ:[^\]]*\]/g, '').replace(/\[(?:图片|表情|文件|语音|视频|动画表情|合并转发)[^\]]*\]/g, '').trim();
  if (!plain && faces.length === 0) return null;
  if (/^\[.*\]$/.test(plain) && /image-content|sticker-wrap|file-bubble|voice-bubble|video-bubble/.test(html)) return null;
  return { text: plain, faces };
}
function quantile(values, rate) { if (!values.length) return 0; const sorted = [...values].sort((a, b) => a - b); return sorted[Math.floor((sorted.length - 1) * rate)]; }
function rounded(value) { return Number(value.toFixed(6)); }
function summary(messages) {
  const lengths = messages.map(message => [...message.text].length), count = messages.length;
  const rate = predicate => rounded(count ? messages.filter(predicate).length / count : 0);
  return { messageCount: count, medianChars: quantile(lengths, .5), p90Chars: quantile(lengths, .9),
    questionRate: rate(message => /[?？]/u.test(message.text)), exclamationRate: rate(message => /[!！]/u.test(message.text)),
    faceRate: rate(message => message.faces.length > 0), multiMessageRate: rate(message => message.burst),
    terminalPunctuationRate: rate(message => /[。.!！?？]$/u.test(message.text)), newlineRate: rate(message => /\n/u.test(message.text)) };
}

export function analyzeExports(exports, options) {
  const uin = String(options.ownerUin);
  if (!/^[1-9]\d{4,19}$/.test(uin)) throw new Error('Owner UIN must be explicit and valid');
  const seen = new Set(), messages = [], phrases = new Map(), faces = new Map(), conversations = new Map();
  const counters = { files: 0, filesWithVerifiedOwner: 0, messagesSeen: 0, matchingSender: 0, identityConflict: 0, excluded: 0, duplicate: 0, included: 0 };
  const sourceHash = crypto.createHash('sha256');
  for (const source of exports) {
    counters.files++; sourceHash.update(source);
    const { manifest, messages: eachMessage } = parseExport(source), verifiedUid = ownerUid(manifest, uin);
    if (!verifiedUid) continue;
    counters.filesWithVerifiedOwner++;
    const conversation = crypto.createHash('sha256').update(`${manifest.chat?.type || ''}\0${manifest.chat?.peerUid || manifest.chat?.peerUin || counters.files}`).digest('hex');
    let previous = null;
    for (const message of eachMessage()) {
      counters.messagesSeen++;
      const explicitUins = [message.senderUin, message.uin, message.sender?.uin].filter(value => value !== undefined && value !== null && String(value) !== '').map(String);
      const hasUid = message.uid !== undefined && message.uid !== null && String(message.uid) !== '';
      const uidMatches = message.uid === verifiedUid;
      const explicitMatches = explicitUins.length > 0 && explicitUins.every(value => value === uin);
      const conflict = explicitUins.length > 0 && ((uidMatches && !explicitMatches) || (explicitMatches && hasUid && !uidMatches) || new Set(explicitUins).size > 1);
      if (conflict) counters.identityConflict++;
      const isOwner = !conflict && (hasUid ? uidMatches && (explicitUins.length === 0 || explicitMatches) : explicitMatches);
      const timestamp = Number(message.ts), burst = Boolean(isOwner && previous?.owner && Number.isFinite(timestamp) && timestamp >= previous.ts && timestamp - previous.ts <= 30_000);
      previous = { owner: isOwner, ts: timestamp };
      if (!isOwner) continue;
      counters.matchingSender++;
      const identity = String(message.id || '');
      if (!identity) { counters.excluded++; continue; }
      if (seen.has(identity)) { counters.duplicate++; continue; }
      seen.add(identity);
      const eligible = eligibleMessage(message);
      if (!eligible) { counters.excluded++; continue; }
      counters.included++;
      const row = { ...eligible, burst }; messages.push(row);
      const rows = conversations.get(conversation) || []; rows.push(row); conversations.set(conversation, rows);
      for (const face of eligible.faces) faces.set(face, (faces.get(face) || 0) + 1);
      if ([...eligible.text].length <= 12 && GENERIC_UTTERANCE.test(eligible.text)) {
        const entry = phrases.get(eligible.text) || { count: 0, conversations: new Set() }; entry.count++; entry.conversations.add(conversation); phrases.set(eligible.text, entry);
      }
    }
  }
  if (!messages.length) throw new Error('No original owner messages matched explicit sender identity and exclusion rules');
  const stats = summary(messages), allowedFaces = new Set(options.allowedFaceIds || []);
  const preferredPhrases = [...phrases].filter(([, entry]) => entry.count >= 5 && entry.conversations.size >= 3).sort((a, b) => b[1].count - a[1].count || a[0].localeCompare(b[0], 'zh-CN')).slice(0, 12).map(([phrase]) => phrase);
  const qqFaceIds = [...faces].filter(([id]) => allowedFaces.has(id)).sort((a, b) => b[1] - a[1] || Number(a[0]) - Number(b[0])).slice(0, 16).map(([id]) => id);
  const { terminalPunctuationRate, newlineRate, ...statistics } = stats;
  const pack = { schemaVersion: 1, id: options.id || 'local-chat-style', name: options.name || '本地聊天风格', version: '1.0.0',
    rules: { sentenceLength: stats.medianChars <= 16 ? 'short' : stats.medianChars <= 40 ? 'balanced' : 'detailed',
      punctuation: terminalPunctuationRate < .18 ? 'minimal' : stats.questionRate + stats.exclamationRate > .3 ? 'expressive' : 'standard',
      tone: 'casual', lineBreaks: stats.multiMessageRate > .5 || newlineRate > .1 ? 'frequent' : 'natural', preferredPhrases, avoidPhrases: [], qqFaceIds }, statistics };
  const ranked = [...conversations.values()].sort((a, b) => b.length - a.length), cutoff = Math.max(1, Math.ceil(ranked.length * .2));
  const audit = { schemaVersion: 1, counters, sourceDigest: sourceHash.digest('hex'), conversationCount: ranked.length,
    extraction: 'Explicit UIN-to-UID metadata mapping; exact sender matching; no nickname/isSelf attribution; excludes whole messages containing quotes/forwards/system or explicit AI provenance.',
    limitations: ['Exporter has no reliable per-message human-versus-unmarked-AI provenance; unlabeled historical AI output cannot be conclusively identified.', 'Activity cohorts describe volume, not verified relationship closeness.', 'Only conservative reusable short reactions repeated across at least three conversations become phrases; no raw message or contact is exported.'],
    punctuation: { terminalPunctuationRate, newlineRate }, activityCohorts: { higherVolume: summary(ranked.slice(0, cutoff).flat()), otherVolume: summary(ranked.slice(cutoff).flat()) },
    phraseEvidence: preferredPhrases.map(phrase => ({ phrase, count: phrases.get(phrase).count, conversations: phrases.get(phrase).conversations.size })), faceCounts: Object.fromEntries([...faces].sort((a, b) => b[1] - a[1])) };
  return { pack, audit };
}

function argumentsFrom(argv) { const values = {}; for (let i = 0; i < argv.length; i += 2) { if (!argv[i]?.startsWith('--') || !argv[i + 1]) throw new Error('Expected --input, --owner, --output and optional --id/--name/--audit/--faces'); values[argv[i].slice(2)] = argv[i + 1]; } return values; }
function* sources(input) {
  const metadata = fs.lstatSync(input); if (metadata.isSymbolicLink()) throw new Error('Input symlinks are not accepted');
  const files = metadata.isDirectory() ? fs.readdirSync(input).filter(name => /\.html$/i.test(name)).sort().map(name => path.join(input, name)) : [input];
  for (const file of files) { const info = fs.lstatSync(file); if (!info.isFile() || info.isSymbolicLink() || info.size > MAX_HTML_BYTES) throw new Error('Input must contain bounded regular HTML files'); yield fs.readFileSync(file, 'utf8'); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = argumentsFrom(process.argv.slice(2)); if (!args.input || !args.owner || !args.output) throw new Error('Usage: --input <QCE HTML folder> --owner <UIN> --output <new private JSON> [--audit <new private JSON>]');
    const facesPath = args.faces || fileURLToPath(new URL('../../src/data/qq-faces.json', import.meta.url));
    const allowedFaceIds = JSON.parse(fs.readFileSync(facesPath, 'utf8')).map(face => String(face.id));
    const result = analyzeExports(sources(args.input), { ownerUin: args.owner, id: args.id, name: args.name, allowedFaceIds });
    for (const [target, value] of [[args.output, result.pack], ...(args.audit ? [[args.audit, result.audit]] : [])]) { fs.mkdirSync(path.dirname(path.resolve(target)), { recursive: true }); fs.writeFileSync(target, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' }); }
    console.log(JSON.stringify({ schemaVersion: 1, files: result.audit.counters.files, matchingSender: result.audit.counters.matchingSender, included: result.audit.counters.included, excluded: result.audit.counters.excluded, duplicate: result.audit.counters.duplicate, conversations: result.audit.conversationCount, statistics: result.pack.statistics }));
  } catch (error) { console.error(error instanceof Error ? error.message : 'Style import failed'); process.exitCode = 1; }
}
