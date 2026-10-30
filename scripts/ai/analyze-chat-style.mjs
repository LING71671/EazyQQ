import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseExport } from './import-chat-style.mjs';

const MAX_HTML_BYTES = 256 * 1024 * 1024;
const TURN_GAP_MS = 120_000;
const BURST_GAP_MS = 30_000;
const SESSION_GAP_MS = 1_800_000;
const CATEGORIES = {
  question: /[?？]|(?:怎么|什么|为何|为什么|能不能|可不可以|多少|哪里|哪儿|是不是|有没有)/u,
  coordination: /(?:等我|等下|一会|待会|马上|明天|今天|几点|时间|发我|给我|帮我|记得|先|然后|安排|到[了哪]|出发|一起)/u,
  disagreement: /(?:不是|不对|不行|不[太是能]|没有|没必要|别|不要|不至于|但|算了)/u,
  explanation: /(?:因为|所以|就是|其实|比如|意思是|首先|然后|相当于|导致|主要|原因|由于|的话)/u,
  acknowledgment: /^(?:嗯+|哦+|好+|行|可以|收到|知道了|懂了|确实|对|是的|ok|OK|哦哦|这样)[啊吧呢呀啦哦了嘛的哈?？!！。\.]*$/u,
  support: /(?:没事|没关系|别急|不用谢|谢谢|辛苦|加油|可以的|会好的|抱抱|别担心|慢慢来)/u,
};

