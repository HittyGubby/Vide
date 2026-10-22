import { isTauri } from "$lib/isTauri";
import { isRemoteGuest, isRemoteMode } from "$lib/transport";

export const auth = $state({ authenticated: false });

export function canManage(): boolean {
  if (isRemoteMode()) return !isRemoteGuest() || auth.authenticated;
  return isTauri || auth.authenticated;
}
