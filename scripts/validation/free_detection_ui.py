"""Validate cancellation and remount using only the isolated slow runtime fixture."""
import json
import time


def _wait(client, expression, message, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if client.evaluate(expression):
            return
        time.sleep(.1)
    raise AssertionError(message)


def validate_free_detection_lifecycle(client, root, runtime):
    assert runtime.parent.resolve() == root.resolve() and '.test-runtime' in root.parts
    slow = runtime.with_name(runtime.name + '.slow')
    started = runtime.with_name(runtime.name + '.started')
    before = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_config');return r.data.ai})()")
    try:
        slow.write_text('isolated validation only', encoding='utf-8')
        client.evaluate("(()=>{const b=[...document.querySelectorAll('button')].find(b=>b.textContent==='自动检测免费模型');if(!b||b.disabled)throw Error('Discovery is not ready');b.click();return true})()")
        deadline = time.monotonic() + 15
        while not started.exists() and time.monotonic() < deadline:
            time.sleep(.05)
        assert started.exists(), 'The owned fake inference process did not start'
        _wait(client, "[...document.querySelectorAll('button')].some(b=>b.textContent==='停止检测'&&!b.disabled)", 'Stop action was unavailable')
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='停止检测').click();true")
        _wait(client, "(()=>{const s=document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]');return s?.textContent.includes('已停止')&&[...s.querySelectorAll('button')].some(b=>b.textContent==='自动检测免费模型'&&!b.disabled)})()", 'Cancelled discovery did not finish')
        reports = list(root.glob('EazyQQ_Data/accounts/*/10001/ai-runtime/free-model-last-attempt.json'))
        assert len(reports) == 1
        report = json.loads(reports[0].read_text(encoding='utf-8'))
        assert report['status'] == 'cancelled' and report['completed'] < report['total'], report
        assert not list(reports[0].parent.glob('free-model-detection-*/*')), 'Owned temporary runtime files remain after cancellation'
        client.evaluate("document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]').scrollIntoView({block:'center'});true")
        client.screenshot(root / 'free-model-cancelled.png')
        slow.unlink()
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='联系人规则').click();true")
        client.evaluate("[...document.querySelectorAll('button')].find(b=>b.textContent==='系统设置').click();true")
        _wait(client, "(()=>{const s=document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]');return s?.textContent.includes('历史检测证据')&&[...s.querySelectorAll('button')].some(b=>b.textContent==='自动检测免费模型'&&!b.disabled)})()", 'Discovery did not remount with historical evidence')
        after = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_config');return r.data.ai})()")
        assert before == after, 'Cancel/remount silently changed the saved model'
        evidence = {'cancelled': True, 'status': report['status'], 'completed': report['completed'], 'total': report['total'], 'modelUnchanged': before == after, 'remountReady': True, 'realModelTouched': False}
        (root / 'free-model-cancellation.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    finally:
        slow.unlink(missing_ok=True)
        started.unlink(missing_ok=True)
