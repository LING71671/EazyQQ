"""Verify real IPC save/send refusal against isolated SQLite and OneBot fixtures."""
import json
from pathlib import Path
import sqlite3
import time


def _wait(client, expression, message, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if client.evaluate(expression):
                return
        except (AssertionError, OSError):
            # A page reload can briefly invalidate its old JavaScript context.
            pass
        time.sleep(.1)
    raise AssertionError(message)


def _read_config(client):
    return client.evaluate("""(async () => {
      const result = await window.__TAURI_INTERNALS__.invoke('get_config');
      if (!result.success || !result.data) throw new Error('Cannot read the real account config');
      return result.data;
    })()""")


def validate_reliability(client, root, servers):
    root = Path(root).resolve(strict=True)
    runtime_root = next((path for path in root.parents if path.name == '.test-runtime'), None)
    assert runtime_root and root.is_relative_to(runtime_root.resolve(strict=True)), 'Use an isolated native fixture directory'
    databases = list((root / 'EazyQQ_Data/accounts').glob('*/10001/eazyqq.db'))
    assert len(databases) == 1, f'Expected one primary fixture database, found {len(databases)}'
    database = databases[0].resolve(strict=True)
    assert database.is_relative_to(root), 'The database escaped the native fixture root'
    assert servers and all(hasattr(server, 'reject_manual_sends') and isinstance(server.sent_messages, list) for server in servers), 'OneBot servers must support real send refusal'
    original_config = _read_config(client)
    original_storage = client.evaluate("Object.fromEntries(Object.entries(localStorage))")
    original_rejections = [server.reject_manual_sends for server in servers]
    sent_offsets = [len(server.sent_messages) for server in servers]
    evidence = {'usesRealIpc': True, 'database': str(database.relative_to(root))}
    connection = None
    try:
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='系统设置').click();true")
        _wait(client, "!!document.querySelector('input[list=\"dynamic-models-list\"]')", 'Model input did not render')
        connection = sqlite3.connect(database.as_uri() + '?mode=rw', uri=True, timeout=5, isolation_level=None)
        assert connection.execute('PRAGMA journal_mode = WAL').fetchone()[0].lower() == 'wal'
        connection.execute('PRAGMA synchronous = NORMAL')
        connection.execute('PRAGMA foreign_keys = ON')
        connection.execute('PRAGMA cache_size = -64000')
        connection.execute('PRAGMA busy_timeout = 5000')
        connection.execute('BEGIN IMMEDIATE')
        client.evaluate("""(() => {
          const input=document.querySelector('input[list="dynamic-models-list"]');
          Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'reliability-unsaved-model');
          input.dispatchEvent(new Event('input',{bubbles:true}));
          return true;
        })()""")
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='保存所有设置').click();true")
        _wait(client, "[...document.querySelectorAll('[role=alert]')].some(e=>e.textContent.includes('设置未保存') && /locked|busy|锁|占用/i.test(e.textContent))", 'The real SQLite write refusal did not become a rendered error')
        evidence['settingsError'] = client.evaluate("[...document.querySelectorAll('[role=alert]')].find(e=>e.textContent.includes('设置未保存')).textContent")
        evidence['settingsDraftRetained'] = client.evaluate("document.querySelector('input[list=\"dynamic-models-list\"]').value==='reliability-unsaved-model'")
        evidence['settingsDidNotClaimSaved'] = client.evaluate("![...document.querySelectorAll('button')].some(b=>b.textContent==='已保存')")
        assert evidence['settingsDraftRetained'] and evidence['settingsDidNotClaimSaved'], evidence
        connection.rollback()
        connection.close()
        connection = None
        client.evaluate("[...document.querySelectorAll('[role=alert]')].find(e=>e.textContent.includes('设置未保存')).scrollIntoView({block:'center'});true")
        client.screenshot(root / 'reliability-settings.png')

        for server in servers:
            server.reject_manual_sends = True
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='联系人规则').click();true")
        _wait(client, "[...document.querySelectorAll('button')].some(b=>b.textContent==='群聊会话')", 'Fixture group did not render in contacts')
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='群聊会话').click();true")
        _wait(client, "!!document.querySelector('textarea[aria-label=\"消息内容\"]')", 'Chat composer did not render')
        client.evaluate("""(() => {
          const input=document.querySelector('textarea[aria-label="消息内容"]');
          Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'Reliability reply stays unsent');
          input.dispatchEvent(new Event('input',{bubbles:true}));
          return true;
        })()""")
        client.evaluate("document.querySelector('button[aria-label=\"发送消息\"]').click();true")
        _wait(client, "[...document.querySelectorAll('[role=alert]')].some(e=>e.textContent.includes('模拟消息发送失败'))", 'The real OneBot send refusal did not become a rendered error')
        evidence['sendError'] = client.evaluate("[...document.querySelectorAll('[role=alert]')].find(e=>e.textContent.includes('模拟消息发送失败')).textContent")
        evidence['messageDraftRetained'] = client.evaluate("document.querySelector('textarea[aria-label=\"消息内容\"]').value==='Reliability reply stays unsent'")
        evidence['sentMessages'] = [dict(body) for server, offset in zip(servers, sent_offsets) for body in server.sent_messages[offset:]]
        assert evidence['messageDraftRetained'], evidence
        assert len(evidence['sentMessages']) == 1, evidence
        sent = evidence['sentMessages'][0]
        assert str(sent.get('group_id')) == '995001' and 'user_id' not in sent, evidence
        assert sent.get('message') == 'Reliability reply stays unsent', evidence
        client.screenshot(root / 'reliability-chat.png')
    finally:
        if connection is not None:
            try:
                if connection.in_transaction:
                    connection.rollback()
            finally:
                connection.close()
        for server, original in zip(servers, original_rejections):
            server.reject_manual_sends = original
        client.evaluate("document.querySelector('button[aria-label=\"关闭会话\"]')?.click();true")
        client.command('Page.reload', {})
        _wait(client, "!!window.__TAURI_INTERNALS__ && [...document.querySelectorAll('button')].some(b=>b.textContent==='系统设置')", 'Restored application did not render')
        _wait(client, "!document.querySelector('.boot-anim-spin')", 'Restored application did not finish startup')
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='系统设置').click();true")
        _wait(client, "!!document.querySelector('input[list=\"dynamic-models-list\"]')", 'Restored settings did not render')
    evidence['persistentConfigUnchanged'] = _read_config(client) == original_config
    evidence['browserStorageUnchanged'] = client.evaluate("Object.fromEntries(Object.entries(localStorage))") == original_storage
    assert evidence['persistentConfigUnchanged'] and evidence['browserStorageUnchanged'], evidence
    root.joinpath('reliability-ui.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    return evidence

