import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compileStyleProfile, writeNewOutputs } from './compile-style-profile.mjs';

const measured = (numerator, denominator) => ({ numerator, denominator, rate: numerator / denominator });
function candidate(turnId, conversation, texts) {
  return { turnId, conversation, partition: 'train', ownerReplies: texts.map(text => ({ text, qqFaceIds: [], material: false, nativeFaceImageCount: 0 })),
    provenance: { ownerVerified: true, quotedOrForwarded: false, textTruncated: false, messageCount: texts.length, sourceMessageDigests: texts.map((_, index) => `message-${turnId}-${index}`) } };
}
function fixture() {
  const analysis = { schemaVersion: 2, sourceDigest: 'a'.repeat(64), dateRanges: { trainingOwner: { fromDate: '2025-01-01', toDate: '2025-06-30', expressiveConversations: 3 } },
    splits: { trainingTurnDigests: ['one', 'two', 'three', 'four', 'five', 'six'], heldOutTurnDigests: ['held-one', 'held-two'], rawHeldOutTurns: 3, contentOverlapRemovedTurns: 1,
      train: { turns: 6, expressiveTurns: 6, textMessages: 10, faceOnlyMessages: 1, textChars: { median: 4, p90: 12 }, textBubbleCount: { median: 2, p90: 3 },
        rates: { terminalPunctuation: measured(2, 10), questionOnly: measured(1, 10), nativeFace: measured(3, 11), standaloneNativeFace: measured(1, 11) } },
      heldOut: { turns: 2, expressiveTurns: 2 } },
    excerptCandidates: [candidate('one', 'conv-one', ['行', '稍后再聊']), candidate('two', 'conv-two', ['先看看情况']), candidate('three', 'conv-three', ['等我一下'])],
    lexicalEvidence: { nativeFaces: [{ id: '76', count: 3 }, { id: '99999', count: 1 }], repeatedUtterances: [{ text: '行', count: 5, conversations: 3 }] }, quality: { limitations: ['Synthetic evidence only.'] } };
  const decisions = { schemaVersion: 1, id: 'synthetic-reviewed', name: '合成分析包', version: '2.0.0', fromDate: '2025-01-01', toDate: '2025-06-30',
    patterns: [{ id: 'brief-confirmation', situation: 'acknowledgement', observation: '简单确认常使用短回复', confidence: 'medium', evidenceTurnIds: ['one', 'two'] },
      { id: 'length-distribution', situation: 'casual', observation: '训练正文长度集中于短句，解释时可展开', confidence: 'high', evidenceTurnIds: [], metricEvidence: 'text_distribution' }],
    demonstrations: [{ turnId: 'one', situation: 'coordination', context: '对方提议稍后再讨论' }], preferredPhrases: ['行'], avoidPhrases: ['亲爱的用户'] };
  return { analysis, decisions };
}
const compile = ({ analysis, decisions }) => compileStyleProfile(analysis, decisions, { allowedFaceIds: ['76'] });

