#include "ternary_kernels.hpp"

#if defined(__aarch64__) && defined(__ARM_FEATURE_DOTPROD)
#include <arm_neon.h>

#include <algorithm>
#if defined(__linux__)
#include <sys/auxv.h>
#ifndef HWCAP_ASIMDDP
#define HWCAP_ASIMDDP (1 << 20)
#endif
#endif

namespace pk {
namespace {

// One int32x4 lane per output row: a 16-row block is four vectors. sdot by
// lane multiplies the codes (0..2, as int8) of 4 rows by 4 elements with one
// broadcast group of 4 activations. Same math as ternary_matmul_rows_ref:
// per group an exact int32 sum, then acc = acc + scale * (float)(sum - group_sum)
// in group order (separate multiply and add), then acc * act_scale.
template <int C>
void tile_neon(const TernaryWeight& w, const uint8_t* act, size_t rb, int t0, float* y, int b0, int r0,
               int r1) {
    const int N = w.N, K = w.K, G = w.groups();
    const uint8x16_t m3 = vdupq_n_u8(3);
    const int8_t* xr[C];
    const int32_t* gsr[C];
    for (int c = 0; c < C; ++c) {
        const uint8_t* row = act + (size_t)(t0 + c) * rb;
        xr[c] = reinterpret_cast<const int8_t*>(row);
        gsr[c] = reinterpret_cast<const int32_t*>(row + K + 4);
    }
    float32x4_t facc[C][4];
    for (int c = 0; c < C; ++c)
        for (int q = 0; q < 4; ++q) facc[c][q] = vdupq_n_f32(0.0f);
    for (int g = 0; g < G; ++g) {
        int32x4_t ia[C][4];
        for (int c = 0; c < C; ++c)
            for (int q = 0; q < 4; ++q) ia[c][q] = vdupq_n_s32(0);
        const uint8_t* pb = w.planes.data() + ((size_t)b0 * G + g) * 512;
        for (int s = 0; s < 8; ++s) {
            uint8x16_t v[4];
            for (int q = 0; q < 4; ++q) v[q] = vld1q_u8(pb + s * 64 + 16 * q);
            int8x16_t xa[C];
            for (int c = 0; c < C; ++c) xa[c] = vld1q_s8(xr[c] + (size_t)g * 128 + s * 16);
            for (int q = 0; q < 4; ++q) {
                const int8x16_t w0 = vreinterpretq_s8_u8(vandq_u8(v[q], m3));
                const int8x16_t w1 = vreinterpretq_s8_u8(vandq_u8(vshrq_n_u8(v[q], 2), m3));
                const int8x16_t w2 = vreinterpretq_s8_u8(vandq_u8(vshrq_n_u8(v[q], 4), m3));
                const int8x16_t w3 = vreinterpretq_s8_u8(vshrq_n_u8(v[q], 6));
                for (int c = 0; c < C; ++c) {
                    ia[c][q] = vdotq_laneq_s32(ia[c][q], w0, xa[c], 0);
                    ia[c][q] = vdotq_laneq_s32(ia[c][q], w1, xa[c], 1);
                    ia[c][q] = vdotq_laneq_s32(ia[c][q], w2, xa[c], 2);
                    ia[c][q] = vdotq_laneq_s32(ia[c][q], w3, xa[c], 3);
                }
            }
        }
        const float* scp = &w.scales[((size_t)b0 * G + g) * kTernaryRowBlock];
        for (int c = 0; c < C; ++c) {
            const int32x4_t gs = vdupq_n_s32(gsr[c][g]);
            for (int q = 0; q < 4; ++q) {
                const float32x4_t f = vcvtq_f32_s32(vsubq_s32(ia[c][q], gs));
                facc[c][q] = vaddq_f32(facc[c][q], vmulq_f32(vld1q_f32(scp + 4 * q), f));
            }
        }
    }
    for (int c = 0; c < C; ++c) {
        const float32x4_t sa = vdupq_n_f32(*reinterpret_cast<const float*>(xr[c] + K));
        float out[16];
        for (int q = 0; q < 4; ++q) vst1q_f32(out + 4 * q, vmulq_f32(facc[c][q], sa));
        float* yr = y + (size_t)(t0 + c) * N;
        const int lo = std::max(r0, b0 * kTernaryRowBlock);
        const int hi = std::min(r1, (b0 + 1) * kTernaryRowBlock);
        for (int n = lo; n < hi; ++n) yr[n] = out[n - b0 * kTernaryRowBlock];
    }
}

void rows_neon(const TernaryWeight& w, const uint8_t* act, int T, float* y, int r0, int r1) {
    if (r0 >= r1 || T <= 0) return;
    constexpr int C = 2;
    const size_t rb = ternary_act_row_bytes(w.K);
    const int bl = (r1 - 1) / kTernaryRowBlock;
    for (int b = r0 / kTernaryRowBlock; b <= bl; ++b) {
        int t0 = 0;
        for (; t0 + C <= T; t0 += C) tile_neon<C>(w, act, rb, t0, y, b, r0, r1);
        for (; t0 < T; ++t0) tile_neon<1>(w, act, rb, t0, y, b, r0, r1);
    }
}

}  // namespace

const TernaryKernel* ternary_kernel_neon() {
    static const TernaryKernel k{"neon", rows_neon};
#if defined(__linux__)
    if (!(getauxval(AT_HWCAP) & HWCAP_ASIMDDP)) return nullptr;
#endif
    return &k;
}

}  // namespace pk
#else
namespace pk {
const TernaryKernel* ternary_kernel_neon() { return nullptr; }
}  // namespace pk
#endif
