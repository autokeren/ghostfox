/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "WsFilter.h"

namespace mozilla {

StaticRefPtr<WsFilter> gWsFilter;

NS_IMPL_ISUPPORTS(WsFilter, nsIJugglerWsFilter)

already_AddRefed<nsIJugglerWsFilter> WsFilter::GetSingleton() {
  if (!gWsFilter) {
    gWsFilter = new WsFilter();
  }
  return do_AddRef(gWsFilter.get());
}

bool WsFilter::IsBlockedInternal(uint32_t aSerial) {
  MutexAutoLock lock(mLock);
  return mBlocked.Contains(aSerial);
}

NS_IMETHODIMP WsFilter::SetBlocked(uint32_t aSerial, bool aBlocked) {
  MutexAutoLock lock(mLock);
  if (aBlocked) {
    mBlocked.Insert(aSerial);
  } else {
    mBlocked.Remove(aSerial);
  }
  return NS_OK;
}

NS_IMETHODIMP WsFilter::IsBlocked(uint32_t aSerial, bool* aBlocked) {
  *aBlocked = IsBlockedInternal(aSerial);
  return NS_OK;
}

}  // namespace mozilla
