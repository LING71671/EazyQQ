"""Exercise task, persona and typed draft controls in an isolated real Tauri window."""
import json
import time

from conversation_context_ui import _connect, _database, _invoke, _root, _wait


def _fill(client, label, value):
    client.evaluate(f"""(() => {{
      const input = document.querySelector('[aria-label=' + JSON.stringify({json.dumps(label)}) + ']');
      const prototype = input.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : input.tagName === 'SELECT' ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(prototype, 'value').set.call(input, {json.dumps(value, ensure_ascii=False)});
      input.dispatchEvent(new Event(input.tagName === 'SELECT' ? 'change' : 'input', {{bubbles: true}}));
      return true;
    }})()""")


def _click(client, text):
    client.evaluate(f"[...document.querySelectorAll('button')].find(button => button.textContent === {json.dumps(text, ensure_ascii=False)} || (button.closest('aside') && button.textContent.startsWith({json.dumps(text, ensure_ascii=False)}))).click();true")


def _capture_dark(client, root, selector, filename):
    light = client.evaluate(f"getComputedStyle(document.querySelector({json.dumps(selector)})).backgroundColor")
    client.evaluate("document.querySelector('button[title=\"切换至暗色模式\"]').click();true")
    try:
        _wait(client, "document.documentElement.classList.contains('dark')", 'The real theme toggle did not enable dark mode')
        _wait(client, f"getComputedStyle(document.querySelector({json.dumps(selector)})).backgroundColor !== {json.dumps(light)}", 'The new view retained its light background in dark mode')
        dark = client.evaluate(f"getComputedStyle(document.querySelector({json.dumps(selector)})).backgroundColor")
        contrast = None
        if filename == 'tasks-workbench-dark.png':
            contrast_expression = """(() => {
              const row = document.querySelector('nav[aria-label="任务列表"] button[aria-current="true"]');
              const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1;
              const context = canvas.getContext('2d', {willReadFrequently: true});
              const luminance = color => {
                context.clearRect(0, 0, 1, 1); context.fillStyle = color; context.fillRect(0, 0, 1, 1);
                const rgb = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map(value => { const channel = value / 255; return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4; });
                return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
              };
              const foreground = getComputedStyle(row.querySelector('span')).color;
              const background = getComputedStyle(row).backgroundColor;
              const a = luminance(foreground), b = luminance(background);
              return {foreground, background, ratio: (Math.max(a, b) + .05) / (Math.min(a, b) + .05)};
            })()"""
            _wait(client, f"({contrast_expression}).ratio >= 4.5", 'The selected task title did not settle to readable dark contrast')
            contrast = client.evaluate(contrast_expression)
            assert contrast['ratio'] >= 4.5, f'The selected task title lacks readable dark contrast: {contrast}'
        client.screenshot(root / filename)
        return {'lightBackground': light, 'darkBackground': dark, 'themeToggleChangesRenderedStyle': True, **({'selectedTaskContrast': contrast} if contrast else {})}
    finally:
        client.evaluate("document.querySelector('button[title=\"切换至明亮模式\"]').click();true")
        _wait(client, "!document.documentElement.classList.contains('dark')", 'The real theme toggle did not restore light mode')


