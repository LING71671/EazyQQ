import { test } from 'node:test';
import assert from 'node:assert/strict';
import { analyzeExports, eligibleMessage, jsonLiteral } from './import-chat-style.mjs';

function exported(peer, messages) {
  const manifest = { format: 'qce-modern-html-chunked', version: 1, chat: { selfUin: '10001', selfUid: 'owner-uid', peerUid: peer }, senders: [] };
  return `<script>window.__QCE_MANIFEST__ && window.__QCE_MANIFEST__(${JSON.stringify(manifest)});</script>\n<script>window.__QCE_CHUNK__ && window.__QCE_CHUNK__({id:"c1",messages:${JSON.stringify(messages)}});</script>`;
}
const own = (id, extra = {}) => ({ id, uid: 'owner-uid', text: '好的', ts: 1000, html: '<div class="message self"><span class="text-content">好的</span></div>', ...extra });

test('attributes authors only through exact UIN mapping and filters quoted or generated content', () => {
  const result = analyzeExports([exported('peer-one', [own('a'), own('b', { uid: 'peer-uid', name: 'Owner', isSelf: true }), own('c', { html: '<div class="reply-content">quoted</div>' }), own('d', { aiGenerated: true }), own('a')])], { ownerUin: '10001' });
  assert.equal(result.pack.statistics.messageCount, 1); assert.equal(result.audit.counters.duplicate, 1); assert.equal(result.audit.counters.excluded, 2);
  const serialized = JSON.stringify(result.pack); for (const forbidden of ['10001', 'peer-one', 'owner-uid', 'Owner', 'selfUin', 'sourcePath']) assert.equal(serialized.includes(forbidden), false);
});
test('extracts only reusable reactions across several conversations without contacts or transcript', () => {
  const sources = ['one', 'two', 'three'].map((peer, index) => exported(peer, Array.from({ length: 5 }, (_, n) => own(`${index}-${n}`, { ts: 1000 + n * 1000 }))));
  const result = analyzeExports(sources, { ownerUin: '10001' }); assert.deepEqual(result.pack.rules.preferredPhrases, ['好的']); assert.equal(result.pack.statistics.messageCount, 15); assert.equal(result.pack.statistics.multiMessageRate, .8);
  assert.equal(result.audit.activityCohorts.higherVolume.messageCount, 5); assert.equal(result.audit.activityCohorts.otherVolume.messageCount, 10);
});
test('never executes embedded JavaScript and handles bracket-looking data safely', () => {
  assert.deepEqual(jsonLiteral('[{"text":"]} \\\" quoted"}] trailing()', 0).value, [{ text: ']} " quoted' }]);
  assert.throws(() => jsonLiteral('[process.exit()]', 0), /valid JSON/);
  assert.equal(eligibleMessage(own('x', { html: '<div class="forward-card">forwarded text</div>' })), null);
});
test('native face preferences require a real face image and an allowed catalog ID', () => {
  const result = analyzeExports([exported('one', [own('face', { text: '', html: '<img class="face-emoji face-emoji-image" src="https://example.invalid/apng/76.png" alt="test">' })])], { ownerUin: '10001', allowedFaceIds: ['76'] });
  assert.deepEqual(result.pack.rules.qqFaceIds, ['76']); assert.equal(result.pack.statistics.faceRate, 1);
});

test('conflicting explicit UIN and mapped UID never attribute a message to the owner', () => {
  const result = analyzeExports([exported('peer-one', [own('valid'), own('bad-uin', { senderUin: '20002' }), own('bad-uid', { uid: 'peer-uid', senderUin: '10001' }), own('bad-fields', { senderUin: '10001', uin: '20002' })])], { ownerUin: '10001' });
  assert.equal(result.pack.statistics.messageCount, 1);
  assert.equal(result.audit.counters.identityConflict, 3);
});
