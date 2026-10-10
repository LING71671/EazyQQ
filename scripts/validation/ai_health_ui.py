"""Verify inference health and preview/save boundaries through the native UI."""
import time


def validate_ai_health(client, root):
    client.evaluate("[...document.querySelectorAll('button')].find(button=>button.textContent.includes('测试模型')).click(); true")
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if client.evaluate("document.querySelector('[data-chain-link=\"ai_provider\"]')?.dataset.health==='ok'"):
            break
        time.sleep(.1)
    else:
        raise AssertionError('A successful model test did not refresh the rendered diagnostic row')
    assert client.evaluate("![...document.querySelectorAll('button')].some(button=>/切.*免费通道/.test(button.textContent))"), 'Diagnosis still changes the model channel'
    time.sleep(6)
    assert client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_chain_status');return r.data.links.find(link=>link.link==='ai_provider').health})()") == 'ok', 'Passive refresh erased verified inference'
    preview = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('test_ai_connection',{modelId:'unselected-preview',apiKey:''});return r.data})()")
    assert preview['isSuccess'] and preview['matchesActiveConfig'] is False, preview
    assert client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_chain_status');return r.data.links.find(link=>link.link==='ai_provider').health})()") == 'ok', 'An unsaved preview changed active health'
    keyed = client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('test_ai_connection',{apiKey:'selected-fixture-key'});return r.data})()")
    assert keyed['reply'] == 'PAID-FIXTURE-OK' and keyed['matchesActiveConfig'] is False, keyed
    assert client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_chain_status');return r.data.links.find(link=>link.link==='ai_provider').health})()") == 'ok', 'A keyed preview changed the active no-key health'
    assert client.evaluate("(async()=>{const r=await window.__TAURI_INTERNALS__.invoke('get_config');return r.data.ai.model})()") == 'big-pickle', 'The model test silently saved its preview'
    client.evaluate("document.querySelector('[data-chain-link=\"ai_provider\"]').scrollIntoView({block:'center'});true")
    client.screenshot(root / 'ai-health-synchronized.png')
