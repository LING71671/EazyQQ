import { test } from 'node:test';
import assert from 'node:assert/strict';
import { analyzeCorpus, assembleTurns, extractMessage, wilson } from './analyze-chat-style.mjs';

const BASE = Date.parse('2025-01-01T00:00:00Z');
function exported(peer, messages, extra = {}) {
  const manifest = { format: 'qce-modern-html-chunked', version: 1,
    chat: { type: 'private', selfUin: '10001', selfUid: 'owner-uid', peerUid: peer, ...extra },
    senders: [{ uid: 'owner-uid', displayName: 'Owner Name' }, { uid: peer, displayName: 'Peer Name' }] };
  return `<script>window.__QCE_MANIFEST__ && window.__QCE_MANIFEST__(${JSON.stringify(manifest)});</script><script>window.__QCE_CHUNK__ && window.__QCE_CHUNK__({messages:${JSON.stringify(messages)}});</script>`;
}
function own(id, text = 'Hello', ts = BASE, extra = {}) {
  return { id, uid: 'owner-uid', text, ts, html: `<div class="content"><span class="text-content">${text}</span></div>`, ...extra };
}
function peer(id, text = 'Question', ts = BASE, extra = {}) { return own(id, text, ts, { uid: 'peer-uid', ...extra }); }
function report(sources) { return analyzeCorpus(sources, { ownerUin: '10001' }); }

test('counts native face labels separately from original owner text and material placeholders', () => {
  const faceOnly = own('face', '/Test face', BASE, { html: '<span class="native-face-wrap"><img class="face-emoji" src="https://example.invalid/76/apng/76.png" alt="/Test face"></span>' });
  const mixed = own('mixed', 'Hello /Test face', BASE + 1000, { html: '<span class="text-content">Hi</span><img class="face-emoji" src="https://example.invalid/76/apng/76.png" alt="/Test face">' });
  const material = own('media', '[图片:private.jpg]', BASE + 2000, { html: '<div class="image-content">Private caption</div>' });
  const analysis = report([exported('peer-one', [faceOnly, mixed, material])]);
  assert.equal(extractMessage(faceOnly).text, '');
  assert.equal(extractMessage(mixed).text, 'Hi');
  assert.equal(analysis.corpus.metrics.textChars.median, 2);
  assert.equal(analysis.corpus.metrics.textMessages, 1);
  assert.equal(analysis.corpus.metrics.faceOnlyMessages, 1);
  assert.equal(analysis.corpus.metrics.materialOnlyMessages, 1);
  assert.equal(analysis.corpus.metrics.rates.nativeFace.denominator, 2);
});

test('attributes only verified sender identity and excludes conflicts, truncation, quote and forward whole', () => {
  const analysis = report([exported('peer-one', [own('valid'), own('nick', 'No', BASE + 1000, { uid: 'peer-uid', name: 'Owner Name', isSelf: true }),
    own('conflict', 'No', BASE + 2000, { senderUin: '20002' }), own('truncated', 'No', BASE + 3000, { textTruncated: true }),
    own('quote', 'No', BASE + 4000, { html: '<div class="reply-content-text">Other author</div><span class="text-content">Outer reply</span>' }),
    own('forward', 'No', BASE + 5000, { forwarded: true }), own('generated', 'No', BASE + 6000, { aiGenerated: true })])]);
  assert.equal(analysis.corpus.metrics.textMessages, 1);
  assert.equal(analysis.quality.counters.identityConflict, 1);
  assert.equal(analysis.quality.ownerExclusionReasons.truncated, 1);
  assert.equal(analysis.quality.ownerExclusionReasons.quoted, 1);
  assert.equal(analysis.quality.ownerExclusionReasons.forwardOrCard, 1);
  assert.equal(analysis.quality.ownerExclusionReasons.explicitAi, 1);
  assert.match(analysis.quality.limitations.join(' '), /underrepresent quoting habits/);
});

