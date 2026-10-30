"""Verify account-bound conversation context through offline native fixtures only."""
import base64
import copy
import hashlib
from contextlib import contextmanager
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os
import sqlite3
import threading
import time


PACK_IDS = ('native-context-short', 'native-context-warm')
TARGET = '995001'
CURRENT_ID = f'msg_{TARGET}_context-current'
_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a/2kAAAAASUVORK5CYII='


def _wait(client, expression, message, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if client.evaluate(expression):
                return
        except (AssertionError, OSError):
            pass
        time.sleep(.1)
    raise AssertionError(message)


def _invoke(client, command, arguments=None):
    result = client.evaluate(f"window.__TAURI_INTERNALS__.invoke({json.dumps(command)}, {json.dumps(arguments or {})})")
    assert result and result.get('success'), f'{command} failed: {result}'
    return result.get('data')


def _root(root):
    root = Path(root).resolve(strict=True)
    parent = next((path for path in root.parents if path.name == '.test-runtime'), None)
    assert parent and root.is_relative_to(parent.resolve(strict=True)), 'Use an isolated native fixture directory'
    return root


def _database(root, account):
    matches = list((root / 'EazyQQ_Data/accounts').glob(f'*/{account}/eazyqq.db'))
    assert len(matches) == 1, f'Expected one fixture database for {account}: {matches}'
    database = matches[0].resolve(strict=True)
    assert database.is_relative_to(root), 'The fixture database escaped its root'
    return database


@contextmanager
def _connect(database):
    connection = sqlite3.connect(database.as_uri() + '?mode=rw', uri=True, timeout=5)
    try:
        for statement in ['PRAGMA journal_mode=WAL', 'PRAGMA synchronous=NORMAL', 'PRAGMA foreign_keys=ON', 'PRAGMA cache_size=-64000', 'PRAGMA busy_timeout=5000']:
            connection.execute(statement)
        with connection:
            yield connection
    finally:
        connection.close()


def _pack(pack_id, name, tone):
    return {'schemaVersion': 1, 'id': pack_id, 'name': name, 'version': '1.0.0', 'rules': {
        'sentenceLength': 'short', 'punctuation': 'standard', 'tone': tone,
        'lineBreaks': 'natural', 'preferredPhrases': [], 'avoidPhrases': [], 'qqFaceIds': ['76'],
    }}


def _profile_pack():
    pack = _pack(PACK_IDS[1], '合成证据画像', 'warm')
    pack['schemaVersion'] = 2
    pack['profile'] = {
        'methodology': '合成回合证据与时间留出，仅验证数据契约。',
        'sample': {'messageCount': 120, 'textMessageCount': 100, 'turnCount': 80, 'conversationCount': 6,
                   'fromDate': '2026-09-01', 'toDate': '2026-09-28', 'holdoutTurnCount': 20},
        'metrics': {'textMedianChars': 7, 'textP90Chars': 24, 'bubblesPerTurnMedian': 1,
                    'bubblesPerTurnP90': 3, 'noTerminalPunctuationRate': .8, 'questionOnlyRate': .03,
                    'faceOnlyRate': .1, 'mixedFaceRate': .05, 'faceCoverageRate': 1},
        'patterns': [{'id': 'fixture-repair', 'situation': 'repair', 'observation': '先确认具体差异，再补充一句。',
                      'evidenceCount': 24, 'conversationCount': 4, 'confidence': 'high'}],
        'demonstrations': [{'situation': 'repair', 'context': '对方指出遗漏', 'replies': ['这条漏了', '我补一下'],
                            'evidenceRefs': ['fixture-turn-repair']}],
        'limitations': ['合成数据只验证导入和提示词消费，不代表私人风格准确度。'],
    }
    return pack


def _open_settings(client):
    client.evaluate("document.querySelector('button[aria-label=\"关闭会话\"]')?.click();[...document.querySelectorAll('button')].find(b=>b.textContent==='系统设置').click();true")
    _wait(client, "!!document.querySelector('#style-pack-heading')", 'The real style pack card did not render')


def _import_pack(client, pack):
    # A real File exercises the production File.text and IPC import path.
    client.evaluate(f"""(() => {{
      const input=document.querySelector('input[aria-label="风格包JSON文件"]');
      const transfer=new DataTransfer();
      transfer.items.add(new File([{json.dumps(json.dumps(pack, ensure_ascii=False))}], 'fixture.qqstyle.json', {{type:'application/json'}}));
      input.files=transfer.files;input.dispatchEvent(new Event('change',{{bubbles:true}}));return true;
    }})()""")
    _wait(client, f"document.querySelector('#style-pack-heading')?.closest('section').querySelector('select')?.value==={json.dumps(pack['id'])} && document.querySelector('#style-pack-heading')?.closest('section').getAttribute('aria-busy')==='false'", 'The style pack import did not finish')


def _validate_provider_presets(client, root):
    original = _invoke(client, 'get_config')['ai']
    labels = ['DeepSeek 官方', 'NVIDIA 开发试用', 'AMD 开发试用', '本机 OpenCode']
    order = client.evaluate(f"[...document.querySelectorAll('button')].filter(button=>{json.dumps(labels, ensure_ascii=False)}.some(label=>button.textContent.includes(label))).map(button=>button.textContent.trim())")
    assert len(order) == 4 and labels[0] in order[0] and labels[-1] in order[-1], order
    for label, endpoint, model in [
        ('DeepSeek 官方', 'https://api.deepseek.com', 'deepseek-flash'),
        ('AMD 开发试用', 'https://developer.amd.com.cn/radeon/api/v1', 'DeepSeek-V4.1-Flash'),
        ('NVIDIA 开发试用', 'https://integrate.api.nvidia.com/v1', 'deepseek-ai/deepseek-v4-pro'),
    ]:
        client.evaluate(f"[...document.querySelectorAll('button')].find(button=>button.textContent.includes({json.dumps(label, ensure_ascii=False)})).click();true")
        _wait(client, f"document.querySelector('#ai-provider-url')?.value==={json.dumps(endpoint)}", f'{label} endpoint did not render')
        assert client.evaluate("document.querySelector('#ai-provider-key').value") == '', 'A provider inherited another key'
        assert client.evaluate("document.querySelector('#ai-provider-model').value") == model, label
    assert _invoke(client, 'get_config')['ai'] == original, 'Inspecting presets silently saved an AI configuration'
    client.evaluate("document.querySelector('#ai-provider-url').closest('div.space-y-3').scrollIntoView({block:'center'});true")
    client.screenshot(root / 'provider-presets.png')
    client.evaluate("[...document.querySelectorAll('button')].find(button=>button.textContent.includes('本机 OpenCode')).click();true")
    _wait(client, "document.querySelector('#ai-provider-url')?.value==='https://opencode.ai/zen/v1'", 'The existing provider draft was not restored')
    return {'order': order, 'officialAndTrialEndpointsRendered': True, 'keysNotCopied': True, 'selectionDidNotSave': True}


def _seed_messages(database, account):
    account_dir = database.parent
    material_dir = account_dir / 'message-materials'
    material_dir.mkdir(exist_ok=True)
    image = material_dir / 'native-context-fixture.png'
    assert image.resolve().is_relative_to(account_dir.resolve()), 'The fixture material escaped its account'
    image.write_bytes(base64.b64decode(_PNG))
    scope = {'accountUin': account, 'targetType': 'group', 'targetId': TARGET}
    now = int(time.time() * 1000)
    fixtures = [
        ('context-owner', account, '本人', 'owner context fixture', False),
        ('context-face', '30001', '同名成员', '[CQ:face,id=76]', False),
        ('context-mixed', '30002', '同名成员', '混排前[CQ:face,id=76]混排后[CQ:face,id=999999]', False),
        ('context-current', '30002', '改名后的成员', '请根据这张图回复', False),
    ]
    ids = []
    with _connect(database) as connection:
        for index, (platform_id, sender, name, text, is_me) in enumerate(fixtures):
            msg_id = f'msg_{TARGET}_{platform_id}'
            ids.append(msg_id)
            timestamp = now - 4000 + index * 1000
            segments = [{'type': 'text', 'data': {'text': text}}]
            if platform_id == 'context-current':
                segments.append({'type': 'image', 'data': {'file': 'fixture-image-id'}})
            raw = {'message_id': platform_id, 'message_type': 'group', 'group_id': TARGET,
                   'user_id': sender, 'sender': {'user_id': sender, 'nickname': name}, 'message': segments, 'time': timestamp}
            source = {'schemaVersion': 1, 'scope': scope, 'origin': 'native_fixture', 'message': raw}
            if platform_id == 'context-current':
                # Only the program-owned sidecar contains a trusted local material marker.
                source['resolvedSegments'] = [segments[0], {'type': 'image', 'data': {
                    'path': str(image), 'resolution': 'resolved_by_bound_onebot',
                }}]
            connection.execute('INSERT INTO messages_log(msg_id,target_id,sender_id,sender_name,content,is_from_me,ai_reply_status,timestamp) VALUES(?,?,?,?,?,?,?,?)',
                               (msg_id, TARGET, sender, name, text, int(is_me), 'none', timestamp))
            connection.execute('INSERT INTO message_sources(msg_id,source_json) VALUES(?,?)', (msg_id, json.dumps(source, ensure_ascii=False)))
        # Cached target IDs alone cannot attest whether an old row came from this group.
        legacy_id = f'msg_{TARGET}_context-legacy-private'
        ids.append(legacy_id)
        connection.execute('INSERT INTO messages_log(msg_id,target_id,sender_id,sender_name,content,is_from_me,ai_reply_status,timestamp) VALUES(?,?,?,?,?,?,?,?)',
                           (legacy_id, TARGET, '30009', '未知旧行', 'LEGACY_SCOPE_MUST_NOT_LEAK', 0, 'none', now - 5000))
    return ids, image


class _Provider(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        assert self.client_address[0] == '127.0.0.1', 'Only loopback fixture traffic is allowed'
        length = int(self.headers.get('Content-Length', '0'))
        assert 0 < length <= 4_000_000, 'Fixture request exceeded its bound'
        payload = json.loads(self.rfile.read(length))
        self.server.requests.append(payload)
        body = json.dumps({'id': 'native-context-fixture', 'object': 'chat.completion', 'choices': [
            {'index': 0, 'message': {'role': 'assistant', 'content': '离线夹具回复'}, 'finish_reason': 'stop'},
        ]}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def _capture_request(client, database, account):
    original = _invoke(client, 'get_config')
    server = ThreadingHTTPServer(('127.0.0.1', 0), _Provider)
    server.requests = []
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        fixture = copy.deepcopy(original)
        fixture['ai'] = {**fixture.get('ai', {}), 'activeProvider': 'openai', 'baseUrl': f'http://127.0.0.1:{server.server_port}/v1', 'model': 'native-context-fixture', 'apiKey': 'native-fixture-key', 'maxContextMessages': 10}
        _invoke(client, 'update_config', {'config': fixture})
        reply = _invoke(client, 'trigger_ai_reply', {'targetId': TARGET, 'targetType': 'group', 'sourceMessageId': CURRENT_ID, 'contextSnippet': 'THIS_FALLBACK_MUST_NOT_REPLACE_SOURCE'})
        assert reply == '离线夹具回复', reply
        assert len(server.requests) == 1, server.requests
        request = server.requests[0]
        messages = request['messages']
        text_parts = []
        images = []
        envelopes = []
        for message in messages:
            content = message.get('content', '')
            for part in content if isinstance(content, list) else [{'type': 'text', 'text': content}]:
                if part.get('type') == 'image_url':
                    images.append(part['image_url']['url'])
                if part.get('type') == 'text':
                    text = part.get('text', '')
                    text_parts.append(text)
                    try:
                        parsed = json.loads(text)
                        if isinstance(parsed, dict) and parsed.get('messageId'):
                            envelopes.append((message['role'], parsed))
                    except json.JSONDecodeError:
                        pass
        joined = '\n'.join(text_parts)
        assert 'fixture-repair' in joined and 'fixture-turn-repair' in joined, 'The selected V2 profile was not consumed'
        current = [(role, value) for role, value in envelopes if value['messageId'] == CURRENT_ID]
        assert len(current) == 1 and current[0][0] == 'user', envelopes
        assert current[0][1]['senderUin'] == '30002' and current[0][1]['scope'] == {'accountUin': account, 'targetType': 'group', 'targetId': TARGET}, current
        owners = [(role, value) for role, value in envelopes if value.get('senderUin') == account]
        assert owners and all(role == 'assistant' and value['relation'] == 'account_owner' for role, value in owners), owners
        assert 'LEGACY_SCOPE_MUST_NOT_LEAK' not in joined and 'THIS_FALLBACK_MUST_NOT_REPLACE_SOURCE' not in joined, joined
        assert len(images) == 1 and images[0].startswith('data:image/png;base64,'), images
        return {'capturedRequests': 1, 'currentSourceCount': 1, 'ownerRole': 'assistant', 'groupMemberUin': '30002', 'unknownLegacyScopeExcluded': True, 'imageParts': len(images), 'v2ProfileConsumed': True, 'remoteProviderContacted': False}
    finally:
        _invoke(client, 'update_config', {'config': original})
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def _validate_native_animation(client, root, database):
    with _connect(database) as connection:
        for suffix, text, segments in [
            ('context-face', '[CQ:face,id=324]', [{'type': 'face', 'data': {'id': '324'}}]),
            ('context-mixed', '混排前[CQ:face,id=427]混排后', [
                {'type': 'text', 'data': {'text': '混排前'}}, {'type': 'face', 'data': {'id': '427'}},
                {'type': 'text', 'data': {'text': '混排后'}}]),
        ]:
            message_id = f'msg_{TARGET}_{suffix}'
            source = json.loads(connection.execute('SELECT source_json FROM message_sources WHERE msg_id=?', (message_id,)).fetchone()[0])
            source['message']['message'] = segments
            connection.execute('UPDATE messages_log SET content=? WHERE msg_id=?', (text, message_id))
            connection.execute('UPDATE message_sources SET source_json=? WHERE msg_id=?', (json.dumps(source, ensure_ascii=False), message_id))
    client.command('Network.setBlockedURLs', {'urls': []})
    client.evaluate("document.querySelector('button[aria-label=\"关闭会话\"]').click();[...document.querySelectorAll('button')].find(b=>b.textContent==='群聊会话').click();true")
    evidence = {'realTauri': True, 'externalArtworkNetworkAllowed': True, 'realQQMessageSent': False, 'faces': {}}
    for face_id, name in [('324', '吃糖'), ('427', '偷感')]:
        selector = f'img[title="QQ原生表情：{name}"]'
        client.evaluate(f"document.querySelector({json.dumps(selector)})?.scrollIntoView({{block:'center'}});true")
        _wait(client, f"(()=>{{const image=document.querySelector({json.dumps(selector)});image?.scrollIntoView({{block:'center'}});return !!image?.complete && image.naturalWidth>0;}})()", f'The actual {name} APNG did not decode', timeout=30)
        state = client.evaluate(f"(()=>{{const image=document.querySelector({json.dumps(selector)});const r=image.getBoundingClientRect();return {{url:image.currentSrc,width:image.naturalWidth,height:image.naturalHeight,clip:{{x:r.x,y:r.y,width:r.width,height:r.height,scale:1}}}};}})()")
        assert state['url'].startswith('https://raw.githubusercontent.com/koishijs/QFace/c96fa14ff894147ac0442dda0fce6a4523e5fcbd/'), state
        frames = {}
        for index in range(18):
            screenshot = client.command('Page.captureScreenshot', {'format': 'png', 'clip': state['clip'], 'captureBeyondViewport': False})
            data = base64.b64decode(screenshot['data'])
            digest = hashlib.sha256(data).hexdigest()
            if digest not in frames:
                filename = f'native-animation-{face_id}-frame-{len(frames)}.png'
                (root / filename).write_bytes(data)
                frames[digest] = filename
            time.sleep([.17, .29, .23][index % 3])
        assert len(frames) > 1, f'{name} decoded but its rendered frames stayed static: {state}'
        evidence['faces'][face_id] = {**state, 'distinctRenderedFrames': len(frames), 'frameFiles': list(frames.values())}
    client.screenshot(root / 'native-animation-full.png')
    (root / 'native-animation-render.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    return evidence


def validate_conversation_context(client, root, servers, account='10001'):
    """Run before the primary account switch; return evidence for the switch checks."""
    root = _root(root)
    database = _database(root, account)
    assert servers and all(str(server.uin) in ('10001', '10002', '10003') for server in servers), 'Only synthetic protocol servers are allowed'
    sent_offsets = [len(server.sent_messages) for server in servers]
    original = _invoke(client, 'manage_style_packs', {'operation': 'list'})
    assert not original['packs'], 'Use a fresh synthetic account for native style validation'
    evidence = {'usesRealTauri': True, 'realQQTouched': False, 'fixtureAccount': account}
    ids = []
    image = None
    try:
        _open_settings(client)
        evidence['providers'] = _validate_provider_presets(client, root)
        _import_pack(client, _pack(PACK_IDS[0], '合成短句风格', 'direct'))
        _import_pack(client, _profile_pack())
        client.evaluate(f"""(() => {{const input=document.querySelector('#style-pack-heading').closest('section').querySelector('select');input.value={json.dumps(PACK_IDS[0])};input.dispatchEvent(new Event('change',{{bubbles:true}}));return true;}})()""")
        _wait(client, f"document.querySelector('#style-pack-heading')?.closest('section').querySelector('select')?.value==={json.dumps(PACK_IDS[0])} && document.querySelector('#style-pack-heading')?.closest('section').getAttribute('aria-busy')==='false'", 'The selected style did not update')
        assert _invoke(client, 'manage_style_packs', {'operation': 'list'})['activeId'] == PACK_IDS[0]
        client.evaluate("[...document.querySelector('#style-pack-heading').closest('section').querySelectorAll('button')].find(b=>b.textContent==='停用风格包').click();true")
        _wait(client, "document.querySelector('#style-pack-heading')?.closest('section').textContent.includes('已停用')", 'The style disable state did not render')
        status = _invoke(client, 'manage_style_packs', {'operation': 'list'})
        assert status['activeId'] == PACK_IDS[0] and not status['enabled'], status
        evidence['stylePack'] = {'importedIds': list(PACK_IDS), 'selectedId': status['activeId'], 'disabled': True}
        client.evaluate("document.querySelector('#style-pack-heading').scrollIntoView({block:'center'});true")
        client.screenshot(root / 'conversation-style-pack.png')

        ids, image = _seed_messages(database, account)
        # Prevent known face artwork from reaching any remote host in this fixture.
        client.command('Network.enable', {})
        client.command('Network.setBlockedURLs', {'urls': ['*koishi.js.org/QFace/*', '*raw.githubusercontent.com/koishijs/QFace/*']})
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='联系人规则').click();true")
        _wait(client, "[...document.querySelectorAll('button')].some(b=>b.textContent==='群聊会话')", 'The fixture group did not render')
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='群聊会话').click();true")
        _wait(client, "!!document.querySelector('textarea[aria-label=\"消息内容\"]') && document.body.textContent.includes('混排前')", 'The real cached group messages did not render')
        _wait(client, "document.querySelectorAll('[title=\"QQ原生表情 76\"]').length>=2 && document.querySelector('[title=\"QQ原生表情 999999\"]')?.textContent.includes('QQ表情 999999')", 'The offline native face fallback did not render')
        evidence['nativeFace'] = {'pureFace': True, 'mixedTextAndFace': True, 'knownNamedFallback': client.evaluate("document.querySelector('[title=\"QQ原生表情 76\"]').textContent"), 'unknownIdFallback': True, 'artworkNetworkBlocked': True}
        with _connect(database) as connection:
            assert connection.execute('SELECT is_from_me FROM messages_log WHERE msg_id=?', (f'msg_{TARGET}_context-owner',)).fetchone()[0] == 1, 'History did not repair the owner direction'
        client.screenshot(root / 'conversation-native-face.png')
        _invoke(client, 'manage_style_packs', {'operation': 'select', 'packId': PACK_IDS[1]})
        try:
            evidence['context'] = _capture_request(client, database, account)
        finally:
            _invoke(client, 'manage_style_packs', {'operation': 'select', 'packId': PACK_IDS[0]})
            _invoke(client, 'manage_style_packs', {'operation': 'disable'})
        if os.environ.get('EAZYQQ_TEST_FACE_NETWORK') == '1':
            evidence['nativeAnimation'] = _validate_native_animation(client, root, database)
        assert all(len(server.sent_messages) == offset for server, offset in zip(servers, sent_offsets)), 'Context validation unexpectedly sent a QQ message'
    finally:
        client.evaluate("document.querySelector('button[aria-label=\"关闭会话\"]')?.click();true")
        client.command('Network.setBlockedURLs', {'urls': []})
        if ids:
            with _connect(database) as connection:
                connection.executemany('DELETE FROM messages_log WHERE msg_id=?', [(value,) for value in ids])
        if image is not None:
            image.unlink(missing_ok=True)
    root.joinpath('conversation-context-ui.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    return evidence


def validate_style_pack_account_isolation(client, root, account='10002'):
    """Run after selecting the second account; no primary pack may be visible."""
    root = _root(root)
    _database(root, account)
    status = _invoke(client, 'manage_style_packs', {'operation': 'list'})
    assert not status['packs'] and not status['activeId'] and not status['enabled'], status
    _open_settings(client)
    assert client.evaluate("document.querySelector('#style-pack-heading').closest('section').querySelector('select').disabled"), 'The secondary account exposed a style selection'
    evidence = {'account': account, 'primaryPacksAbsent': True, 'usesRealIpc': True}
    root.joinpath('conversation-style-isolation.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    return evidence


def validate_style_pack_account_return(client, root, account='10001'):
    """Run after returning to the primary account; its own disabled selection survives."""
    root = _root(root)
    _database(root, account)
    status = _invoke(client, 'manage_style_packs', {'operation': 'list'})
    assert status['activeId'] == PACK_IDS[0] and not status['enabled'] and {pack['id'] for pack in status['packs']} == set(PACK_IDS), status
    return {'account': account, 'primarySelectionRetained': True, 'disabledRetained': True}
