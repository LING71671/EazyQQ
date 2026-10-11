"""Send synthetic events only when explicitly requested by the QA caller."""
import json
import time

def validate_sentry_live(client, root):
    client.evaluate("document.querySelector('section[aria-label=\"Sentry 诊断\"]').scrollIntoView({block:'center'});true")
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("(()=>{const button=[...document.querySelectorAll('button')].find(button=>button.textContent==='发送诊断测试'&&!button.disabled);if(!button)return false;button.click();return true;})()"):
            break
        time.sleep(.1)
    else:
        raise AssertionError('Live Sentry test was not enabled in this explicitly requested QA session')
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        text = client.evaluate("document.querySelector('section[aria-label=\"Sentry 诊断\"]').textContent")
        if '发送队列已处理' in text:
            break
        time.sleep(.1)
    else:
        raise AssertionError(f'Sentry queues did not drain: {text}')
    import re
    ids = re.findall(r'事件 ([0-9a-f]{32})', text)
    assert len(ids)==2, text
    (root/'sentry-live-events.json').write_text(json.dumps({'rendererEventId':ids[0],'desktopEventId':ids[1],'realQQTouched':False}),encoding='utf-8')
    client.screenshot(root/'sentry-diagnostic.png')