test('deduplicates by conversation and message ID, quarantines conflicting replays, and never collapses equal text', () => {
  const one = exported('peer-one', [own('same-id', 'One'), own('different-id', 'One', BASE + 1000)]);
  const two = exported('peer-two', [own('same-id', 'Two')]);
  const replay = exported('peer-one', [own('same-id', 'Changed')]);
  const analysis = report([one, two, replay]);
  assert.equal(analysis.quality.counters.duplicate, 1);
  assert.equal(analysis.quality.counters.conflictingDuplicate, 1);
  assert.equal(analysis.corpus.metrics.textMessages, 2);
  assert.equal(analysis.quality.counters.ownerIncluded, 2);
  assert.equal(analysis.quality.ownerExclusionReasons.conflictingDuplicate, 1);
  assert.equal(analysis.corpus.conversationsWithOwnerTurns, 2);
});

test('provenance-only or nested sender replay changes quarantine the original row', () => {
  for (const mutation of [{ uin: '20002' }, { sender: { uid: 'peer-uid' } }, { sender: { uin: '20002' } }, { senderUid: 'peer-uid' },
    { quoted: true }, { forwarded: true }, { aiGenerated: true }, { ai_reply_status: 'sent' }, { isSystem: true }]) {
    const original = exported('peer-one', [own('stable'), own('conflicted')]);
    const changed = exported('peer-one', [own('conflicted', 'Hello', BASE, mutation)]);
    const analysis = report([original, changed]);
    assert.equal(analysis.quality.counters.conflictingDuplicate, 1, JSON.stringify(mutation));
    assert.equal(analysis.quality.counters.ownerIncluded, 1);
    assert.equal(analysis.quality.ownerExclusionReasons.conflictingDuplicate, 1);
    assert.equal(analysis.corpus.metrics.textMessages, 1);
  }
});

test('measures question-only messages separately from questions containing words', () => {
  const analysis = report([exported('peer-one', [own('only', '？？'), own('words', 'A question?'), own('plain', 'Fine')])]);
  assert.equal(analysis.corpus.metrics.rates.question.numerator, 2);
  assert.equal(analysis.corpus.metrics.rates.questionOnly.numerator, 1);
  assert.equal(analysis.corpus.metrics.rates.questionOnly.denominator, 3);
});

test('separates unresolved native images and numeric-ID catalog coverage without inventing text', () => {
  const rows = [own('supported', '/One', BASE, { html: '<img class="face-emoji" src="https://example.invalid/apng/76.png" alt="/One">' }),
    own('unsupported', '/Two', BASE + 1000, { html: '<img class="face-emoji" src="https://example.invalid/apng/99.png" alt="/Two">' }),
    own('unresolved', '/Three', BASE + 2000, { html: '<img class="face-emoji" src="https://example.invalid/unknown.webp" alt="/Three">' })];
  const analysis = analyzeCorpus([exported('peer-one', rows)], { ownerUin: '10001', allowedFaceIds: ['76'] });
  assert.equal(analysis.corpus.metrics.textMessages, 0);
  assert.equal(analysis.corpus.metrics.faceOnlyMessages, 3);
  assert.equal(analysis.nativeFaceCoverage.nativeFaceImages, 3);
  assert.equal(analysis.nativeFaceCoverage.resolvedNumericIds, 2);
  assert.equal(analysis.nativeFaceCoverage.resolvedIdsSupportedByCatalog, 1);
  assert.equal(analysis.nativeFaceCoverage.sourceResolution.denominator, 3);
  assert.equal(analysis.nativeFaceCoverage.catalogCoverageAmongResolved.denominator, 2);
  assert.deepEqual(analysis.nativeFaceCoverage.unmatchedNumericIds, ['99']);
});

test('does not count a text-plus-material turn as multiple expressive bubbles or short text burst', () => {
  const analysis = report([exported('peer-one', [own('text', 'One'), own('material', '[图片:x.jpg]', BASE + 1000, { html: '<div class="image-content"></div>' })])]);
  assert.equal(analysis.corpus.metrics.rates.multipleBubbles.numerator, 0);
  assert.equal(analysis.corpus.metrics.rates.shortBurst.numerator, 0);
  assert.equal(analysis.corpus.metrics.interBubbleSeconds.n, 0);
});

