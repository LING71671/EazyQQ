import QRCode from 'qrcode';

export function qrImage(value: string): Promise<string> {
  if (value.startsWith('data:image/')) return Promise.resolve(value);
  if (/^https?:\/\//.test(value)) return QRCode.toDataURL(value, { width: 256, margin: 2 });
  return Promise.resolve(`data:image/png;base64,${value}`);
}
