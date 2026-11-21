#include <metal_stdlib>
using namespace metal;

kernel void stamp(const device uint* source [[LOFT_SLOT(1,7)]],
                  device uint* output [[LOFT_SLOT(2,5)]],
                  texture2d<float, access::read> probe [[LOFT_SLOT(3,9)]]) {
    bfloat value = bfloat(probe.read(uint2(0)).r);
    uint count = source[1];
    for (uint i = 0; i < source[0]; ++i) { count += 2u; }
    output[0] = count + uint(float(value) * 32.0f);
    output[1] = 1u;
}