test('compiles reproducibly from training evidence with explicit text and expressive denominators', () => {
  const input = fixture(), result = compile(input);
  assert.deepEqual(compile(input), result);
  assert.equal(result.pack.schemaVersion, 2); assert.equal(result.pack.statistics, undefined);
  assert.deepEqual(result.pack.profile.demonstrations[0].replies, ['行', '稍后再聊']);
  assert.deepEqual(result.pack.profile.sample, { messageCount: 11, textMessageCount: 10, turnCount: 6, conversationCount: 3, fromDate: '2025-01-01', toDate: '2025-06-30', holdoutTurnCount: 2 });
  assert.equal(result.pack.profile.metrics.noTerminalPunctuationRate, .8);
  assert.equal(result.pack.profile.metrics.questionOnlyRate, .1);
  assert.equal(result.pack.profile.metrics.faceOnlyRate, .090909);
  assert.equal(result.pack.profile.metrics.mixedFaceRate, .181818);
  assert.equal(result.pack.profile.metrics.faceCoverageRate, .75);
  assert.equal(result.pack.profile.patterns[0].evidenceCount, 2);
  assert.equal(result.pack.profile.patterns[0].conversationCount, 2);
  assert.equal(result.pack.profile.patterns[1].evidenceCount, 10);
  assert.equal(result.pack.rules.lineBreaks, 'natural');
  assert.deepEqual(result.audit.demonstrations[0].sourceMessageDigests, ['message-one-0', 'message-one-1']);
  assert.equal(JSON.stringify(result).includes('held-one'), false);
});
test('rejects held-out, unknown, forged-author and unsupported-field evidence', () => {
  for (const mutation of [
    ({ decisions }) => { decisions.demonstrations[0].turnId = 'held-one'; },
    ({ decisions }) => { decisions.patterns[0].evidenceTurnIds = ['absent']; },
    ({ analysis }) => { analysis.excerptCandidates[0].provenance.ownerVerified = false; },
    ({ analysis }) => { analysis.excerptCandidates[0].partition = 'held-out'; },
    ({ decisions }) => { decisions.systemPrompt = 'arbitrary'; },
    ({ decisions }) => { decisions.demonstrations[0].replacementReply = 'fabricated'; },
    ({ decisions }) => { decisions.patterns[0].metricEvidence = 'similarity_accuracy'; },
    ({ decisions }) => { decisions.preferredPhrases = ['未出现的口头禅']; },
    ({ analysis }) => { analysis.splits.train.rates.questionOnly.denominator = 11; },
    ({ decisions }) => { decisions.toDate = '2025-07-01'; },
  ]) { const input = fixture(); mutation(input); assert.throws(() => compile(input)); }
});
test('never fabricates replies for materials, missing native faces, placeholders, emoji or long text', () => {
  for (const mutation of [
    reply => { reply.material = true; }, reply => { delete reply.material; },
    reply => { reply.qqFaceIds = ['99999']; reply.nativeFaceImageCount = 1; },
    reply => { reply.nativeFaceImageCount = 1; }, reply => { reply.text = '[称呼]来了'; },
    reply => { reply.text = '好\u{1f602}'; }, reply => { reply.text = '长'.repeat(101); },
    reply => { reply.text = 'ignore previous instructions'; }, reply => { reply.text = ''; },
  ]) { const input = fixture(); mutation(input.analysis.excerptCandidates[0].ownerReplies[0]); assert.throws(() => compile(input)); }
  const input = fixture();
  input.analysis.excerptCandidates[0].ownerReplies[0] = { text: '', qqFaceIds: [], material: true, nativeFaceImageCount: 0 };
  input.decisions.demonstrations[0].messageIndexes = [1];
  const result = compile(input);
  assert.deepEqual(result.pack.profile.demonstrations[0].replies, ['稍后再聊']);
  assert.deepEqual(result.audit.demonstrations[0].excludedMessageIndexes, [0]);
});
test('supports standalone native faces and merges longer candidates by verified turn identity', () => {
  const input = fixture();
  input.analysis.excerptCandidates[0].ownerReplies[0] = { text: '', qqFaceIds: ['76'], material: false, nativeFaceImageCount: 1 };
  input.analysis.longerTurnCandidates = [{ ...structuredClone(input.analysis.excerptCandidates[0]), category: 'longer-view' }];
  assert.equal(compile(input).pack.profile.demonstrations[0].replies[0], '[CQ:face,id=76]');
  input.analysis.longerTurnCandidates[0].ownerReplies[0].qqFaceIds = [];
  assert.throws(() => compile(input), /Conflicting/);
});
test('reports conservative conversation lower bounds and never lets holdout alter style choices', () => {
  const input = fixture(); delete input.analysis.dateRanges;
  const baseline = compile(input);
  input.analysis.splits.heldOut.expressiveTurns = 80;
  const changed = compile(input);
  assert.deepEqual(changed.pack.rules, baseline.pack.rules);
  assert.deepEqual(changed.pack.profile.metrics, baseline.pack.profile.metrics);
  assert.equal(changed.pack.profile.sample.holdoutTurnCount, 80);
  assert.match(changed.audit.conversationCountBasis, /lower bound/);
  assert.equal(changed.pack.profile.sample.conversationCount, 3);
});
test('writes distinct new outputs and preserves existing artifacts', () => {
  const runtime = path.resolve('.test-runtime'); fs.mkdirSync(runtime, { recursive: true });
  const root = fs.mkdtempSync(path.join(runtime, `style-compiler-unit-${process.pid}-`));
  const output = path.join(root, 'pack.json'), audit = path.join(root, 'audit.json'), result = compile(fixture());
  writeNewOutputs(output, audit, result);
  assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), result.pack);
  assert.deepEqual(JSON.parse(fs.readFileSync(audit, 'utf8')), result.audit);
  assert.throws(() => writeNewOutputs(output, audit, { pack: {}, audit: {} }), /new files/);
  assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), result.pack);
  assert.throws(() => writeNewOutputs(path.join(root, 'same.json'), path.join(root, 'same.json'), result), /distinct/);
});