def validate_tasks_replies(client, root, servers, account='10001'):
    root = _root(root)
    database = _database(root, account)
    assert servers and all(str(server.uin) in ('10001', '10002', '10003') for server in servers), 'Only synthetic protocol servers are allowed'
    sent_offsets = [len(server.sent_messages) for server in servers]
    evidence = {'usesRealTauri': True, 'usesRealIpc': True, 'realQQTouched': False, 'remoteModelsContacted': False}
    created_id = None
    draft_id = 'native-typed-bubbles'
    original_persona = _invoke(client, 'manage_personas', {'request': {'operation': 'get'}})
    try:
        client.evaluate("document.querySelector('button[aria-label=\"关闭会话\"]')?.click();true")
        _click(client, '任务工作台')
        _wait(client, "!!document.querySelector('form[aria-label=\"新建任务\"]') && !!document.querySelector('select[aria-label=\"任务会话\"] option[value=\"group:995001\"]')", 'The real task form or fixture target did not render')
        _fill(client, '任务会话', 'group:995001')
        _fill(client, '任务标题', '原生任务界面夹具')
        _fill(client, '任务要求', '只读取此群已经缓存的消息，整理安排')
        _wait(client, "![...document.querySelectorAll('button')].find(button=>button.textContent==='创建任务').disabled", 'The task form remained disabled')
        _click(client, '创建任务')
        _wait(client, "document.querySelector('section[aria-label=\"任务详情\"]')?.textContent.includes('原生任务界面夹具')", 'Task creation was not reflected in the real window')
        listing = _invoke(client, 'manage_tasks', {'request': {'operation': 'list'}})
        created = next(task for task in listing['tasks'] if task['title'] == '原生任务界面夹具')
        created_id = created['id']
        assert created['accountUin'] == account and created['targetType'] == 'group' and created['targetId'] == '995001', created
        assert created['state'] == 'ready' and not created['messages'], 'Creating a task unexpectedly performed inference'
        client.screenshot(root / 'tasks-workbench.png')
        evidence['tasks'] = {'createdForFixedAccount': True, 'explicitConversationType': True, 'creationDoesNotInfer': True, 'sessionId': created['session']['id']}
        evidence['tasks']['theme'] = _capture_dark(client, root, 'section[aria-label="任务详情"]', 'tasks-workbench-dark.png')
        _click(client, '删除任务')
        _wait(client, "!document.querySelector('section[aria-label=\"任务详情\"]')", 'Deleting the task did not clear its detail')
        assert all(task['id'] != created_id for task in _invoke(client, 'manage_tasks', {'request': {'operation': 'list'}})['tasks'])
        created_id = None

        _click(client, '系统设置')
        _wait(client, "!!document.querySelector('#persona-heading') && document.querySelector('#persona-heading').closest('section').getAttribute('aria-busy')==='false'", 'The real persona editor did not finish loading')
        _fill(client, '表达偏好正文', '合成验收偏好：先回应重点，再给下一步')
        _click(client, '保存表达偏好')
        _wait(client, "document.querySelector('#persona-heading').closest('section').querySelector('[role=status]')?.textContent==='已保存'", 'The persona editor did not confirm persistence')
        assert _invoke(client, 'manage_personas', {'request': {'operation': 'get'}})['text'] == '合成验收偏好：先回应重点，再给下一步'
        client.evaluate("document.querySelector('#persona-heading').scrollIntoView({block:'center'});true")
        client.screenshot(root / 'tasks-persona-editor.png')
        evidence['persona'] = {'savedViaEditor': True, 'readBackThroughIpc': True}
        evidence['persona']['theme'] = _capture_dark(client, root, 'section[aria-labelledby="persona-heading"]', 'tasks-persona-editor-dark.png')

        _invoke(client, 'get_pending_drafts')
        plan = {'schemaVersion': 1, 'bubbles': [
            {'id': 'b1', 'segments': [{'type': 'text', 'text': '第一条\n同一气泡的换行'}]},
            {'id': 'b2', 'segments': [{'type': 'face', 'faceKey': '427'}]},
        ]}
        draft = {'id': draft_id, 'targetId': '995001', 'targetName': '合成草稿', 'targetType': 'group', 'replyToMsgId': 'native-typed-source', 'incomingMessageSnippet': '合成来信', 'generatedContent': '第一条\n同一气泡的换行\n\n[CQ:face,id=427]', 'replyPlan': plan, 'thinkingContent': None, 'modelUsed': 'offline-fixture', 'createdAt': int(time.time() * 1000)}
        with _connect(database) as connection:
            connection.execute('INSERT INTO pending_drafts(id,target_id,target_name,target_type,reply_to_msg_id,incoming_message_snippet,generated_content,thinking_content,model_used,created_at,reply_plan_json) VALUES (?,?,?,?,?,?,?,?,?,?,?)', (draft_id, draft['targetId'], draft['targetName'], draft['targetType'], draft['replyToMsgId'], draft['incomingMessageSnippet'], draft['generatedContent'], None, draft['modelUsed'], draft['createdAt'], json.dumps(plan)))
        client.command('Network.enable', {})
        client.command('Network.setBlockedURLs', {'urls': ['*koishi.js.org/QFace/*', '*raw.githubusercontent.com/koishijs/QFace/*']})
        client.evaluate(f"window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {{event:'new-draft',payload:{json.dumps(draft, ensure_ascii=False)}}})")
        _click(client, '草稿审核')
        _wait(client, "document.querySelectorAll('article[aria-label=\"草稿：合成草稿\"] textarea').length===2", 'The typed draft did not render as two editable bubbles')
        assert client.evaluate("document.querySelector('textarea[aria-label=\"回复正文：合成草稿\"]').value") == '第一条\n同一气泡的换行'
        assert client.evaluate("document.querySelector('textarea[aria-label=\"回复正文第2条：合成草稿\"]').value") == '[CQ:face,id=427]'
        _wait(client, "document.querySelector('article[aria-label=\"草稿：合成草稿\"]').textContent.includes('[偷感]')", 'The pure native face preview did not render its named offline fallback')
        article = 'document.querySelector(\'article[aria-label="草稿：合成草稿"]\')'
        client.evaluate(f"[...{article}.querySelectorAll('button')].find(button=>button.textContent==='添加一条消息').click();true")
        _wait(client, f"{article}.querySelectorAll('textarea').length===3", 'Adding an explicit bubble failed')
        assert client.evaluate(f"[...{article}.querySelectorAll('button')].find(button=>button.textContent==='确认发送').disabled"), 'An empty added bubble was sendable'
        client.evaluate(f"{article}.querySelector('button[aria-label=\"删除第3条消息\"]').click();true")
        _wait(client, f"{article}.querySelectorAll('textarea').length===2", 'Deleting an explicit bubble failed')
        client.screenshot(root / 'tasks-typed-drafts.png')
        evidence['drafts'] = {'explicitBubbleCount': 2, 'internalNewlinePreserved': True, 'pureNativeFacePreview': True, 'emptyBubbleBlocked': True, 'bubbleAddRemove': True}
        evidence['drafts']['theme'] = _capture_dark(client, root, 'article[aria-label="草稿：合成草稿"]', 'tasks-typed-drafts-dark.png')
        assert all(len(server.sent_messages) == offset for server, offset in zip(servers, sent_offsets)), 'UI preview unexpectedly sent a message'
    finally:
        if created_id:
            _invoke(client, 'manage_tasks', {'request': {'operation': 'delete', 'taskId': created_id}})
        _invoke(client, 'manage_personas', {'request': {'operation': 'set', 'text': original_persona['text']}} if original_persona['text'] else {'request': {'operation': 'clear'}})
        with _connect(database) as connection:
            connection.execute('DELETE FROM pending_drafts WHERE id=?', (draft_id,))
        client.command('Network.setBlockedURLs', {'urls': []})
    root.joinpath('tasks-replies-ui.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    return evidence
