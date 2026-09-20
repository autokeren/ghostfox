"""
Ghostfox GeeTest v3 icon-click solver — "the eyes"
Models: yolov8s.onnx (char detection) + siamese.onnx (strip->field matching)
ships with ghostfox under models/geetest_click/
"""
import json
import time
import os
import subprocess
import sys
import io
import urllib.request

sys.path.insert(0, '/tmp/opencode/geetest-v3-click-crack')

MODEL_DIR = '/data/ghostfox/dist/ghostfox/models/geetest_click'


def get_model():
    import onnxruntime
    import cv2
    import numpy as np

    class GtModel:
        def __init__(self):
            self.img = None
            self.yolo = onnxruntime.InferenceSession(os.path.join(MODEL_DIR, 'yolov8s.onnx'))
            self.siam_sess = onnxruntime.InferenceSession(os.path.join(MODEL_DIR, 'siamese.onnx'))

        def detect(self, img_bytes):
            conf_thres = 0.8
            iou_thres = 0.8
            inputs = self.yolo.get_inputs()
            in_w = inputs[0].shape[2]
            in_h = inputs[0].shape[3]
            self.img = cv2.imdecode(np.frombuffer(img_bytes, np.uint8), cv2.IMREAD_ANYCOLOR)
            ih, iw = self.img.shape[:2]
            im = cv2.cvtColor(self.img, cv2.COLOR_BGR2RGB)
            im = cv2.resize(im, (in_h, in_w))
            data = np.transpose(np.array(im) / 255.0, (2, 0, 1))
            data = np.expand_dims(data, axis=0).astype(np.float32)
            out = self.yolo.run(None, {inputs[0].name: data})
            outputs = np.transpose(np.squeeze(out[0]))
            xf = iw / in_w
            yf = ih / in_h
            boxes, scores, cls_ids = [], [], []
            for i in range(outputs.shape[0]):
                cs = outputs[i][4:]
                mx = np.amax(cs)
                if mx >= conf_thres:
                    x, y, w, h = outputs[i][0], outputs[i][1], outputs[i][2], outputs[i][3]
                    boxes.append([int((x - w / 2) * xf), int((y - h / 2) * yf),
                                  int(w * xf), int(h * yf)])
                    scores.append(mx)
                    cls_ids.append(int(np.argmax(cs)))
            idxs = cv2.dnn.NMSBoxes(boxes, scores, conf_thres, iou_thres)
            smalls, bigs = {}, []
            for i in idxs:
                b = boxes[i]
                crop = self.img[b[1]:b[1] + b[3], b[0]:b[0] + b[2]]
                if crop.shape[0] < 35 and crop.shape[1] < 35:
                    smalls[b[0]] = crop
                else:
                    bigs.append(b)
            return smalls, bigs

        @staticmethod
        def _prep(im, size=(105, 105)):
            r = cv2.resize(im, size)
            return np.expand_dims(np.transpose(np.array(r) / 255.0, (2, 0, 1)), axis=0).astype(np.float32)

        def siamese(self, smalls, bigs):
            prepped = {x: self._prep(smalls[x]) for x in sorted(smalls)}
            res = []
            for x in sorted(prepped):
                d1 = prepped[x]
                for b in bigs:
                    if [b[0], b[1]] in res:
                        continue
                    crop = self.img[b[1]:b[1] + b[3], b[0]:b[0] + b[2]]
                    d2 = self._prep(crop)
                    out = self.siam_sess.run(None, {'input': d1, 'input.53': d2})
                    sig = 1 / (1 + np.exp(-out[0]))
                    if sig[0][0] >= 0.1:
                        res.append([b[0], b[1]])
                        break
            return res

    return GtModel()


def solve_image(img_bytes):
    """Solve a challenge image (PNG/JPEG bytes of the 344x384 composite).
    Returns list of [x0, y0] top-left boxes in RAW image coordinates, in CLICK ORDER."""
    m = get_model()
    smalls, bigs = m.detect(img_bytes)
    return m.siamese(smalls, bigs)


def solve_api(gt, challenge, max_retries=6):
    """Full API-level solve: returns {'result': 'success', 'validate': ...}"""
    sys.path.insert(0, '/tmp/opencode/geetest-v3-click-crack')
    os.chdir('/tmp/opencode/geetest-v3-click-crack')
    from crack import Crack
    crack = Crack(gt, challenge)
    crack.gettype()
    crack.get_c_s()
    time.sleep(0.5)
    crack.ajax()
    from model import Model
    m = Model()
    for retry in range(max_retries):
        pic = crack.get_pic(retry)
        ttt = time.time()
        smalls, bigs = m.detect(pic)
        matches = m.siamese(smalls, bigs)
        points = [f"{round((i[0] + 30) / 333 * 10000)}_{round((i[1] + 30) / 333 * 10000)}" for i in matches]
        wait = 2.0 - (time.time() - ttt)
        if wait > 0:
            time.sleep(wait)
        raw = crack.verify(points)
        if isinstance(raw, str):
            res = json.loads(raw) if raw.strip() else {}
        else:
            res = raw
        if res.get('data', {}).get('result') == 'success':
            return {'result': 'success', 'validate': res['data']['validate'],
                    'points': points, 'retry': retry}
        time.sleep(0.3)
    return {'result': 'fail', 'retries': max_retries}


if __name__ == '__main__':
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument('--image', help='path to challenge image')
    ap.add_argument('--gt')
    ap.add_argument('--challenge')
    args = ap.parse_args()
    if args.image:
        with open(args.image, 'rb') as f:
            print(json.dumps(solve_image(f.read())))
    elif args.gt and args.challenge:
        print(json.dumps(solve_api(args.gt, args.challenge)))
    else:
        ap.print_help()