/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef mozilla_GfxXray_h
#define mozilla_GfxXray_h

#include "nsIGfxXray.h"
#include "mozilla/StaticPtr.h"

namespace mozilla {

class GfxXray final : public nsIGfxXray {
 public:
  NS_DECL_ISUPPORTS
  NS_DECL_NSIGFXXRAY

  static already_AddRefed<nsIGfxXray> GetSingleton();

 private:
  ~GfxXray() = default;
};

}  // namespace mozilla

#endif  // mozilla_GfxXray_h