function digest(value) { return crypto.createHash('sha256').update(value).digest('hex'); }
function pseudonym(value) { return digest(value).slice(0, 16); }
function round(value) { return Number(value.toFixed(6)); }
function increment(map, key, count = 1) { map.set(key, (map.get(key) || 0) + count); }
function quantile(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor((sorted.length - 1) * fraction)];
}
function distribution(values) {
  return { n: values.length, mean: values.length ? round(values.reduce((a, b) => a + b, 0) / values.length) : null,
    p10: quantile(values, .1), p25: quantile(values, .25), median: quantile(values, .5), p75: quantile(values, .75), p90: quantile(values, .9), p95: quantile(values, .95) };
}
export function wilson(successes, total) {
  if (!total) return { numerator: successes, denominator: total, rate: null, ci95: null };
  const z = 1.959963984540054, rate = successes / total, scale = 1 + z * z / total;
  const center = (rate + z * z / (2 * total)) / scale;
  const radius = z * Math.sqrt(rate * (1 - rate) / total + z * z / (4 * total * total)) / scale;
  return { numerator: successes, denominator: total, rate: round(rate), ci95: [round(center - radius), round(center + radius)] };
}
function rate(rows, predicate) { return wilson(rows.filter(predicate).length, rows.length); }
function decodeEntities(value) {
  return value.replace(/&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/giu, (all, entity) => {
    if (entity[0] === '#') {
      const code = entity[1].toLowerCase() === 'x' ? parseInt(entity.slice(2), 16) : parseInt(entity.slice(1), 10);
      return code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : all;
    }
    return { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' }[entity.toLowerCase()] || all;
  });
}
function attribute(tag, name) { return tag.match(new RegExp(`\\b${name}=["']([^"']*)["']`, 'iu'))?.[1] || ''; }
function hasClass(html, pattern) {
  return [...html.matchAll(/\bclass=["']([^"']*)["']/giu)].some(match => match[1].split(/\s+/u).some(value => pattern.test(value)));
}

export function extractMessage(message) {
  const html = String(message.html || '');
  if (message.isSystem || hasClass(html, /^system-message(?:-container)?$/u)) return { exclusion: 'system' };
  if (hasClass(html, /^recalled-message$/u)) return { exclusion: 'recalled' };
  if (message.aiGenerated || (message.ai_reply_status && message.ai_reply_status !== 'none')) return { exclusion: 'explicitAi' };
  if (message.textTruncated) return { exclusion: 'truncated' };
  if (message.quoted || hasClass(html, /^reply-content(?:-[\w-]+)?$/u)) return { exclusion: 'quoted' };
  if (message.forwarded || hasClass(html, /^(?:forward-card(?:-[\w-]+)?|json-card|native-card)$/u)) return { exclusion: 'forwardOrCard' };
  const faces = [], faceLabels = []; let nativeFaceImageCount = 0;
  for (const match of html.matchAll(/<img\b[^>]*>/giu)) {
    if (!attribute(match[0], 'class').split(/\s+/u).includes('face-emoji')) continue;
    nativeFaceImageCount++;
    const id = attribute(match[0], 'src').match(/\/(?:apng|gif|png)\/(\d+)\.(?:png|gif|webp)(?:\?|$)/iu)?.[1];
    if (id) faces.push(id);
    const alt = decodeEntities(attribute(match[0], 'alt')); if (alt) faceLabels.push(alt);
  }
  const textSpans = [...html.matchAll(/<span\b[^>]*\bclass=["'][^"']*\btext-content\b[^"']*["'][^>]*>([\s\S]*?)<\/span\s*>/giu)];
  const hasMaterial = hasClass(html, /^(?:image-content|sticker-wrap|file-bubble|voice-bubble|video-bubble|message-file)$/u);
  let text = textSpans.length ? textSpans.map(match => decodeEntities(match[1].replace(/<br\s*\/?\s*>/giu, '\n').replace(/<[^>]*>/gu, ''))).join('') : '';
  if (!textSpans.length && !hasMaterial && !nativeFaceImageCount) text = String(message.text || '');
  text = text.replace(/\[CQ:face,[^\]]*\]/gu, '').replace(/\[(?:图片|表情|文件|语音|视频|动画表情|合并转发)(?::[^\]]*)?\]/gu, '').trim();
  if (text.length > 8000) return { exclusion: 'oversize' };
  return { text, faces, faceLabels, nativeFaceImageCount, material: hasMaterial, kind: text ? (nativeFaceImageCount ? 'textWithFace' : 'text') : nativeFaceImageCount ? 'faceOnly' : hasMaterial ? 'materialOnly' : 'empty' };
}

function verifiedOwner(manifest, ownerUin) {
  const candidates = new Set();
  if (String(manifest.chat?.selfUin || '') === ownerUin && typeof manifest.chat.selfUid === 'string' && manifest.chat.selfUid) candidates.add(manifest.chat.selfUid);
  for (const sender of manifest.senders || []) if (String(sender.uin ?? sender.senderUin ?? '') === ownerUin && sender.uid) candidates.add(String(sender.uid));
  return candidates.size === 1 ? [...candidates][0] : null;
}
function classifySender(message, ownerUin, ownerUid) {
  const uins = [message.senderUin, message.uin, message.sender?.uin].filter(value => value !== undefined && value !== null && String(value) !== '').map(String);
  const uids = [message.uid, message.senderUid, message.sender?.uid].filter(value => value !== undefined && value !== null && String(value) !== '').map(String);
  const uid = uids[0] || '';
  const uinOwner = uins.length > 0 && uins.every(value => value === ownerUin);
  if (new Set(uins).size > 1 || new Set(uids).size > 1 || (uid === ownerUid && uins.length && !uinOwner) || (uinOwner && uid && uid !== ownerUid)) return 'conflict';
  if (uid ? uid === ownerUid : uinOwner) return 'owner';
  return uid || uins.length ? 'peer' : 'unknown';
}
function conversationIdentity(chat) {
  const type = chat.type || 'unknown';
  if (type === 'group') return chat.groupId || chat.groupUin || chat.peerUid || chat.peerUin ? `${type}\0${chat.groupId || chat.groupUin || chat.peerUid || chat.peerUin}` : null;
  return chat.peerUid || chat.peerUin ? `${type}\0${chat.peerUid || chat.peerUin}` : null;
}
function explicitBot(chat) { return chat.isBot === true || chat.bot === true || chat.peerIsBot === true || ['bot', 'service', 'system'].includes(chat.peerType); }
function sensitiveText(value, names) {
  let text = String(value).replace(/https?:\/\/\S+/giu, '[链接]').replace(/[\w.+-]+@[\w.-]+\.[a-z]{2,}/giu, '[邮箱]')
    .replace(/\bu_[a-zA-Z0-9_-]{10,}\b/gu, '[UID]').replace(/\b\d{5,}\b/gu, '[号码]')
    .replace(/(?:[A-Za-z]:[\\/]|\\\\)[^\s，。！？]+/gu, '[本地路径]')
    .replace(/\b(?:sk-|rc_|nvapi-)[A-Za-z0-9_-]{8,}\b/gu, '[密钥]');
  for (const name of names) if (name.length >= 2 && name.length <= 40) text = text.split(name).join('[称呼]');
  return text;
}
function turnFingerprint(turn) { return digest(turn.messages.map(message => `${message.text.normalize('NFKC').trim()}\0${message.faces.join(',')}`).join('\n')); }
function messageFingerprints(turn) { return turn.messages.filter(message => message.text || message.faces.length).map(message => digest(`${message.text.normalize('NFKC').trim()}\0${message.faces.join(',')}`)); }
function isExpressive(message) { return Boolean(message.text || message.nativeFaceImageCount || message.faces.length); }

export function assembleTurns(rows, conversation) {
  const turns = []; let current = null, previous = null, session = 0, incoming = [];
  for (const message of rows) {
    if (previous && message.ts - previous.ts > SESSION_GAP_MS) { session++; current = null; incoming = []; }
    if (message.role !== 'owner' || message.exclusion || message.kind === 'empty') {
      current = null;
      if (message.role === 'peer' && !message.exclusion && message.text) incoming = [...incoming, message].slice(-2);
      else if (message.role !== 'peer' || message.exclusion) incoming = [];
      previous = message; continue;
    }
    if (!current || previous?.role !== 'owner' || message.ts - previous.ts > TURN_GAP_MS) {
      current = { id: pseudonym(`${conversation}\0${message.id}`), conversation, session, startTs: message.ts, endTs: message.ts, incoming: [...incoming], messages: [], burstGroups: [] };
      turns.push(current); incoming = [];
    }
    if (!current.messages.length || message.ts - previous.ts > BURST_GAP_MS) current.burstGroups.push(0);
    current.burstGroups[current.burstGroups.length - 1]++; current.messages.push(message); current.endTs = message.ts; previous = message;
  }
  for (const turn of turns) {
    turn.categories = Object.entries(CATEGORIES).filter(([, expression]) => expression.test(turn.messages.map(message => message.text).join('\n'))).map(([name]) => name);
    if (!turn.categories.length) turn.categories = ['other'];
    turn.fingerprint = turnFingerprint(turn);
  }
  return turns;
}

function summarize(turns) {
  const messages = turns.flatMap(turn => turn.messages), texts = messages.filter(message => message.text), expressive = messages.filter(isExpressive);
  const expressiveTurns = turns.filter(turn => turn.messages.some(isExpressive));
  const interBubbleMs = expressiveTurns.flatMap(turn => { const bubbles = turn.messages.filter(isExpressive); return bubbles.slice(1).map((message, index) => message.ts - bubbles[index].ts); });
  return {
    turns: turns.length, expressiveTurns: expressiveTurns.length, messages: messages.length, textMessages: texts.length, faceOnlyMessages: messages.filter(message => message.kind === 'faceOnly').length,
    materialOnlyMessages: messages.filter(message => message.kind === 'materialOnly').length,
    textChars: distribution(texts.map(message => [...message.text].length)), textBubbleCount: distribution(expressiveTurns.map(turn => turn.messages.filter(isExpressive).length)),
    turnChars: distribution(expressiveTurns.map(turn => turn.messages.reduce((sum, message) => sum + [...message.text].length, 0))),
    interBubbleSeconds: distribution(interBubbleMs.map(value => value / 1000)),
    rates: {
      textAtMost5: rate(texts, message => [...message.text].length <= 5), textAtMost10: rate(texts, message => [...message.text].length <= 10), textAtMost20: rate(texts, message => [...message.text].length <= 20),
      terminalPunctuation: rate(texts, message => /[。.!！?？]$/u.test(message.text)), terminalPeriod: rate(texts, message => /[。.\uFF0E]$/u.test(message.text)),
      question: rate(texts, message => /[?？]/u.test(message.text)), questionOnly: rate(texts, message => /^[?？]+$/u.test(message.text)), exclamation: rate(texts, message => /[!！]/u.test(message.text)), newline: rate(texts, message => /\n/u.test(message.text)),
      nativeFace: rate(expressive, message => (message.nativeFaceImageCount || message.faces.length) > 0), standaloneNativeFace: rate(expressive, message => message.kind === 'faceOnly'),
      multipleBubbles: rate(expressiveTurns, turn => turn.messages.filter(isExpressive).length > 1),
      shortBurst: rate(expressiveTurns, turn => { const bubbles = turn.messages.filter(isExpressive); return bubbles.slice(1).some((message, index) => message.ts - bubbles[index].ts <= BURST_GAP_MS); }),
      unicodeEmoji: rate(texts, message => /\p{Extended_Pictographic}/u.test(message.text)),
    },
  };
}
function macroRates(groups) {
  const summaries = groups.map(summarize).filter(summary => summary.expressiveTurns > 0), result = {};
  for (const key of Object.keys(summaries[0]?.rates || {})) {
    const values = summaries.map(summary => summary.rates[key].rate).filter(value => value !== null);
    result[key] = { conversations: values.length, mean: values.length ? round(values.reduce((a, b) => a + b, 0) / values.length) : null, median: quantile(values, .5), p10: quantile(values, .1), p90: quantile(values, .9) };
  }
  return result;
}
function clusteredRateIntervals(groups) {
  const summaries = groups.map(summarize).filter(summary => summary.expressiveTurns > 0); if (summaries.length < 2) return {};
  let state = 0x165ca12b;
  const random = () => { state ^= state << 13; state ^= state >>> 17; state ^= state << 5; return (state >>> 0) / 4294967296; };
  const output = {};
  for (const key of Object.keys(summaries[0].rates)) {
    const draws = [];
    for (let iteration = 0; iteration < 400; iteration++) {
      let numerator = 0, denominator = 0;
      for (let sample = 0; sample < summaries.length; sample++) { const stat = summaries[Math.floor(random() * summaries.length)].rates[key]; numerator += stat.numerator; denominator += stat.denominator; }
      if (denominator) draws.push(numerator / denominator);
    }
    output[key] = { method: 'conversation-cluster percentile bootstrap, deterministic seed, 400 resamples', ci95: draws.length ? [round(quantile(draws, .025)), round(quantile(draws, .975))] : null };
  }
  return output;
}
function splitBySession(conversations) {
  const train = [], heldOutRaw = [], noHoldout = [];
  for (const conversation of conversations) {
    const turns = conversation.turns, sessions = [...new Set(turns.map(turn => turn.session))];
    if (sessions.length < 2 || turns.length < 5) { train.push(...turns); if (turns.length) noHoldout.push(conversation.id); continue; }
    const candidates = sessions.slice(1).map(session => ({ session, count: turns.filter(turn => turn.session < session).length }));
    candidates.sort((a, b) => Math.abs(a.count / turns.length - .8) - Math.abs(b.count / turns.length - .8) || a.session - b.session);
    train.push(...turns.filter(turn => turn.session < candidates[0].session)); heldOutRaw.push(...turns.filter(turn => turn.session >= candidates[0].session));
  }
  const trainFingerprints = new Set(train.flatMap(messageFingerprints)), heldOut = [], removed = [];
  for (const turn of heldOutRaw) {
    if (messageFingerprints(turn).some(fingerprint => trainFingerprints.has(fingerprint))) removed.push(turn);
    else heldOut.push(turn);
  }
  return { train, heldOutRaw, heldOut, removed, noHoldout };
}
function rankedEvidence(turns, names) {
  const messages = turns.flatMap(turn => turn.messages), phrases = new Map(), prefixes = new Map(), endings = new Map(), faces = new Map();
  for (const turn of turns) for (const message of turn.messages) {
    for (const face of message.faces) { const entry = faces.get(face) || { count: 0, conversations: new Set() }; entry.count++; entry.conversations.add(turn.conversation); faces.set(face, entry); }
    if (!message.text) continue;
    const safe = sensitiveText(message.text, names), length = [...safe].length;
    if (length >= 1 && length <= 28 && !/\[(?:链接|邮箱|号码|UID|称呼|密钥|本地路径)\]/u.test(safe)) {
      const entry = phrases.get(safe) || { count: 0, conversations: new Set() }; entry.count++; entry.conversations.add(turn.conversation); phrases.set(safe, entry);
    }
    if (length >= 3) for (const width of [2, 3]) {
      const prefix = [...safe].slice(0, width).join(''), ending = [...safe].slice(-width).join('');
      if (!/[\[\]\s\d]/u.test(prefix)) increment(prefixes, prefix); if (!/[\[\]\s\d]/u.test(ending)) increment(endings, ending);
    }
  }
  const ordered = map => [...map].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], 'zh-CN')).slice(0, 48).map(([text, count]) => ({ text, count }));
  return { measuredFrom: 'training partition only', textMessageCount: messages.filter(message => message.text).length,
    repeatedUtterances: [...phrases].filter(([, entry]) => entry.count >= 5 && entry.conversations.size >= 3).sort((a, b) => b[1].count - a[1].count).slice(0, 100).map(([text, entry]) => ({ text, count: entry.count, conversations: entry.conversations.size })),
    prefixes: ordered(prefixes), endings: ordered(endings), nativeFaces: [...faces].sort((a, b) => b[1].count - a[1].count).map(([id, entry]) => ({ id, count: entry.count, conversations: entry.conversations.size })) };
}
function excerptCandidates(turns, names, maximum = 96, options = {}) {
  const { minimumOwnerChars = 0, categories = [...Object.keys(CATEGORIES), 'other'], conversationLimit = 6 } = options;
  const candidates = turns.filter(turn => turn.incoming.length && turn.messages.length <= 5 && turn.messages.some(message => message.text) && turn.messages.every(message => !message.exclusion)
    && turn.messages.reduce((count, message) => count + [...message.text].length, 0) >= minimumOwnerChars);
  const output = [], used = new Set(), perConversation = new Map();
  const ordered = [...candidates].sort((a, b) => b.startTs - a.startTs || a.id.localeCompare(b.id));
  for (let sweep = 0; sweep < 12 && output.length < maximum; sweep++) for (const category of categories) {
    const turn = ordered.find(candidate => !used.has(candidate.id) && candidate.categories.includes(category) && (perConversation.get(candidate.conversation) || 0) < Math.min(conversationLimit, sweep + 1));
    if (!turn) continue;
    used.add(turn.id);
    const incoming = turn.incoming.map(message => sensitiveText(message.text, names)), replies = turn.messages.map(message => ({ text: sensitiveText(message.text, names), qqFaceIds: message.faces,
      material: Boolean(message.material), nativeFaceImageCount: message.nativeFaceImageCount || 0 }));
    if (incoming.some(text => text.length > 220) || replies.some(message => message.text.length > 220) || [...incoming, ...replies.map(message => message.text)].join('\n').length > 500) continue;
    increment(perConversation, turn.conversation); output.push({ conversation: turn.conversation, turnId: turn.id, month: new Date(turn.startTs).toISOString().slice(0, 7), partition: 'train', category, heuristicLabels: turn.categories,
      incoming, ownerReplies: replies, provenance: { ownerVerified: true, quotedOrForwarded: false, textTruncated: false, messageCount: turn.messages.length, sourceMessageDigests: turn.messages.map(message => pseudonym(`${turn.conversation}\0${message.id}`)) } });
    if (output.length >= maximum) break;
  }
  return output;
}

