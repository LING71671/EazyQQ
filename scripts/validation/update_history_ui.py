"""Check real IPC receipts, rendered status semantics, and compact layout."""
import time

def seed_update_history(root):
    import json
    directory = root / 'EazyQQ_Data/updates/receipts'
    directory.mkdir(parents=True)
    now = int(time.time() * 1000)
    phases = ['failed', 'installer_succeeded', 'deployed', 'succeeded', 'checking']
    for index, phase in enumerate(phases):
        identity = 'abcde'[index] * 32
        receipt = {'schemaVersion': 1, 'operationId': identity, 'component': 'app', 'targetVersion': '9.8.7',
                   'phase': phase, 'startedAtMs': now - index * 60000, 'updatedAtMs': now,
                   'installerExitCode': 9 if index == 0 else 0 if phase in ['installer_succeeded', 'succeeded'] else None,
                   'error': '模拟安装程序退出码 9' if index == 0 else None}
        if phase == 'deployed':
            receipt.update(component='napcat', operation='install', events=[{'phase': 'deployed', 'atMs': now}])
        elif phase == 'succeeded':
            receipt.update(operation='install', verifiedVersion='9.8.7', events=[{'phase': 'succeeded', 'atMs': now}])
        elif phase == 'checking':
            receipt.update(operation='check', targetVersion=None, events=[{'phase': 'checking', 'atMs': now}])
        (directory / f'{identity}.json').write_text(json.dumps(receipt, ensure_ascii=False), encoding='utf-8')

def validate_update_history(client, root, run):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("document.querySelectorAll('[data-update-operation]').length===5"):
            break
        time.sleep(.1)
    else:
        raise AssertionError('The update receipts did not render')
    values = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_update_history');return r.data})()")
    assert values == run('updates', 'history')['data'], 'GUI and CLI disagree about update evidence'
    assert [value['phase'] for value in values] == ['failed', 'installer_succeeded', 'deployed', 'succeeded', 'checking']
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'a'.repeat(32)+'\"]').textContent.includes('安装未完成')")
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'b'.repeat(32)+'\"]').textContent.includes('待核验版本')")
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'c'.repeat(32)+'\"]').textContent.includes('运行版本待确认')")
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'d'.repeat(32)+'\"]').textContent.includes('版本已核验')")
    assert client.evaluate("document.querySelector('[data-update-operation=\"'+'e'.repeat(32)+'\"]').textContent.includes('任务未确认结束')")
    for width, height in [(1100, 740), (900, 600)]:
        client.command('Emulation.setDeviceMetricsOverride', {'width': width, 'height': height, 'deviceScaleFactor': 1, 'mobile': False})
        client.evaluate("document.querySelector('section[aria-labelledby=\"update-history-heading\"]').scrollIntoView({block:'center'});true")
        time.sleep(.15)
        assert client.evaluate("(()=>{const s=document.querySelector('section[aria-labelledby=\"update-history-heading\"]');const r=s.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&s.scrollWidth<=s.clientWidth;})()"), 'Update history overflows the compact window'
        client.screenshot(root / f'update-history-{width}x{height}.png')
    client.command('Emulation.clearDeviceMetricsOverride', {})