test('sorts timestamps and computes owner turns across peer switches, gaps and short bursts', () => {
  const analysis = report([exported('peer-one', [own('three', 'Third', BASE + 50_000), own('one', 'First', BASE), own('two', 'Second', BASE + 20_000),
    peer('peer', 'Peer', BASE + 60_000), own('four', 'Fourth', BASE + 70_000), own('five', 'Fifth', BASE + 200_001)])]);
  assert.equal(analysis.corpus.metrics.turns, 3);
  assert.equal(analysis.corpus.metrics.textBubbleCount.p90, 1);
  assert.equal(analysis.corpus.metrics.textBubbleCount.mean, 1.666667);
  assert.equal(analysis.corpus.metrics.rates.multipleBubbles.numerator, 1);
  assert.equal(analysis.corpus.metrics.rates.shortBurst.numerator, 1);
  assert.equal(analysis.corpus.metrics.rates.newline.numerator, 0);
  const rows = [own('a', 'One'), own('b', 'Two', BASE + 35_000)].map(message => ({ ...message, role: 'owner', faces: [], kind: 'text' }));
  const turns = assembleTurns(rows, 'scope');
  assert.equal(turns.length, 1);
  assert.deepEqual(turns[0].burstGroups, [1, 1]);
});

test('holds out complete latest sessions per conversation and removes train-content overlap', () => {
  const rows = [];
  for (let index = 0; index < 10; index++) {
    const ts = BASE + index * 86400000;
    rows.push(peer(`p-${index}`, 'Question', ts), own(`o-${index}`, index === 9 ? 'Answer zero' : `Answer ${index === 0 ? 'zero' : index}`, ts + 1000));
  }
  const analysis = report([exported('peer-one', rows)]);
  assert.equal(analysis.splits.trainingTurns, 8);
  assert.equal(analysis.splits.rawHeldOutTurns, 2);
  assert.equal(analysis.splits.contentOverlapRemovedTurns, 1);
  assert.equal(analysis.splits.heldOutTurns, 1);
  assert.equal(analysis.splits.exactMessageContentOverlapAfterRemoval, 0);
  assert.equal(analysis.excerptCandidates.every(example => example.partition === 'train'), true);
  const train = new Set(analysis.splits.trainingTurnDigests);
  assert.equal(analysis.excerptCandidates.every(example => train.has(example.turnId)), true);
  assert.equal(analysis.splits.heldOutTurnDigests.some(id => train.has(id)), false);
});

test('does not split a single long session or infer bots from names', () => {
  const rows = Array.from({ length: 6 }, (_, index) => [peer(`p-${index}`, 'Question', BASE + index * 60_000), own(`o-${index}`, `Answer ${index}`, BASE + index * 60_000 + 1000)]).flat();
  const analysis = report([exported('bot-looking-name', rows, { name: 'An assistant bot' }), exported('excluded-bot', rows, { isBot: true }), exported('owner-uid', rows), exported('device', rows, { peerUin: '10001' })]);
  assert.equal(analysis.quality.counters.explicitBotFiles, 1);
  assert.equal(analysis.quality.counters.selfConversationFiles, 2);
  assert.equal(analysis.corpus.conversations, 1);
  assert.equal(analysis.splits.heldOutTurns, 0);
  assert.equal(analysis.splits.noHoldoutConversations, 1);
});

test('reports concentration, macro statistics and uncertainty without claiming model resemblance', () => {
  const analysis = report([exported('peer-one', [own('one', 'Hi')]), exported('peer-two', Array.from({ length: 9 }, (_, index) => own(String(index), 'A longer example sentence', BASE + index * 1000)))]);
  assert.equal(analysis.concentration.topConversations[0].share, .9);
  assert.equal(analysis.corpus.metrics.rates.textAtMost5.rate, .1);
  assert.equal(analysis.concentration.macroRates.textAtMost5.mean, .5);
  assert.equal(analysis.concentration.sensitivity[0].metrics.textMessages, 1);
  assert.equal(analysis.concentration.clusteredRateIntervals.textAtMost5.ci95.length, 2);
  const interval = wilson(5, 10); assert.equal(interval.rate, .5); assert.ok(interval.ci95[0] < .5 && interval.ci95[1] > .5);
  assert.equal(wilson(0, 0).ci95, null);
});

