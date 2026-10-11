import { useState } from 'react';
import { ClientUpdate } from './updates/ClientUpdate';
import { CoreUpdate } from './updates/CoreUpdate';
import { UpdateHistory } from './updates/UpdateHistory';

export function AppUpdateCard() {
  const [appBusy, setAppBusy] = useState(false);
  const [coreBusy, setCoreBusy] = useState(false);
  return <div className="space-y-4"><ClientUpdate blocked={coreBusy} onBusy={setAppBusy} /><CoreUpdate blocked={appBusy} onBusy={setCoreBusy} /><UpdateHistory /></div>;
}
