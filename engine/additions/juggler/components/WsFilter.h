/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef mozilla_WsFilter_h
#define mozilla_WsFilter_h

#include "nsIJugglerWsFilter.h"
#include "mozilla/Mutex.h"
#include "mozilla/StaticPtr.h"
#include "nsTHashSet.h"

namespace mozilla {

class WsFilter final : public nsIJugglerWsFilter {
 public:
  NS_DECL_ISUPPORTS
  NS_DECL_NSIJUGGLERWSFILTER

  static already_AddRefed<nsIJugglerWsFilter> GetSingleton();

  bool IsBlockedInternal(uint32_t aSerial);

 private:
  ~WsFilter() = default;
  Mutex mLock{"WsFilter"};
  nsTHashSet<uint32_t> mBlocked;
};

}  // namespace mozilla

#endif  // mozilla_WsFilter_h
