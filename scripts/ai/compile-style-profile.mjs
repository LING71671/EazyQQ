import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const SITUATIONS = new Set(['acknowledgement', 'question', 'coordination', 'explanation', 'disagreement', 'support', 'repair', 'casual']);
const METRIC_EVIDENCE = new Set(['text_distribution', 'punctuation', 'burst_distribution', 'face_distribution']);
const CATALOG = fileURLToPath(new URL('../../src/data/qq-faces.json', import.meta.url));
const MAX_PACK_BYTES = 64 * 1024;
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const round = value => Number(value.toFixed(6));
function fail(message) { throw new Error(message); }
function object(value, keys, required = keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !keys.includes(key)) || required.some(key => !(key in value))) fail('Unexpected or missing decision field');
}
function integer(value, minimum = 0, maximum = 100_000_000) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) fail('Invalid bounded count');
  return value;
}
function slug(value, maximum = 80) { return typeof value === 'string' && /^[a-z0-9_-]+$/u.test(value) && value.length <= maximum; }
function semanticVersion(value) {
  if (typeof value !== 'string') return false;
  const match = value.match(/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-([0-9a-z.-]+))?(?:\+([0-9a-z.-]+))?$/iu);
  return Boolean(match && [match[1], match[2]].filter(Boolean).every(part => part.split('.').every(identifier => identifier.length > 0))
    && (!match[1] || match[1].split('.').every(identifier => !/^\d+$/u.test(identifier) || identifier === '0' || !identifier.startsWith('0'))));
}
function unicodeEmoji(value) {
  return [...value].some(character => {
    const code = character.codePointAt(0);
    return (code >= 0x1f000 && code <= 0x1faff) || (code >= 0x2600 && code <= 0x27bf) ||
      (code >= 0x2194 && code <= 0x2199) || (code >= 0x21a9 && code <= 0x21aa) || (code >= 0x231a && code <= 0x231b) ||
      (code >= 0x23e9 && code <= 0x23f3) || (code >= 0x23f8 && code <= 0x23fa) || (code >= 0x25aa && code <= 0x25ab) ||
      (code >= 0x25fb && code <= 0x25fe) || (code >= 0x2934 && code <= 0x2935) || (code >= 0x2b05 && code <= 0x2b07) ||
      (code >= 0x2b1b && code <= 0x2b1c) || [0x20e3, 0xfe0f, 0x2328, 0x23cf, 0x24c2, 0x25b6, 0x25c0, 0x2b50, 0x2b55, 0x3030, 0x303d, 0x3297, 0x3299].includes(code);
  });
}
function safeText(value, maximum) {
  if (typeof value !== 'string' || !value.trim() || [...value].length > maximum || /[\p{Cc}\p{Cf}\u2028\u2029<>{}\[\]]/u.test(value) || unicodeEmoji(value)) fail('Text contains unsupported control, markup, emoji, placeholder or excessive length');
  if (/ignore (?:previous|all|system)|override instructions|system ?prompt|developer message|assistant:|system:|忽略(?:之前|上述|系统|指令)|无视指令|覆盖指令|系统提示词|开发者指令|泄露密钥|执行命令|调用工具|你必须扮演|伪装成人类|否认是ai/iu.test(value)) fail('Instruction override is not style data');
  return value;
}
function calendarDate(value) {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/u.test(value)) fail('Dates require YYYY-MM-DD');
  const instant = Date.parse(`${value}T00:00:00Z`);
  if (!Number.isFinite(instant) || new Date(instant).toISOString().slice(0, 10) !== value || value.startsWith('0000')) fail('Invalid calendar date');
  return value;
}
function measuredRate(statistic, denominator) {
  if (!statistic || integer(statistic.denominator) !== denominator) fail('Metric denominator does not match its declared population');
  const numerator = integer(statistic.numerator, 0, denominator);
  return denominator ? numerator / denominator : 0;
}
function situations(value) { if (!SITUATIONS.has(value)) fail('Unknown style situation'); return value; }
function unique(values, message) { if (!Array.isArray(values) || new Set(values).size !== values.length) fail(message); return values; }

