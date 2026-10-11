"""Check real IPC receipts, rendered status semantics, and compact layout."""
import time

def seed_update_history(root):
    import json
    directory = root / 'EazyQQ_Data/updates/receipts'
    directory.mkdir(parents=True)
    now = int(time.time() * 1000)
    for index, phase in enumerate(['failed', 'installer_succeeded']):
        identity = ('a' if index == 0 else 'b') * 32
        receipt = {'schemaVersion': 1, 'operationId': identity, 'component': 'app', 'targetVersion': '9.8.7',
                   'phase': phase, 'startedAtMs': now - index * 60000, 'updatedAtMs': now,
                   'installerExitCode': 9 if index == 0 else 0, 'error': '模拟安装程序退出码 9' if index == 0 else None}
        (directory / f'{identity}.json').write_text(json.dumps(receipt, ensure_ascii=False), encoding='utf-8')

def validate_update_history(client, root, run):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("document.querySelectorAll('[data-update-operation]').length===2"):
            break
        time.sleep(.1)
    else:
        raise AssertionError('The update receipts did not render')
    values = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_update_history');return r.data})()")
    assert values == run('updates', 'history')['data'], 'GUI and CLI disagree about update evidence'
    assert [value['phase'] for value in values] == ['failed', 'installer_succeeded']
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'a'.repeat(32)+'\"]').textContent.includes('安装未完成')")
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'b'.repeat(32)+'\"]').textContent.includes('待核验版本')")
    for width, height in [(1100, 740), (900, 600)]:
        client.command('Emulation.setDeviceMetricsOverride', {'width': width, 'height': height, 'deviceScaleFactor': 1, 'mobile': False})
        client.evaluate("document.querySelector('section[aria-labelledby=\"update-history-heading\"]').scrollIntoView({block:'center'});true")
        time.sleep(.15)
        assert client.evaluate("(()=>{const s=document.querySelector('section[aria-labelledby=\"update-history-heading\"]');const r=s.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&s.scrollWidth<=s.clientWidth;})()"), 'Update history overflows the compact window'
        client.screenshot(root / f'update-history-{width}x{height}.png')
    client.command('Emulation.clearDeviceMetricsOverride', {})
