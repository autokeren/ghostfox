/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "GfxXray.h"

#include "mozilla/Base64.h"
#include "mozilla/Monitor.h"
#include "mozilla/gfx/DataSurfaceHelpers.h"
#include "mozilla/dom/CanvasUtils.h"
#include "mozilla/dom/OffscreenCanvas.h"
#include "mozilla/dom/WorkerRunnable.h"
#include "nsIThread.h"
#include "nsIThreadManager.h"
#include "nsThreadUtils.h"
#include "prthread.h"
#include "mozilla/dom/HTMLCanvasElement.h"
#include "mozilla/dom/OffscreenCanvasDisplayHelper.h"
#include "nsIContent.h"
#include "nsString.h"

namespace mozilla {

NS_IMPL_ISUPPORTS(GfxXray, nsIGfxXray)

namespace {
StaticRefPtr<GfxXray> gGfxXray;

// M2.9: snapshot a worker-owned OffscreenCanvas on ITS owning thread.
// The 2D context's current drawing surface is only touchable there
// (upstream's front-buffer snapshot is WebGL-only — 2D workers were
// unreadable). Dispatched through the canvas GLOBAL's event target, so
// no WorkerRef/WorkerPrivate lifecycle is involved.
class XrayWorkerSnapshotRunnable final : public Runnable {
 public:
  explicit XrayWorkerSnapshotRunnable(dom::OffscreenCanvasDisplayHelper* aHelper)
      : Runnable("XrayWorkerSnapshotRunnable"),
        mMonitor("XrayWorkerSnapshotRunnable::mMonitor"),
        mHelper(aHelper) {}

  NS_IMETHOD Run() override {
    RefPtr<dom::OffscreenCanvas> canvas = mHelper->GetCanvas();
    gfxAlphaType alphaType = gfxAlphaType::Premult;
    RefPtr<gfx::SourceSurface> surface;
    if (canvas) {
      surface = canvas->GetSurfaceSnapshot(&alphaType);
      if (surface && surface->GetType() == gfx::SurfaceType::SKIA) {
        surface = gfx::Factory::CopyDataSourceSurface(
            static_cast<gfx::DataSourceSurface*>(surface.get()));
      }
    }
    MonitorAutoLock lock(mMonitor);
    mSurface = std::move(surface);
    mComplete = true;
    lock.NotifyAll();
    return NS_OK;
  }

  already_AddRefed<gfx::SourceSurface> Wait(int32_t aTimeoutMs) {
    MonitorAutoLock lock(mMonitor);
    TimeDuration timeout = TimeDuration::FromMilliseconds(aTimeoutMs);
    while (!mComplete) {
      if (lock.Wait(timeout) == CVStatus::Timeout) {
        return nullptr;
      }
    }
    return mSurface.forget();
  }

 private:
  Monitor mMonitor;
  RefPtr<dom::OffscreenCanvasDisplayHelper> mHelper;
  RefPtr<gfx::SourceSurface> mSurface MOZ_GUARDED_BY(mMonitor);
  bool mComplete MOZ_GUARDED_BY(mMonitor) = false;
};
}  // namespace

already_AddRefed<nsIGfxXray> GfxXray::GetSingleton() {
  if (!gGfxXray) {
    gGfxXray = new GfxXray();
  }
  return do_AddRef(gGfxXray);
}

NS_IMETHODIMP
GfxXray::CanvasBuffer(nsISupports* aElement, nsAString& aRetval) {
  aRetval.Truncate();
  if (!aElement) {
    return NS_OK;
  }

  nsCOMPtr<nsIContent> content = do_QueryInterface(aElement);
  if (!content) {
    return NS_OK;
  }
  auto* canvas = dom::HTMLCanvasElement::FromNode(content);
  if (!canvas) {
    return NS_OK;
  }

  // One native read covers every canvas flavor: 2D (mCurrentContext),
  // WebGL (context internal) and worker-transferred OffscreenCanvas
  // (the mutex-guarded display helper). Unrestricted extraction: no
  // fingerprint randomization, the honest pixels.
  int32_t format = 0;
  gfx::IntSize size;
  UniquePtr<uint8_t[]> buf = canvas->GetImageBufferPublic(
      CanvasUtils::ImageExtraction::Unrestricted, &format, &size);
  RefPtr<gfx::SourceSurface> xraySurface;
  if (!buf && canvas->GetOffscreenDisplay()) {
    auto* display = canvas->GetOffscreenDisplay();
    // Worker-transferred canvas: snapshot the CURRENT drawing surface on
    // the owning thread (2D contexts have no front-buffer snapshot).
    if (canvas->GetOffscreenCanvas() && display->GetWorkerPrivate()) {
      // Worker-owned canvas: snapshot on its thread. Known limitation —
      // the helper's worker ref can be cleared by the transfer lifecycle
      // for short-lived test workers; persistent site workers keep it.
      auto runnable = MakeRefPtr<XrayWorkerSnapshotRunnable>(display);
      nsCOMPtr<nsISerialEventTarget> target =
          display->GetWorkerPrivate()->HybridEventTarget();
      if (target &&
          NS_SUCCEEDED(target->Dispatch(runnable, NS_DISPATCH_NORMAL))) {
        xraySurface = runnable->Wait(1000);
      }
    } else {
      // Main-thread transferred canvas: flush + poll the display frame.
      display->FlushForDisplay();
      nsCOMPtr<nsIThread> main = NS_GetCurrentThread();
      for (int i = 0; i < 200 && !buf; ++i) {
        PR_Sleep(PR_MillisecondsToInterval(5));
        NS_ProcessNextEvent(main, false);
        buf = canvas->GetImageBufferPublic(
            CanvasUtils::ImageExtraction::Unrestricted, &format, &size);
      }
    }
  }
  if (!buf && xraySurface) {
    RefPtr<gfx::DataSourceSurface> dataSurface = xraySurface->GetDataSurface();
    if (dataSurface) {
      size = dataSurface->GetSize();
      buf = gfx::SurfaceToPackedBGRA(dataSurface);
    }
  }
  if (!buf || size.width <= 0 || size.height <= 0) {
    return NS_OK;
  }

  // SurfaceToPackedBGRA -> RGBA (uniform with the WebGL readPixels path).
  const size_t pixelCount = static_cast<size_t>(size.width) * size.height;
  uint8_t* data = buf.get();
  for (size_t i = 0; i < pixelCount; ++i) {
    const uint8_t blue = data[i * 4 + 0];
    data[i * 4 + 0] = data[i * 4 + 2];
    data[i * 4 + 2] = blue;
  }

  nsCString encoded;
  nsresult rv = Base64EncodeAppend(reinterpret_cast<const char*>(data),
                                   pixelCount * 4, encoded);
  if (NS_FAILED(rv)) {
    return rv;
  }

  nsCString packed;
  packed.AppendInt(size.width);
  packed.Append('x');
  packed.AppendInt(size.height);
  packed.Append(':');
  packed.Append(encoded);
  aRetval = NS_ConvertUTF8toUTF16(packed);
  return NS_OK;
}

}  // namespace mozilla