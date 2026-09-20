# geetest-click — GeeTest v3 icon-click (文字点选) solver

Ghostfox's trained eyes for the hardest GeeTest variant: characters
drawn on a photo, click in the order shown by the instruction strip.

## How it works

| Component | Model | Role |
|---|---|---|
| `yolov8s.onnx` (44 MB) | YOLOv8s, 2 classes | Detects char boxes: `small` (<35px = instruction strip, in order) vs `big` (field chars on the photo) |
| `siamese.onnx` (17 MB) | Siamese similarity net | Compares strip glyph -> field glyph embeddings, emits the click order |

Total solve time: **~0.5 s on CPU**. No vision-language model, no OCR,
fully deterministic. Verified E2E on the real bilibili login page
(`Verification Succeeded` from the widget, 2/2 runs).

## Usage

```python
from geetest_solve import solve_image, solve_api

# 1) Browser-integrated (Ghostfox hands): solve the image shown by the widget
boxes = solve_image(img_bytes)          # [[x0, y0], ...] in click order, raw img coords
#    -> click each box center via page_drag (human mouse), then .geetest_commit

# 2) Pure API (no browser): full GeeTest protocol + verify
solve_api(gt, challenge)                # -> {'result': 'success', 'validate': ...}
```

`crack.py` implements the GeeTest v3 API protocol (gettype -> get_c_s ->
ajax -> get_pic -> verify) with AES/RSA parameter encryption and mouse
path encoding. Image fetch goes straight to `api.geevisit.com/get.php`
— bypasses the browser widget entirely (useful when the in-page widget
rate-limits, error_01 "refresh too much").

## Model location

`yolov8s.onnx` and `siamese.onnx` ship with the Ghostfox distribution
under `models/geetest_click/`.

## Attribution

Protocol + inference adapted from
[ravizhan/geetest-v3-click-crack](https://github.com/ravizhan/geetest-v3-click-crack)
(**AGPL-3.0**): YOLOv8s from [ultralytics](https://github.com/ultralytics/ultralytics),
siamese from [bubbliiiing/Siamese-pytorch](https://github.com/bubbliiiing/Siamese-pytorch),
API ideas from [Amorter/biliTicker_gt](https://github.com/Amorter/biliTicker_gt).