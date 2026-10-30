import { useState } from 'react';
import faces from '@/data/qq-faces.json';

export function nativeFaceLabel(id: string): string {
  return faces.find(face => face.id === id)?.name || `QQ表情 ${id || '未知'}`;
}

export function NativeFace({ id }: { id: string }) {
  const [failedId, setFailedId] = useState<string | null>(null);
  const name = nativeFaceLabel(id);
  const face = faces.find(face => face.id === id);
  if (failedId === id || !face?.previewUrl) {
    return <span className="inline" title={`QQ原生表情 ${id}`}>[{name}]</span>;
  }
  return <img src={face.previewUrl} alt={`[${name}]`}
    title={`QQ原生表情：${name}`} className="inline-block h-7 w-7 align-middle object-contain"
    loading="lazy" referrerPolicy="no-referrer" onError={() => setFailedId(id)} />;
}

export function NativeMessageText({ content }: { content: string }) {
  return <>{content.split(/(\[CQ:face,id=\d+\])/g).map((part, index) => {
    const face = /^\[CQ:face,id=(\d+)\]$/.exec(part);
    return face ? <NativeFace key={`${index}:${face[1]}`} id={face[1]} /> : part;
  })}</>;
}