function calendarDate(ts) { return new Date(ts + 8 * 3600000).toISOString().slice(0, 10); }
function eligibleDateRange(turns) {
  let first = Infinity, last = -Infinity;
  for (const turn of turns) for (const row of turn.messages) { first = Math.min(first, row.ts); last = Math.max(last, row.ts); }
  return { timezone: 'UTC+08:00', fromDate: Number.isFinite(first) ? calendarDate(first) : null, toDate: Number.isFinite(last) ? calendarDate(last) : null,
    expressiveConversations: new Set(turns.filter(turn => turn.messages.some(isExpressive)).map(turn => turn.conversation)).size };
}
function templateSensitivity(conversations, train, rules) {
  if (!rules) return null;
  if (!Array.isArray(rules) || rules.length > 64 || rules.some(rule => !rule || typeof rule.id !== 'string' || !/^[a-z0-9-]{1,64}$/u.test(rule.id) || typeof rule.value !== 'string' || !rule.value || rule.value.length > 256 || !['exact', 'prefix', 'includes'].includes(rule.match) || typeof rule.exclude !== 'boolean')) throw new Error('Template rules must be bounded explicit literal rules');
  const eligible = conversations.flatMap(conversation => conversation.rows.filter(row => row.role === 'owner' && !row.exclusion && row.kind !== 'empty').map(row => ({ ...row, conversation: conversation.id })));
  const matches = (row, rule) => rule.match === 'exact' ? row.text === rule.value : rule.match === 'prefix' ? row.text.startsWith(rule.value) : row.text.includes(rule.value);
  const selected = new Set(), trainRows = new Set(train.flatMap(turn => turn.messages.map(row => `${turn.conversation}\0${row.id}`)));
  const candidates = rules.map(rule => {
    const rows = eligible.filter(row => matches(row, rule)), counts = new Map(), months = new Map();
    for (const row of rows) { increment(counts, row.conversation); increment(months, calendarDate(row.ts).slice(0, 7)); if (rule.exclude) selected.add(`${row.conversation}\0${row.id}`); }
    return { ...rule, ownerMessages: rows.length, trainingMessages: rows.filter(row => trainRows.has(`${row.conversation}\0${row.id}`)).length, conversations: counts.size,
      topConversations: [...counts].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([conversation, count]) => ({ conversation, count, share: rows.length ? round(count / rows.length) : null })), monthly: Object.fromEntries([...months].sort(([a], [b]) => a.localeCompare(b))) };
  });
  const filteredTurns = conversations.flatMap(conversation => assembleTurns(conversation.rows.map(row => selected.has(`${conversation.id}\0${row.id}`) ? { ...row, exclusion: 'explicitTemplateSensitivity' } : row), conversation.id));
  return { interpretation: 'These are manually specified literal template/test candidates. Matching does not prove AI authorship; candidate-only normal greetings are not excluded. Primary corpus statistics and selected excerpts remain unchanged.',
    candidateRules: candidates, excludedOwnerMessages: selected.size, baseline: summarize(conversations.flatMap(conversation => conversation.turns)), afterExplicitExclusions: summarize(filteredTurns),
    dateRanges: { allEligibleOwner: eligibleDateRange(conversations.flatMap(conversation => conversation.turns)), trainingOwner: eligibleDateRange(train) } };
}

