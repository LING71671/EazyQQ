"""Verify the login workspace in the real WebView using simulated identities."""
import json
import time


def validate_login_layout(client, root):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("document.querySelectorAll('section[aria-label=\"本机记忆账号\"] li').length===3 && !!document.querySelector('img[alt=\"Login QR Code\"]')"):
            break
        time.sleep(.1)
    else:
        raise AssertionError('The simulated remembered accounts and QR did not render')
    results = []
    for width, height in [(1100, 740), (900, 600)]:
        client.command('Emulation.setDeviceMetricsOverride', {'width': width, 'height': height, 'deviceScaleFactor': 1, 'mobile': False})
        time.sleep(.15)
        result = client.evaluate("""(() => {
          const workspace=document.querySelector('[data-login-workspace]'); workspace.scrollTop=0;
          const selectors=['[data-login-primary]','img[alt="Login QR Code"]','section[aria-label="本机记忆账号"]','[data-account-management] summary'];
          const elements=selectors.map(selector=>document.querySelector(selector));
          elements.push(...[...workspace.querySelectorAll('button')].filter(button=>button.checkVisibility()));
          const rects=elements.map(element=>{const r=element.getBoundingClientRect();return {text:element.textContent||element.getAttribute('alt'),x:r.x,y:r.y,right:r.right,bottom:r.bottom};});
          return {width:innerWidth,height:innerHeight,rects,visible:rects.every(r=>r.x>=0 && r.y>=0 && r.right<=innerWidth && r.bottom<=innerHeight),noHorizontalOverflow:workspace.scrollWidth<=workspace.clientWidth,noPageScroll:workspace.scrollHeight<=workspace.clientHeight,plainLogin:[...document.querySelectorAll('section[aria-label="本机记忆账号"] button')].every(button=>!button.querySelector('svg'))};
        })()""")
        assert result['visible'] and result['noHorizontalOverflow'] and result['noPageScroll'] and result['plainLogin'], result
        client.screenshot(root / f'login-layout-{width}x{height}.png')
        results.append(result)
    client.command('Emulation.clearDeviceMetricsOverride', {})
    root.joinpath('login-layout.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