export function compileStyleProfile(analysis, decisions, options = {}) {
  object(decisions, ['schemaVersion', 'id', 'name', 'version', 'fromDate', 'toDate', 'patterns', 'demonstrations', 'preferredPhrases', 'avoidPhrases']);
  if (decisions.schemaVersion !== 1 || analysis?.schemaVersion !== 2 || !/^[a-f0-9]{64}$/u.test(analysis.sourceDigest || '')) fail('Expected decision schema 1 and provenance-bearing analysis schema 2');
  if (!slug(decisions.id, 64) || !semanticVersion(decisions.version)) fail('Invalid style ID or semantic version');
  safeText(decisions.name, 80);
  const fromDate = calendarDate(decisions.fromDate), toDate = calendarDate(decisions.toDate);
  if (fromDate > toDate) fail('Date range is reversed');
  const declaredRange = analysis.dateRanges?.trainingOwner || analysis.templateSensitivity?.dateRanges?.trainingOwner;
  if (declaredRange && (declaredRange.fromDate !== fromDate || declaredRange.toDate !== toDate)) fail('Decision dates do not match the measured training range');
  const allowedFaces = new Set((options.allowedFaceIds || JSON.parse(fs.readFileSync(CATALOG, 'utf8')).map(face => face.id)).map(String));
  const training = new Set(unique(analysis.splits?.trainingTurnDigests, 'Training digests must be unique'));
  const heldOut = new Set(unique(analysis.splits?.heldOutTurnDigests, 'Held-out digests must be unique'));
  if ([...training].some(id => !slug(id) || heldOut.has(id))) fail('Training and held-out sources overlap or have invalid digests');
  const candidates = new Map();
  const allCandidates = [...(analysis.excerptCandidates || []), ...(analysis.longerTurnCandidates || []), ...(analysis.templateSensitivity?.longerExamples || [])];
  for (const candidate of allCandidates) {
    if (!candidate || !slug(candidate.turnId) || !slug(candidate.conversation)) fail('Candidate lacks anonymous source digests');
    const existing = candidates.get(candidate.turnId);
    if (existing && (existing.conversation !== candidate.conversation || existing.partition !== candidate.partition || JSON.stringify(existing.ownerReplies) !== JSON.stringify(candidate.ownerReplies) || JSON.stringify(existing.provenance) !== JSON.stringify(candidate.provenance))) fail('Conflicting excerpt candidate identity');
    if (!existing) candidates.set(candidate.turnId, candidate);
  }
  function evidence(id) {
    const candidate = candidates.get(id);
    if (!training.has(id) || heldOut.has(id) || !candidate || candidate.partition !== 'train' || candidate.provenance?.ownerVerified !== true || candidate.provenance?.quotedOrForwarded !== false || candidate.provenance?.textTruncated !== false) fail('Evidence must be an available verified original training turn');
    if (!Array.isArray(candidate.ownerReplies) || candidate.ownerReplies.length !== candidate.provenance.messageCount) fail('Incomplete source reply sequence');
    return candidate;
  }
  const train = analysis.splits.train, holdout = analysis.splits.heldOut;
  if (!train || !holdout) fail('Training and held-out measurements are required');
  const textCount = integer(train.textMessages, 1), faceOnlyCount = integer(train.faceOnlyMessages), messageCount = integer(textCount + faceOnlyCount, 1);
  const turnCount = integer(train.expressiveTurns, 1, messageCount), holdoutTurnCount = integer(holdout.expressiveTurns);
  if (training.size !== train.turns || heldOut.size !== holdout.turns) fail('Turn digests and measured populations disagree');
  const candidateConversations = new Set([...candidates.values()].filter(candidate => training.has(candidate.turnId)).map(candidate => candidate.conversation));
  const measuredConversationCount = declaredRange?.expressiveConversations;
  const conversationCount = measuredConversationCount === undefined ? candidateConversations.size : integer(measuredConversationCount, 1, turnCount);
  if (!conversationCount || conversationCount > turnCount) fail('No credible training conversation count');
  const terminalRate = measuredRate(train.rates?.terminalPunctuation, textCount);
  const questionOnlyRate = measuredRate(train.rates?.questionOnly, textCount);
  const faceRate = measuredRate(train.rates?.nativeFace, messageCount);
  const faceOnlyRate = measuredRate(train.rates?.standaloneNativeFace, messageCount);
  if (faceOnlyRate > faceRate || train.rates.standaloneNativeFace.numerator !== faceOnlyCount) fail('Native face populations are inconsistent');
  const textMedian = integer(train.textChars?.median), textP90 = integer(train.textChars?.p90, textMedian, 100_000);
  const bubbleMedian = train.textBubbleCount?.median, bubbleP90 = train.textBubbleCount?.p90;
  if (![bubbleMedian, bubbleP90].every(Number.isFinite) || bubbleMedian < 1 || bubbleP90 < bubbleMedian || bubbleP90 > messageCount) fail('Invalid measured bubble distribution');
  const faceCounts = analysis.lexicalEvidence?.nativeFaces || [];
  let supportedFaceCount = 0, resolvedFaceCount = 0;
  for (const face of faceCounts) { const count = integer(face.count); resolvedFaceCount += count; if (allowedFaces.has(String(face.id))) supportedFaceCount += count; }
  const repeated = new Map((analysis.lexicalEvidence?.repeatedUtterances || []).map(entry => [entry.text, entry]));
  const preferredPhrases = unique(decisions.preferredPhrases, 'Preferred phrases must be unique');
  const avoidPhrases = unique(decisions.avoidPhrases, 'Avoid phrases must be unique');
  if (preferredPhrases.length > 12 || avoidPhrases.length > 16) fail('Too many phrase choices');
  for (const phrase of preferredPhrases) { safeText(phrase, 12); const measured = repeated.get(phrase); if (!measured || measured.count < 5 || measured.conversations < 3) fail('Preferred phrase lacks repeated cross-conversation training evidence'); }
  for (const phrase of avoidPhrases) safeText(phrase, 24);
  if (!Array.isArray(decisions.patterns) || decisions.patterns.length < 1 || decisions.patterns.length > 16 || !Array.isArray(decisions.demonstrations) || decisions.demonstrations.length < 1 || decisions.demonstrations.length > 24) fail('Pattern or demonstration bounds exceeded');
  const usedPatternIds = new Set(), patternAudit = [];
  const patterns = decisions.patterns.map(pattern => {
    object(pattern, ['id', 'situation', 'observation', 'confidence', 'evidenceTurnIds', 'metricEvidence'], ['id', 'situation', 'observation', 'confidence', 'evidenceTurnIds']);
    if (!slug(pattern.id, 64) || usedPatternIds.has(pattern.id) || !['low', 'medium', 'high'].includes(pattern.confidence)) fail('Invalid pattern ID or confidence');
    usedPatternIds.add(pattern.id); safeText(pattern.observation, 240); situations(pattern.situation);
    const ids = unique(pattern.evidenceTurnIds, 'Pattern source turns must be unique');
    const sourceConversations = new Set(ids.map(id => evidence(id).conversation));
    let evidenceCount = ids.length, observedConversations = sourceConversations.size;
    if (pattern.metricEvidence !== undefined) {
      if (!METRIC_EVIDENCE.has(pattern.metricEvidence)) fail('Unknown aggregate metric evidence');
      evidenceCount = pattern.metricEvidence === 'burst_distribution' ? turnCount : pattern.metricEvidence === 'face_distribution' ? messageCount : textCount;
      observedConversations = conversationCount;
    }
    if (!evidenceCount || !observedConversations || observedConversations > evidenceCount) fail('Pattern lacks evidence');
    patternAudit.push({ id: pattern.id, evidenceTurnIds: ids, metricEvidence: pattern.metricEvidence || null, population: pattern.metricEvidence ? 'training aggregate only' : 'explicit reviewed training turns', evidenceCount, conversationCount: observedConversations });
    return { id: pattern.id, situation: pattern.situation, observation: pattern.observation, evidenceCount, conversationCount: observedConversations, confidence: pattern.confidence };
  });
  const demonstrationAudit = [], demonstrationIds = new Set();
  const demonstrations = decisions.demonstrations.map(choice => {
    object(choice, ['turnId', 'situation', 'context', 'messageIndexes'], ['turnId', 'situation', 'context']);
    if (demonstrationIds.has(choice.turnId)) fail('Demonstrations must use distinct reviewed turns');
    demonstrationIds.add(choice.turnId); const source = evidence(choice.turnId); safeText(choice.context, 160); situations(choice.situation);
    const indices = choice.messageIndexes === undefined ? source.ownerReplies.map((_, index) => index) : unique(choice.messageIndexes, 'Selected source indexes must be unique');
    if (indices.length < 1 || indices.length > 5 || indices.some((index, position) => !Number.isInteger(index) || index < 0 || index >= source.ownerReplies.length || (position > 0 && index <= indices[position - 1]))) fail('Select 1 to 5 source replies in original order');
    const replies = indices.map(index => {
      const original = source.ownerReplies[index];
      if (original.material !== false || !Array.isArray(original.qqFaceIds) || original.qqFaceIds.some(id => !allowedFaces.has(String(id)))) fail('Selected reply contains material, lacks material provenance or uses unsupported native faces');
      if (original.nativeFaceImageCount !== original.qqFaceIds.length) fail('Selected reply has unresolved native face images');
      const text = original.text || '';
      if (text) safeText(text, 100);
      const reply = text + original.qqFaceIds.map(id => `[CQ:face,id=${id}]`).join('');
      if (!reply || [...reply].length > 100) fail('Selected reply is empty material or exceeds the complete-example bound');
      return reply;
    });
    const hashes = source.provenance.sourceMessageDigests;
    if (!Array.isArray(hashes) || hashes.length !== source.ownerReplies.length || hashes.some(hash => !slug(hash))) fail('Source message digests are missing');
    demonstrationAudit.push({ turnId: choice.turnId, conversation: source.conversation, selectedMessageIndexes: indices,
      excludedMessageIndexes: source.ownerReplies.map((_, index) => index).filter(index => !indices.includes(index)),
      sourceMessageDigests: indices.map(index => hashes[index]), transformations: { context: 'assistant-reviewed context paraphrase', ownerText: 'exact sanitized source text', nativeFaces: 'known source IDs appended in recorded ID order', delivery: 'one message; no automatic multi-bubble sends' },
      missingSourceMaterialMetadata: indices.some(index => source.ownerReplies[index].material === undefined || source.ownerReplies[index].nativeFaceImageCount === undefined) });
    return { situation: choice.situation, context: choice.context, replies, evidenceRefs: [choice.turnId] };
  });
  const qqFaceIds = faceCounts.filter(face => allowedFaces.has(String(face.id))).sort((a, b) => b.count - a.count || Number(a.id) - Number(b.id)).slice(0, 16).map(face => String(face.id));
  const limitations = ['样本反映训练记录中的表达分布，不代表所有关系和场景。', '逐会话尾部留出用于稳定性检查，不代表全局时间外推或拟人准确率。', '没有可靠来源标记的旧 AI 内容无法完全识别；引用转发整条排除会低估相关习惯。', '情境经助手逐条语义复核，非真人标注；词法标签不等同于对方意图，示例不继承旧事实。', 'QQ 原生表情覆盖仅统计训练记录可解析编号的目录支持，不能证明所有原始表情或远端预览可用。', '多气泡指标来自原始发送记录；当前应用只返回一条消息，不把气泡数冒充换行偏好。', 'tone 的 casual 为现有枚举选择；具体语气以经语义复核的条件模式为准。', measuredConversationCount === undefined ? '训练会话数按可核对候选会话给出保守下界；不冒用全时段会话总数。' : '统计模式的会话数指训练表达会话总体；不声称每条模式均在全部会话出现。'];
  const pack = { schemaVersion: 2, id: decisions.id, name: decisions.name, version: decisions.version,
    rules: { sentenceLength: textMedian <= 16 ? 'short' : textMedian <= 40 ? 'balanced' : 'detailed', punctuation: terminalRate < .2 ? 'minimal' : 'standard', tone: 'casual', lineBreaks: 'natural', preferredPhrases, avoidPhrases, qqFaceIds },
    profile: { methodology: '核验本人身份，仅用训练分区统计与助手逐条语义复核的例子；正文长度和问号按正文消息，表情按文字或原生表情消息计。连续本人发言按120秒回合、30分钟会话段切分；保留逐会话尾段作独立稳定性检查。',
      sample: { messageCount, textMessageCount: textCount, turnCount, conversationCount, fromDate, toDate, holdoutTurnCount },
      metrics: { textMedianChars: textMedian, textP90Chars: textP90, bubblesPerTurnMedian: bubbleMedian, bubblesPerTurnP90: bubbleP90,
        noTerminalPunctuationRate: round(1 - terminalRate), questionOnlyRate: round(questionOnlyRate), faceOnlyRate: round(faceOnlyRate), mixedFaceRate: round(faceRate - faceOnlyRate), faceCoverageRate: resolvedFaceCount ? round(supportedFaceCount / resolvedFaceCount) : 0 },
      patterns, demonstrations, limitations } };
  if (Buffer.byteLength(JSON.stringify(pack, null, 2) + '\n') > MAX_PACK_BYTES) fail('Compiled style pack exceeds 64 KiB');
  const audit = { schemaVersion: 1, compilerVersion: '1.0.0', sourceDigest: analysis.sourceDigest, analysisSha256: options.analysisSha256 || sha(JSON.stringify(analysis)), decisionSha256: options.decisionSha256 || sha(JSON.stringify(decisions)),
    partition: 'training only; held-out values reported without tuning', sample: pack.profile.sample,
    trainingTurns: training.size, rawHeldOutTurns: analysis.splits.rawHeldOutTurns, heldOutTurns: heldOut.size, excludedOverlapTurns: analysis.splits.contentOverlapRemovedTurns,
    conversationCountBasis: measuredConversationCount === undefined ? 'lower bound from training excerpt candidate conversations' : 'analyzer measured expressive training conversations',
    faceCoverage: { population: 'resolved native face IDs in training lexical evidence', supported: supportedFaceCount, total: resolvedFaceCount, zeroDenominator: resolvedFaceCount === 0 },
    preferredPhraseEvidence: preferredPhrases.map(phrase => repeated.get(phrase)), manualAvoidPhrases: avoidPhrases,
    patterns: patternAudit, demonstrations: demonstrationAudit,
    limitations: [...limitations, ...(analysis.quality?.limitations || []), 'Examples may place original source face IDs after text because the export candidate stores text and face lists separately; this does not claim original inline positions.'] };
  return { pack, audit };
}