export function analyzeCorpus(exports, options) {
  const ownerUin = String(options.ownerUin || ''); if (!/^[1-9]\d{4,19}$/u.test(ownerUin)) throw new Error('An explicit valid owner UIN is required');
  const counters = { files: 0, verifiedOwnerFiles: 0, identityUnverifiedFiles: 0, selfConversationFiles: 0, explicitBotFiles: 0, unknownConversationFiles: 0, messagesSeen: 0, ownerMessages: 0, peerMessages: 0, unknownSenderMessages: 0, identityConflict: 0, duplicate: 0, conflictingDuplicate: 0, invalidTimestamp: 0, missingMessageId: 0, ownerIncluded: 0 };
  const exclusions = new Map(), conversations = new Map(), names = new Set(), schema = new Map(), units = new Map(), sourceHash = crypto.createHash('sha256');
  for (const source of exports) {
    counters.files++; sourceHash.update(digest(source)); const { manifest, messages } = parseExport(source), chat = manifest.chat || {};
    const ownerUid = verifiedOwner(manifest, ownerUin); if (!ownerUid) { counters.identityUnverifiedFiles++; continue; } counters.verifiedOwnerFiles++;
    for (const sender of manifest.senders || []) for (const name of [sender.displayName, ...(sender.aliases || [])]) if (typeof name === 'string' && name.length >= 2) names.add(name);
    for (const name of [chat.name, chat.selfName]) if (name) names.add(String(name));
    if (chat.peerUid === ownerUid || String(chat.peerUin || '') === ownerUin) { counters.selfConversationFiles++; continue; }
    if (explicitBot(chat)) { counters.explicitBotFiles++; continue; }
    const identity = conversationIdentity(chat); if (!identity) { counters.unknownConversationFiles++; continue; }
    const conversationId = pseudonym(identity); let conversation = conversations.get(conversationId);
    if (!conversation) { conversation = { id: conversationId, type: chat.type || 'unknown', rows: [], ids: new Map(), files: 0, exportMessages: 0 }; conversations.set(conversationId, conversation); } conversation.files++;
    for (const message of messages()) {
      counters.messagesSeen++; conversation.exportMessages++; for (const key of Object.keys(message)) increment(schema, key);
      const role = classifySender(message, ownerUin, ownerUid); if (role === 'conflict') counters.identityConflict++;
      else if (role === 'owner') counters.ownerMessages++; else if (role === 'peer') counters.peerMessages++; else counters.unknownSenderMessages++;
      const id = String(message.id || ''); if (!id) { counters.missingMessageId++; continue; }
      const fingerprint = digest(JSON.stringify([message.ts, message.uid, message.senderUid, message.uin, message.senderUin, message.sender?.uid, message.sender?.uin,
        message.text, message.textTruncated, message.html, message.quoted, message.forwarded, message.aiGenerated, message.ai_reply_status, message.isSystem]));
      if (conversation.ids.has(id)) { counters.duplicate++; if (conversation.ids.get(id) !== fingerprint) { counters.conflictingDuplicate++; const previous = conversation.rows.find(row => row.id === id); if (previous) previous.exclusion = 'conflictingDuplicate'; } continue; }
      conversation.ids.set(id, fingerprint);
      const ts = Number(message.ts), iso = Date.parse(`${message.date || ''}T00:00:00Z`);
      if (!Number.isFinite(ts) || ts < 946684800000 || ts > 4102444800000 || (Number.isFinite(iso) && Math.abs(ts - iso) > 172800000)) { counters.invalidTimestamp++; continue; }
      increment(units, 'unixMillisecondsVerifiedRange');
      const content = extractMessage(message); if (role === 'conflict') content.exclusion = 'identityConflict';
      conversation.rows.push({ id, ts, order: conversation.rows.length, role, ...content });
    }
  }
  const active = [...conversations.values()];
  for (const conversation of active) {
    conversation.rows.sort((a, b) => a.ts - b.ts || a.order - b.order); conversation.turns = assembleTurns(conversation.rows, conversation.id); conversation.ids.clear();
    for (const row of conversation.rows) if (row.role === 'owner') {
      if (row.exclusion) increment(exclusions, row.exclusion);
      else if (row.kind !== 'empty') counters.ownerIncluded++;
    }
  }
  if (!active.some(conversation => conversation.turns.length)) throw new Error('No verified original owner turns are available');
  const turns = active.flatMap(conversation => conversation.turns), split = splitBySession(active), namesOrdered = [...names].sort((a, b) => b.length - a.length);
  const ranked = [...active].sort((a, b) => b.turns.reduce((n, turn) => n + turn.messages.length, 0) - a.turns.reduce((n, turn) => n + turn.messages.length, 0));
  const summary = summarize(turns), train = summarize(split.train), heldOut = summarize(split.heldOut), latest = turns.reduce((last, turn) => Math.max(last, turn.endTs), -Infinity), cutoff = latest - 90 * 86400000;
  const months = new Map(); for (const turn of turns) { const month = new Date(turn.startTs).toISOString().slice(0, 7); const monthTurns = months.get(month) || []; monthTurns.push(turn); months.set(month, monthTurns); }
  const recent = turns.filter(turn => turn.startTs >= cutoff), earlier = turns.filter(turn => turn.endTs < cutoff), recentSummary = summarize(recent), earlierSummary = summarize(earlier);
  const enoughRecency = recentSummary.textMessages >= 100 && earlierSummary.textMessages >= 100;
  const heuristicPatterns = Object.keys(CATEGORIES).map(category => {
    const selected = split.train.filter(turn => turn.categories.includes(category)), selectedHeldOut = split.heldOut.filter(turn => turn.categories.includes(category));
    return { category, method: 'Nonexclusive owner utterance form cues from Chinese lexical/orthographic heuristics; does not classify incoming intent or infer relationship', train: summarize(selected), heldOut: summarize(selectedHeldOut) };
  });
  const sensitivity = [1, 2, 3].map(count => { const removedIds = new Set(ranked.slice(0, count).map(conversation => conversation.id)); return { removedTopConversations: count, remainingConversations: active.filter(conversation => !removedIds.has(conversation.id) && conversation.turns.length).length, metrics: summarize(turns.filter(turn => !removedIds.has(turn.conversation))) }; });
  const perConversation = active.map(conversation => ({ id: conversation.id, type: conversation.type, files: conversation.files, exportedRowsBeforeDedup: conversation.exportMessages, metrics: summarize(conversation.turns), firstMonth: conversation.turns.length ? new Date(conversation.turns[0].startTs).toISOString().slice(0, 7) : null, lastMonth: conversation.turns.length ? new Date(conversation.turns.at(-1).endTs).toISOString().slice(0, 7) : null }));
  const ownerRows = active.flatMap(conversation => conversation.rows).filter(row => row.role === 'owner' && !row.exclusion), nativeImageCount = ownerRows.reduce((count, row) => count + row.nativeFaceImageCount, 0);
  const resolvedFaces = ownerRows.flatMap(row => row.faces), catalog = options.allowedFaceIds ? new Set(options.allowedFaceIds.map(String)) : null;
  const report = { schemaVersion: 2, analyzerVersion: '2.0.0', sourceDigest: sourceHash.digest('hex'), generatedAt: new Date().toISOString(),
    method: { timestamp: 'QCE message.ts interpreted as Unix milliseconds, range/date checked', attribution: 'Explicit owner UIN-to-UID mapping and exact sender identity; conflicting fields excluded; never display-name or isSelf attribution', deduplication: 'Conversation scope + message ID; conflicting replay content excludes original row too',
      textExtraction: 'text-content spans only when present; native face labels and material placeholders never enter text lengths', turnGapSeconds: 120, shortBurstGapSeconds: 30, sessionGapSeconds: 1800,
      bubbleMetrics: 'textBubbleCount, multipleBubbles, shortBurst and interBubbleSeconds count original owner text/native-face bubbles; material-only bubbles are excluded from these denominators. Newline counts characters within a single original text bubble.',
      uncertainty: 'Wilson intervals are descriptive and assume independent items. Conversation cluster bootstrap is also shown to expose concentration; neither estimates identity resemblance.' },
    quality: { counters, ownerExclusionReasons: Object.fromEntries(exclusions), messageSchemaFields: Object.fromEntries(schema), timestampEvidence: Object.fromEntries(units),
      limitations: ['Exporter does not reliably identify historical AI-generated content without explicit provenance; unmarked AI messages cannot be conclusively removed.', 'Bot/service conversations are excluded only with explicit metadata; names containing bot or assistant are not sufficient evidence.', 'Quoted and forwarded messages are excluded whole to avoid copying other authors. This selection can underrepresent quoting habits.', 'Media placeholders are counted as material only; no claim is made about image, voice, or attachment meaning.', 'Context categories are overlapping lexical heuristics, not verified social roles or relationships.', 'Candidate anonymization is rule based and requires human review before sharing.', 'QQ message timing does not measure reading or typing latency; grouping thresholds describe exported bubbles only.', 'Retaining only content-distinct held-out turns biases held-out samples toward rarer language; raw split counts and removal are reported.'] },
    corpus: { conversations: active.length, conversationsWithOwnerTurns: active.filter(conversation => conversation.turns.length).length, firstMonth: new Date(turns.reduce((first, turn) => Math.min(first, turn.startTs), Infinity)).toISOString().slice(0, 7), lastMonth: new Date(latest).toISOString().slice(0, 7), metrics: summary },
    nativeFaceCoverage: { source: 'Original eligible owner HTML img elements with face-emoji class only; excludes quoted/forwarded/flagged content', nativeFaceImages: nativeImageCount,
      resolvedNumericIds: resolvedFaces.length, sourceResolution: wilson(resolvedFaces.length, nativeImageCount), catalogProvided: Boolean(catalog), catalogSize: catalog?.size ?? null,
      resolvedIdsSupportedByCatalog: catalog ? resolvedFaces.filter(id => catalog.has(id)).length : null, catalogCoverageAmongResolved: catalog ? wilson(resolvedFaces.filter(id => catalog.has(id)).length, resolvedFaces.length) : null,
      unmatchedNumericIds: catalog ? [...new Set(resolvedFaces.filter(id => !catalog.has(id)))].sort((a, b) => Number(a) - Number(b)) : null,
      interpretation: 'HTML resolution and configured program catalog coverage are separate measurements; neither proves remote asset availability or QQ client rendering.' },
    concentration: { topConversations: ranked.slice(0, 3).map(conversation => ({ id: conversation.id, ownerMessages: conversation.turns.reduce((count, turn) => count + turn.messages.length, 0), share: round(conversation.turns.reduce((count, turn) => count + turn.messages.length, 0) / summary.messages) })), macroRates: macroRates(active.map(conversation => conversation.turns)), clusteredRateIntervals: clusteredRateIntervals(active.map(conversation => conversation.turns)), sensitivity },
    splits: { method: 'Within each conversation, complete latest sessions nearest 20% of turns are held out; a 30-minute session is never split. Conversations with fewer than 5 turns or fewer than 2 sessions remain training-only. Any held-out turn containing an exact normalized text+face message fingerprint present in training is removed whole.',
      trainingTurns: split.train.length, rawHeldOutTurns: split.heldOutRaw.length, contentOverlapRemovedTurns: split.removed.length, heldOutTurns: split.heldOut.length, noHoldoutConversations: split.noHoldout.length,
      train, rawHeldOut: summarize(split.heldOutRaw), heldOut, trainingTurnDigests: split.train.map(turn => turn.id), heldOutTurnDigests: split.heldOut.map(turn => turn.id), exactMessageContentOverlapAfterRemoval: 0 },
    temporal: { monthly: [...months].sort(([a], [b]) => a.localeCompare(b)).map(([month, rows]) => ({ month, metrics: summarize(rows) })), recent90Days: { anchor: 'Latest observed owner turn, not current calendar date', enoughDataForComparison: enoughRecency, recent: recentSummary, earlier: earlierSummary,
      rateDelta: enoughRecency ? Object.fromEntries(Object.keys(recentSummary.rates).filter(key => recentSummary.rates[key].rate !== null && earlierSummary.rates[key].rate !== null).map(key => [key, round(recentSummary.rates[key].rate - earlierSummary.rates[key].rate)])) : null } },
    dateRanges: { allEligibleOwner: eligibleDateRange(turns), trainingOwner: eligibleDateRange(split.train), heldOutOwner: eligibleDateRange(split.heldOut) },
    templateSensitivity: templateSensitivity(active, split.train, options.templateRules),
    conversations: perConversation, contextPatterns: heuristicPatterns, lexicalEvidence: rankedEvidence(split.train, namesOrdered), excerptCandidates: excerptCandidates(split.train, namesOrdered, options.maxExcerpts || 96),
    longerTurnCandidates: excerptCandidates(split.train, namesOrdered, 32, { minimumOwnerChars: 24, categories: ['explanation', 'disagreement', 'coordination'], conversationLimit: 3 }),
  };
  return report;
}

