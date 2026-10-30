import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { NativeFace, NativeMessageText, nativeFaceLabel } from '../NativeFace';

afterEach(cleanup);

it('uses the pinned real APNG asset for current QQ faces', () => {
  render(<NativeFace id="427" />);
  const image = screen.getByRole('img', { name: '[偷感]' });
  expect(image.getAttribute('src')).toBe('https://raw.githubusercontent.com/koishijs/QFace/c96fa14ff894147ac0442dda0fce6a4523e5fcbd/public/assets/qq_emoji/427/apng/427.png');
  expect(image.getAttribute('src')).not.toContain('gif/s427.gif');
  expect(nativeFaceLabel('317')).toBe('菜汪');
});

it('keeps current face IDs and text inline with a named offline fallback', () => {
  const { container } = render(<NativeMessageText content="收到[CQ:face,id=324]这个[CQ:face,id=427]" />);
  expect(container.textContent).toBe('收到这个');
  const candy = screen.getByRole('img', { name: '[吃糖]' });
  const sneaky = screen.getByRole('img', { name: '[偷感]' });
  expect(candy.getAttribute('src')).toContain('/324/apng/324.png');
  fireEvent.error(sneaky);
  expect(screen.getByText('[偷感]')).toBeTruthy();
  expect(screen.getByText('[偷感]').getAttribute('title')).toBe('QQ原生表情 427');
});

it('keeps unknown IDs as text and resets a failed preview when the face changes', () => {
  const view = render(<NativeFace id="427" />);
  fireEvent.error(screen.getByRole('img', { name: '[偷感]' }));
  view.rerender(<NativeFace id="324" />);
  expect(screen.getByRole('img', { name: '[吃糖]' })).toBeTruthy();
  view.rerender(<NativeFace id="999999" />);
  expect(screen.getByText('[QQ表情 999999]')).toBeTruthy();
  expect(screen.queryByRole('img')).toBeNull();
});