test('candidate excerpts redact identifiers, names and paths while preserving complete short replies', () => {
  const analysis = report([exported('private-peer', [peer('p', 'Peer Name mail person@example.invalid https://example.invalid/secret', BASE),
    own('o', 'Owner Name call 12345678901', BASE + 1000), own('o2', 'Then open C:/private/file.txt', BASE + 2000)])]);
  const serialized = JSON.stringify(analysis);
  for (const privateValue of ['10001', 'owner-uid', 'private-peer', 'Owner Name', 'Peer Name', '12345678901', 'person@example.invalid', 'https://example.invalid/secret', 'C:/private/file.txt']) assert.equal(serialized.includes(privateValue), false, privateValue);
  assert.equal(analysis.excerptCandidates[0].ownerReplies.length, 2);
  assert.equal(analysis.excerptCandidates[0].ownerReplies[0].material, false);
  assert.equal(analysis.excerptCandidates[0].ownerReplies[0].nativeFaceImageCount, 0);
  assert.match(analysis.excerptCandidates[0].ownerReplies[0].text, /\[称呼\].*\[号码\]/u);
});

test('candidate provenance retains material and unresolved native-image evidence for manual selection', () => {
  const analysis = report([exported('peer-one', [peer('p', 'Question'), own('text', 'A reply', BASE + 1000),
    own('file', '[文件:test.txt]', BASE + 2000, { html: '<div class="file-bubble">test.txt</div>' }),
    own('unknown-face', '/Unresolved face', BASE + 3000, { html: '<img class="face-emoji" src="https://example.invalid/unknown.webp">' })])]);
  const replies = analysis.excerptCandidates[0].ownerReplies;
  assert.equal(replies[1].material, true);
  assert.equal(replies[2].nativeFaceImageCount, 1);
  assert.deepEqual(replies[2].qqFaceIds, []);
});

test('longer candidates remain training-only complete turns with original bubble boundaries', () => {
  const analysis = report([exported('peer-one', [peer('p', 'Question'), own('one', '因为 this is an explanation with enough original text', BASE + 1000), own('two', '然后 a separate second bubble', BASE + 2000)])]);
  assert.equal(analysis.longerTurnCandidates.length, 1);
  assert.equal(analysis.longerTurnCandidates[0].partition, 'train');
  assert.equal(analysis.longerTurnCandidates[0].ownerReplies.length, 2);
});

test('rejects unsupported owner identity and seconds timestamps instead of guessing', () => {
  assert.throws(() => analyzeCorpus([], { ownerUin: 'bad' }), /explicit valid owner/);
  const analysis = report([exported('peer-one', [own('ok'), own('seconds', 'Bad', BASE / 1000)])]);
  assert.equal(analysis.quality.counters.invalidTimestamp, 1);
  assert.equal(analysis.corpus.metrics.textMessages, 1);
});

test('audits explicit template literals without classifying normal greetings as generated content', () => {
  const sources = [exported('peer-one', [own('greeting', 'Hello friend'), own('test', 'Test marker one', BASE + 1000), own('actual', 'An actual response', BASE + 2000)]),
    exported('peer-two', [own('test', 'Test marker two', BASE + 1000)])];
  const analysis = analyzeCorpus(sources, { ownerUin: '10001', templateRules: [{ id: 'test-marker', value: 'Test marker', match: 'prefix', exclude: true }, { id: 'greeting', value: 'Hello', match: 'prefix', exclude: false }] });
  assert.equal(analysis.corpus.metrics.textMessages, 4);
  assert.equal(analysis.templateSensitivity.excludedOwnerMessages, 2);
  assert.equal(analysis.templateSensitivity.afterExplicitExclusions.textMessages, 2);
  assert.equal(analysis.templateSensitivity.candidateRules[0].conversations, 2);
  assert.equal(analysis.templateSensitivity.candidateRules[1].ownerMessages, 1);
  assert.equal(analysis.dateRanges.allEligibleOwner.timezone, 'UTC+08:00');
  assert.equal(analysis.dateRanges.allEligibleOwner.fromDate, '2025-01-01');
  assert.equal(analysis.dateRanges.trainingOwner.expressiveConversations, 2);
  assert.throws(() => analyzeCorpus(sources, { ownerUin: '10001', templateRules: [{ id: 'bad', value: '', match: 'regex', exclude: true }] }), /bounded explicit literal/);
});
