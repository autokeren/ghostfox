"""Dequantize the siamese ONNX (dynamic-quantized VGG16) to pure float32.

The graph pattern per layer is:
  DynamicQuantizeLinear(x) -> (x_q u8, x_scale, x_zp)
  ConvInteger(x_q, w_q u8, x_zp, w_zp) -> int32
  Cast -> f32
  Mul(cast, x_scale * w_scale) -> conv
  Add(conv, bias)

Since per-tensor uniform quantization is linear, folding w_float =
(w_q - w_zp) * w_scale and running a float Conv(x, w_float) computes the
exact same result. All quantization machinery is removed.
"""
import onnx
import numpy as np
from onnx import helper, numpy_helper, TensorProto

SRC = '/data/ghostfox/dist/ghostfox/models/geetest_click/siamese.onnx'
DST = '/data/ghostfox/dist/ghostfox/models/geetest_click/siamese_float.onnx'

m = onnx.load(SRC)
g = m.graph

# 1. Dequantize all weight initializers
inits = {i.name: i for i in g.initializer}
w_deq = {}
for name, ini in list(inits.items()):
    if name.endswith('.weight_quantized'):
        prefix = name[: -len('weight_quantized')]
        scale_name = prefix + 'weight_scale'
        zp_name = prefix + 'weight_zero_point'
        if scale_name not in inits or zp_name not in inits:
            print('MISSING scale/zp for', name)
            continue
        w_q = numpy_helper.to_array(inits[name]).astype(np.float32)
        scale = float(numpy_helper.to_array(inits[scale_name]))
        zp = float(numpy_helper.to_array(inits[zp_name]))
        w_f = (w_q - zp) * scale
        w_deq[prefix + 'weight_dequant'] = w_f

new_nodes = []
R = {}  # rename map: removed-tensor-name -> surviving-tensor-name

def lookup(t):
    return R.get(t, t)

dead = set()

for n in g.node:
    if n.op_type == 'DynamicQuantizeLinear':
        # y quantized -> float input; scale/zp dead
        R[n.output[0]] = n.input[0]
        dead.add(n.output[1])
        dead.add(n.output[2])
        continue
    if n.op_type == 'Cast':
        R[n.output[0]] = lookup(n.input[0])
        continue
    if n.op_type == 'ConvInteger':
        w_name = n.input[1]
        w_deq_name = w_name[: -len('weight_quantized')] + 'weight_dequant'
        new_node = helper.make_node(
            'Conv',
            inputs=[lookup(n.input[0]), w_deq_name],
            outputs=[n.output[0]],
            name=n.name + '_float',
            **{a.name: helper.get_attribute_value(a) for a in n.attribute if a.name != 'dilations'}
        )
        for a in n.attribute:
            if a.name == 'dilations':
                new_node.attribute.append(helper.make_attribute('dilations', list(helper.get_attribute_value(a))))
        new_nodes.append(new_node)
        continue
    if n.op_type == 'MatMulInteger':
        w_name = n.input[1]
        w_deq_name = w_name[: -len('weight_quantized')] + 'weight_dequant'
        new_nodes.append(helper.make_node(
            'MatMul',
            inputs=[lookup(n.input[0]), w_deq_name],
            outputs=[n.output[0]],
            name=n.name + '_float',
        ))
        continue
    if n.op_type == 'Mul':
        def is_dead(t):
            if t in dead:
                return True
            lt = lookup(t)
            if lt in dead:
                return True
            return lt.endswith('.weight_scale') or lt.endswith('.weight_zero_point') or lt.endswith('.weight_quantized')
        i0, i1 = n.input[0], n.input[1]
        d0, d1 = is_dead(i0), is_dead(i1)
        if d0 and d1:
            dead.add(n.output[0])
            continue
        if d0:
            R[n.output[0]] = lookup(i1)
            continue
        if d1:
            R[n.output[0]] = lookup(i0)
            continue
        new_nodes.append(helper.make_node('Mul', inputs=[lookup(i0), lookup(i1)], outputs=[n.output[0]], name=n.name))
        continue
    # everything else: rewire inputs
    new_nodes.append(helper.make_node(
        n.op_type,
        inputs=[lookup(i) for i in n.input],
        outputs=[n.output[0]],
        name=n.name,
        **{a.name: helper.get_attribute_value(a) for a in n.attribute},
    ))

# 2. New initializer list: keep biases + add dequantized weights, drop quantized stuff
new_inits = []
for ini in g.initializer:
    if ini.name.endswith('.weight_quantized') or ini.name.endswith('.weight_scale') or ini.name.endswith('.weight_zero_point'):
        continue
    new_inits.append(ini)
for name, arr in w_deq.items():
    new_inits.append(numpy_helper.from_array(arr.astype(np.float32), name=name))

g.ClearField('node')
g.node.extend(new_nodes)
g.ClearField('initializer')
g.initializer.extend(new_inits)

# 3. Graph inputs stay: input + input.53
onnx.save(m, DST)
print('saved', DST)
print('nodes:', len(new_nodes), 'inits:', len(new_inits))
ops = {}
for n in new_nodes:
    ops[n.op_type] = ops.get(n.op_type, 0) + 1
print('ops:', ops)