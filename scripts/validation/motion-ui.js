async (page) => {
  await page.setViewportSize({ width: 1200, height: 900 });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.keyboard.press('F1');
  await page.getByRole('button', { name: '关闭使用说明书' }).waitFor();
  const normal = await page.locator('.motion-drawer').evaluate(element => ({
    duration: getComputedStyle(element).transitionDuration,
    transform: getComputedStyle(element).transform,
  }));
  if (!normal.duration.includes('0.2s')) throw new Error('Drawer motion does not use the shared duration');
  await page.waitForFunction(() => {
    const panel = document.querySelector('.motion-drawer');
    return panel && Number(getComputedStyle(panel).opacity) === 1 && getComputedStyle(panel).transform === 'matrix(1, 0, 0, 1, 0, 0)';
  });
  await page.screenshot({ path: 'B:/EazyQQ/output/ui/motion-manual-desktop.png' });
  await page.keyboard.press('F1');
  await page.getByRole('button', { name: '关闭使用说明书' }).waitFor({ state: 'hidden' });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.keyboard.press('F1');
  await page.getByRole('button', { name: '关闭使用说明书' }).waitFor();
  const reduced = await page.locator('.motion-drawer').evaluate(element => ({
    duration: getComputedStyle(element).transitionDuration,
    transform: getComputedStyle(element).transform,
  }));
  if (reduced.transform !== 'none' || reduced.duration !== '0.08s') throw new Error('Reduced-motion drawer retains spatial movement');
  await page.setViewportSize({ width: 900, height: 700 });
  await page.screenshot({ path: 'B:/EazyQQ/output/ui/motion-manual-reduced.png' });
  await page.keyboard.press('F1');
  await page.getByRole('button', { name: '关闭使用说明书' }).waitFor({ state: 'hidden' });
  return { sharedTiming: true, reducedMotion: true, drawerClosing: true, screenshots: 2 };
}