export function* readExports(input) {
  const stat = fs.lstatSync(input); if (stat.isSymbolicLink()) throw new Error('Input symlinks are not accepted');
  const paths = stat.isDirectory() ? fs.readdirSync(input).filter(name => /\.html$/iu.test(name)).sort().map(name => path.join(input, name)) : [input];
  for (const filename of paths) { const info = fs.lstatSync(filename); if (!info.isFile() || info.isSymbolicLink() || info.size > MAX_HTML_BYTES) throw new Error('Input must consist of bounded regular HTML exports'); yield fs.readFileSync(filename, 'utf8'); }
}
function argumentsFrom(argv) {
  const values = {}; for (let index = 0; index < argv.length; index += 2) { if (!/^--[a-z-]+$/u.test(argv[index]) || !argv[index + 1]) throw new Error('Expected --input, --owner and --output'); values[argv[index].slice(2)] = argv[index + 1]; } return values;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = argumentsFrom(process.argv.slice(2)); if (!args.input || !args.owner || !args.output) throw new Error('Usage: --input <QCE HTML folder> --owner <UIN> --output <new private report JSON>');
    const facesPath = args.faces || fileURLToPath(new URL('../../src/data/qq-faces.json', import.meta.url));
    const allowedFaceIds = JSON.parse(fs.readFileSync(facesPath, 'utf8')).map(face => String(face.id));
    const templateRules = args['template-rules'] ? JSON.parse(fs.readFileSync(args['template-rules'], 'utf8')) : undefined;
    const report = analyzeCorpus(readExports(args.input), { ownerUin: args.owner, allowedFaceIds, templateRules });
    fs.mkdirSync(path.dirname(path.resolve(args.output)), { recursive: true }); fs.writeFileSync(args.output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
    console.log(JSON.stringify({ schemaVersion: report.schemaVersion, conversations: report.corpus.conversations, textMessages: report.corpus.metrics.textMessages, trainingTurns: report.splits.trainingTurns, heldOutTurns: report.splits.heldOutTurns, excerpts: report.excerptCandidates.length }));
  } catch (error) { console.error(error instanceof Error ? error.message : 'Corpus analysis failed'); process.exitCode = 1; }
}