function readJson(filename, maximum) {
  const stat = fs.lstatSync(filename);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > maximum) fail('Inputs must be bounded ordinary JSON files');
  const bytes = fs.readFileSync(filename);
  return { value: JSON.parse(bytes.toString('utf8')), digest: sha(bytes) };
}
export function writeNewOutputs(packFile, auditFile, result) {
  const output = path.resolve(packFile), audit = path.resolve(auditFile);
  if (output === audit || fs.existsSync(output) || fs.existsSync(audit)) fail('Output paths must be distinct new files');
  fs.mkdirSync(path.dirname(output), { recursive: true }); fs.mkdirSync(path.dirname(audit), { recursive: true });
  const packHandle = fs.openSync(output, 'wx');
  let auditHandle;
  try { auditHandle = fs.openSync(audit, 'wx'); fs.writeFileSync(packHandle, JSON.stringify(result.pack, null, 2) + '\n'); fs.writeFileSync(auditHandle, JSON.stringify(result.audit, null, 2) + '\n'); }
  finally { fs.closeSync(packHandle); if (auditHandle !== undefined) fs.closeSync(auditHandle); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = {}; for (let index = 2; index < process.argv.length; index += 2) { const key = process.argv[index]; if (!['--analysis', '--decisions', '--output', '--audit'].includes(key) || !process.argv[index + 1] || key in args) fail('Expected unique --analysis --decisions --output --audit'); args[key] = process.argv[index + 1]; }
    if (Object.keys(args).length !== 4) fail('Usage: --analysis <private report> --decisions <reviewed choices> --output <new pack> --audit <new audit>');
    const analysis = readJson(args['--analysis'], 64 * 1024 * 1024), choices = readJson(args['--decisions'], 1024 * 1024);
    const result = compileStyleProfile(analysis.value, choices.value, { analysisSha256: analysis.digest, decisionSha256: choices.digest });
    writeNewOutputs(args['--output'], args['--audit'], result);
    console.log(JSON.stringify({ schemaVersion: 2, patterns: result.pack.profile.patterns.length, demonstrations: result.pack.profile.demonstrations.length, trainingMessages: result.pack.profile.sample.messageCount }));
  } catch (error) { console.error(error instanceof Error ? error.message : 'Style profile compilation failed'); process.exitCode = 1; }
}
