#include "ternary_kernels.hpp"

#if (defined(__x86_64__) || defined(__i386__)) && (defined(__GNUC__) || defined(__clang__))
#include <immintrin.h>

#include <algorithm>
#include <cstring>

namespace pk {
namespace {

#ifndef PK_FUSED_C
#define PK_FUSED_C 2
#endif
#define PK_TGT_AVX2 __attribute__((target("avx2")))
#define PK_TGT_512 __attribute__((target("avx2,avx512f,avx512vl,avx512vnni")))
#define PK_TGT_512F __attribute__((target("avx2,avx512f")))

// Both kernels compute the same math as ternary_matmul_rows_ref. One vector
// lane holds one output row of a 16-row block (see TernaryWeight), so the
// per-group integer sums of 16 rows come out in one vector without any
// horizontal reduction. For each output element the float update is exactly
// the reference sequence: acc = acc + scale * (float)(sum - group_sum), in group
// order, then acc * act_scale (separate multiply and add, no FMA).

inline int32_t load_i32(const int8_t* p) {
    int32_t v;
    std::memcpy(&v, p, 4);
    return v;
}

// Row mask of block b for the output range [r0, r1).
inline unsigned block_mask(int b, int r0, int r1) {
    const int lo = std::max(r0 - b * kTernaryRowBlock, 0);
    const int hi = std::min(r1 - b * kTernaryRowBlock, kTernaryRowBlock);
    return hi <= lo ? 0u : ((1u << hi) - 1u) & ~((1u << lo) - 1u);
}

// ---------------------------------------------------------------------------
// AVX-512 VNNI: RB row blocks (16 rows each, one zmm) by C activation rows.
template <int RB, int C>
PK_TGT_512 void tile_512(const TernaryWeight& w, const uint8_t* act, size_t rb, int t0, float* y,
                         int b0, const unsigned* mk) {
    const int N = w.N, K = w.K, G = w.groups();
    const __m512i m3 = _mm512_set1_epi8(3);
    const int8_t* xr[C];
    const int32_t* gsr[C];
    for (int c = 0; c < C; ++c) {
        const uint8_t* row = act + (size_t)(t0 + c) * rb;
        xr[c] = reinterpret_cast<const int8_t*>(row);
        gsr[c] = reinterpret_cast<const int32_t*>(row + K + 4);
    }
    __m512 facc[RB][C];
    for (int b = 0; b < RB; ++b)
        for (int c = 0; c < C; ++c) facc[b][c] = _mm512_setzero_ps();
    for (int g = 0; g < G; ++g) {
        __m512i ia[RB][C];
        for (int b = 0; b < RB; ++b)
            for (int c = 0; c < C; ++c) ia[b][c] = _mm512_setzero_si512();
        const uint8_t* pb[RB];
        for (int b = 0; b < RB; ++b) pb[b] = w.planes.data() + ((size_t)(b0 + b) * G + g) * 512;
        for (int s = 0; s < 8; ++s) {
            __m512i wv[RB][4];
            for (int b = 0; b < RB; ++b) {
                const __m512i v = _mm512_loadu_si512(pb[b] + s * 64);
                wv[b][0] = _mm512_and_si512(v, m3);
                wv[b][1] = _mm512_and_si512(_mm512_srli_epi32(v, 2), m3);
                wv[b][2] = _mm512_and_si512(_mm512_srli_epi32(v, 4), m3);
                wv[b][3] = _mm512_and_si512(_mm512_srli_epi32(v, 6), m3);
            }
            for (int c = 0; c < C; ++c) {
                const int8_t* x = xr[c] + (size_t)g * 128 + s * 16;
                for (int p = 0; p < 4; ++p) {
                    const __m512i a = _mm512_set1_epi32(load_i32(x + 4 * p));
                    for (int b = 0; b < RB; ++b) ia[b][c] = _mm512_dpbusd_epi32(ia[b][c], wv[b][p], a);
                }
            }
        }
        for (int b = 0; b < RB; ++b) {
            const __m512 sc = _mm512_loadu_ps(&w.scales[((size_t)(b0 + b) * G + g) * kTernaryRowBlock]);
            for (int c = 0; c < C; ++c) {
                const __m512 f = _mm512_cvtepi32_ps(_mm512_sub_epi32(ia[b][c], _mm512_set1_epi32(gsr[c][g])));
                facc[b][c] = _mm512_add_ps(facc[b][c], _mm512_mul_ps(sc, f));
            }
        }
    }
    for (int c = 0; c < C; ++c) {
        const __m512 sa = _mm512_set1_ps(*reinterpret_cast<const float*>(xr[c] + K));
        for (int b = 0; b < RB; ++b)
            _mm512_mask_storeu_ps(y + (size_t)(t0 + c) * N + (size_t)(b0 + b) * kTernaryRowBlock,
                                  (__mmask16)mk[b], _mm512_mul_ps(facc[b][c], sa));
    }
}

template <int RB, int C>
PK_TGT_512 void cols_512(const TernaryWeight& w, const uint8_t* act, size_t rb, int T, float* y, int b0,
                         const unsigned* mk) {
    int t0 = 0;
    for (; t0 + C <= T; t0 += C) tile_512<RB, C>(w, act, rb, t0, y, b0, mk);
    for (; t0 < T; ++t0) tile_512<RB, 1>(w, act, rb, t0, y, b0, mk);
}

PK_TGT_512 void rows_512(const TernaryWeight& w, const uint8_t* act, int T, float* y, int r0, int r1) {
    if (r0 >= r1 || T <= 0) return;
    // 3 row blocks by 4 activation rows measured best on Zen 5 (12 int32 and
    // 12 float accumulators); 2 by 6 and 4 by 4 are within a few percent.
    constexpr int C = 4;
    const size_t rb = ternary_act_row_bytes(w.K);
    const int bl = (r1 - 1) / kTernaryRowBlock;
    for (int b = r0 / kTernaryRowBlock; b <= bl;) {
        unsigned mk[3];
        const int nb = std::min(3, bl - b + 1);
        for (int i = 0; i < nb; ++i) mk[i] = block_mask(b + i, r0, r1);
        if (nb == 3) cols_512<3, C>(w, act, rb, T, y, b, mk);
        else if (nb == 2) cols_512<2, C>(w, act, rb, T, y, b, mk);
        else cols_512<1, C>(w, act, rb, T, y, b, mk);
        b += nb;
    }
}

// ---------------------------------------------------------------------------
// AVX2: one row block as two ymm halves (rows 0-7, rows 8-15) by C activation
// rows. maddubs gives int16 pairs of at most 2 * 2 * 127 = 508 in magnitude,
// and a group adds 32 of them per lane, at most 16256, so a whole group
// accumulates exactly in int16 before one widening madd.
template <int C>
PK_TGT_AVX2 void tile_avx2(const TernaryWeight& w, const uint8_t* act, size_t rb, int t0, float* y, int b0,
                           unsigned mk) {
    const int N = w.N, K = w.K, G = w.groups();
    const __m256i m3 = _mm256_set1_epi8(3);
    const __m256i ones = _mm256_set1_epi16(1);
    const int8_t* xr[C];
    const int32_t* gsr[C];
    for (int c = 0; c < C; ++c) {
        const uint8_t* row = act + (size_t)(t0 + c) * rb;
        xr[c] = reinterpret_cast<const int8_t*>(row);
        gsr[c] = reinterpret_cast<const int32_t*>(row + K + 4);
    }
    alignas(32) float facc[C][16] = {};
    for (int g = 0; g < G; ++g) {
        __m256i ia[C][2];
        for (int c = 0; c < C; ++c) ia[c][0] = ia[c][1] = _mm256_setzero_si256();
        const uint8_t* pb = w.planes.data() + ((size_t)b0 * G + g) * 512;
        for (int s = 0; s < 8; ++s) {
            const __m256i v0 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(pb + s * 64));
            const __m256i v1 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(pb + s * 64 + 32));
            for (int p = 0; p < 4; ++p) {
                const __m256i w0 = _mm256_and_si256(_mm256_srli_epi32(v0, 2 * p), m3);
                const __m256i w1 = _mm256_and_si256(_mm256_srli_epi32(v1, 2 * p), m3);
                for (int c = 0; c < C; ++c) {
                    const __m256i a = _mm256_set1_epi32(load_i32(xr[c] + (size_t)g * 128 + s * 16 + 4 * p));
                    ia[c][0] = _mm256_add_epi16(ia[c][0], _mm256_maddubs_epi16(w0, a));
                    ia[c][1] = _mm256_add_epi16(ia[c][1], _mm256_maddubs_epi16(w1, a));
                }
            }
        }
        const float* scp = &w.scales[((size_t)b0 * G + g) * kTernaryRowBlock];
        const __m256 sc0 = _mm256_loadu_ps(scp), sc1 = _mm256_loadu_ps(scp + 8);
        for (int c = 0; c < C; ++c) {
            const __m256i gs = _mm256_set1_epi32(gsr[c][g]);
            const __m256 f0 = _mm256_cvtepi32_ps(_mm256_sub_epi32(_mm256_madd_epi16(ia[c][0], ones), gs));
            const __m256 f1 = _mm256_cvtepi32_ps(_mm256_sub_epi32(_mm256_madd_epi16(ia[c][1], ones), gs));
            _mm256_store_ps(facc[c], _mm256_add_ps(_mm256_load_ps(facc[c]), _mm256_mul_ps(sc0, f0)));
            _mm256_store_ps(facc[c] + 8, _mm256_add_ps(_mm256_load_ps(facc[c] + 8), _mm256_mul_ps(sc1, f1)));
        }
    }
    for (int c = 0; c < C; ++c) {
        const __m256 sa = _mm256_set1_ps(*reinterpret_cast<const float*>(xr[c] + K));
        alignas(32) float out[16];
        _mm256_store_ps(out, _mm256_mul_ps(_mm256_load_ps(facc[c]), sa));
        _mm256_store_ps(out + 8, _mm256_mul_ps(_mm256_load_ps(facc[c] + 8), sa));
        float* yr = y + (size_t)(t0 + c) * N + (size_t)b0 * kTernaryRowBlock;
        if (mk == 0xFFFFu) std::memcpy(yr, out, sizeof(out));
        else
            for (int i = 0; i < kTernaryRowBlock; ++i)
                if (mk >> i & 1u) yr[i] = out[i];
    }
}

PK_TGT_AVX2 void rows_avx2(const TernaryWeight& w, const uint8_t* act, int T, float* y, int r0, int r1) {
    if (r0 >= r1 || T <= 0) return;
    constexpr int C = 2;
    const size_t rb = ternary_act_row_bytes(w.K);
    const int bl = (r1 - 1) / kTernaryRowBlock;
    for (int b = r0 / kTernaryRowBlock; b <= bl; ++b) {
        const unsigned mk = block_mask(b, r0, r1);
        int t0 = 0;
        for (; t0 + C <= T; t0 += C) tile_avx2<C>(w, act, rb, t0, y, b, mk);
        for (; t0 < T; ++t0) tile_avx2<1>(w, act, rb, t0, y, b, mk);
    }
}


// ---------------------------------------------------------------------------
// AVX2, both planes of a five-value linear in one pass. Each plane keeps its
// own int16 group accumulators and its own float accumulator, exactly as in
// tile_avx2, so every value equals the single-plane result; the two scaled
// results are added once, at the end, as the separate graph add did. What is
// shared is the broadcast of the activation bytes and the loop and address
// work, and there is no intermediate output row to write and read back.
template <int C>
PK_TGT_AVX2 void tile2_avx2(const TernaryWeight& wl, const TernaryWeight& wh, const uint8_t* act, size_t rb,
                            int t0, float* y, int b0, unsigned mk) {
    const int N = wl.N, K = wl.K, G = wl.groups();
    const __m256i m3 = _mm256_set1_epi8(3);
    const __m256i ones = _mm256_set1_epi16(1);
    const int8_t* xr[C];
    const int32_t* gsr[C];
    for (int c = 0; c < C; ++c) {
        const uint8_t* row = act + (size_t)(t0 + c) * rb;
        xr[c] = reinterpret_cast<const int8_t*>(row);
        gsr[c] = reinterpret_cast<const int32_t*>(row + K + 4);
    }
    alignas(32) float facc[2][C][16] = {};
    for (int g = 0; g < G; ++g) {
        __m256i ia[2][C][2];
        for (int q = 0; q < 2; ++q)
            for (int c = 0; c < C; ++c) ia[q][c][0] = ia[q][c][1] = _mm256_setzero_si256();
        const uint8_t* pl = wl.planes.data() + ((size_t)b0 * G + g) * 512;
        const uint8_t* ph = wh.planes.data() + ((size_t)b0 * G + g) * 512;
        for (int s = 0; s < 8; ++s) {
            const __m256i l0 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(pl + s * 64));
            const __m256i l1 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(pl + s * 64 + 32));
            const __m256i h0 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(ph + s * 64));
            const __m256i h1 = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(ph + s * 64 + 32));
            for (int p = 0; p < 4; ++p) {
                const __m256i wl0 = _mm256_and_si256(_mm256_srli_epi32(l0, 2 * p), m3);
                const __m256i wl1 = _mm256_and_si256(_mm256_srli_epi32(l1, 2 * p), m3);
                const __m256i wh0 = _mm256_and_si256(_mm256_srli_epi32(h0, 2 * p), m3);
                const __m256i wh1 = _mm256_and_si256(_mm256_srli_epi32(h1, 2 * p), m3);
                for (int c = 0; c < C; ++c) {
                    const __m256i a = _mm256_set1_epi32(load_i32(xr[c] + (size_t)g * 128 + s * 16 + 4 * p));
                    ia[0][c][0] = _mm256_add_epi16(ia[0][c][0], _mm256_maddubs_epi16(wl0, a));
                    ia[0][c][1] = _mm256_add_epi16(ia[0][c][1], _mm256_maddubs_epi16(wl1, a));
                    ia[1][c][0] = _mm256_add_epi16(ia[1][c][0], _mm256_maddubs_epi16(wh0, a));
                    ia[1][c][1] = _mm256_add_epi16(ia[1][c][1], _mm256_maddubs_epi16(wh1, a));
                }
            }
        }
        const float* scp[2] = {&wl.scales[((size_t)b0 * G + g) * kTernaryRowBlock],
                               &wh.scales[((size_t)b0 * G + g) * kTernaryRowBlock]};
        for (int q = 0; q < 2; ++q) {
            const __m256 sc0 = _mm256_loadu_ps(scp[q]), sc1 = _mm256_loadu_ps(scp[q] + 8);
            for (int c = 0; c < C; ++c) {
                const __m256i gs = _mm256_set1_epi32(gsr[c][g]);
                const __m256 f0 = _mm256_cvtepi32_ps(_mm256_sub_epi32(_mm256_madd_epi16(ia[q][c][0], ones), gs));
                const __m256 f1 = _mm256_cvtepi32_ps(_mm256_sub_epi32(_mm256_madd_epi16(ia[q][c][1], ones), gs));
                _mm256_store_ps(facc[q][c], _mm256_add_ps(_mm256_load_ps(facc[q][c]), _mm256_mul_ps(sc0, f0)));
                _mm256_store_ps(facc[q][c] + 8,
                                _mm256_add_ps(_mm256_load_ps(facc[q][c] + 8), _mm256_mul_ps(sc1, f1)));
            }
        }
    }
    for (int c = 0; c < C; ++c) {
        const __m256 sa = _mm256_set1_ps(*reinterpret_cast<const float*>(xr[c] + K));
        alignas(32) float out[16];
        for (int h = 0; h < 2; ++h) {
            const __m256 pa = _mm256_mul_ps(_mm256_load_ps(facc[0][c] + 8 * h), sa);
            const __m256 pb = _mm256_mul_ps(_mm256_load_ps(facc[1][c] + 8 * h), sa);
            _mm256_store_ps(out + 8 * h, _mm256_add_ps(pa, pb));
        }
        float* yr = y + (size_t)(t0 + c) * N + (size_t)b0 * kTernaryRowBlock;
        if (mk == 0xFFFFu) std::memcpy(yr, out, sizeof(out));
        else
            for (int i = 0; i < kTernaryRowBlock; ++i)
                if (mk >> i & 1u) yr[i] = out[i];
    }
}

PK_TGT_AVX2 void rows2_avx2(const TernaryWeight& wl, const TernaryWeight& wh, const uint8_t* act, int T,
                            float* y, int r0, int r1) {
    if (r0 >= r1 || T <= 0) return;
    constexpr int C = PK_FUSED_C;
    const size_t rb = ternary_act_row_bytes(wl.K);
    const int bl = (r1 - 1) / kTernaryRowBlock;
    for (int b = r0 / kTernaryRowBlock; b <= bl; ++b) {
        const unsigned mk = block_mask(b, r0, r1);
        int t0 = 0;
        for (; t0 + C <= T; t0 += C) tile2_avx2<C>(wl, wh, act, rb, t0, y, b, mk);
        for (; t0 < T; ++t0) tile2_avx2<1>(wl, wh, act, rb, t0, y, b, mk);
    }
}

// ---------------------------------------------------------------------------
// Activation quantization, byte-identical to ternary_quant_rows_ref. The max
// of |x| is exact in any order, and max_ps(v, m) returns m for a NaN v, as
// std::max(amax, NaN) keeps amax. The scale and its inverse are the same
// scalar float divisions. The product is the same float multiply; the clamp
// uses max_ps(f, -127) then min_ps(f, 127), which match std::max(-127.f, f)
// and std::min(127.f, f) including NaN; cvtps rounds to nearest even like
// lrintf in the default rounding mode. Group sums are exact integers.
PK_TGT_512F void quant_avx512(const float* x, int K, int t0, int t1, uint8_t* act) {
    const int G = K / kTernaryGroup;
    const size_t rb = ternary_act_row_bytes(K);
    const __m512 lo = _mm512_set1_ps(-127.0f), hi = _mm512_set1_ps(127.0f);
    for (int t = t0; t < t1; ++t) {
        const float* xr = x + (size_t)t * K;
        uint8_t* row = act + (size_t)t * rb;
        __m512 m = _mm512_setzero_ps();
        for (int k = 0; k < K; k += 16) m = _mm512_max_ps(_mm512_abs_ps(_mm512_loadu_ps(xr + k)), m);
        const float amax = _mm512_reduce_max_ps(m);
        const float sc = amax > 0.0f ? amax / 127.0f : 0.0f;
        const float inv = sc > 0.0f ? 1.0f / sc : 0.0f;
        std::memcpy(row + K, &sc, 4);
        const __m512 vinv = _mm512_set1_ps(inv);
        for (int g = 0; g < G; ++g) {
            __m512i sum = _mm512_setzero_si512();
            for (int j = 0; j < kTernaryGroup; j += 16) {
                const int k = g * kTernaryGroup + j;
                __m512 f = _mm512_mul_ps(_mm512_loadu_ps(xr + k), vinv);
                f = _mm512_min_ps(_mm512_max_ps(f, lo), hi);
                const __m512i v = _mm512_cvtps_epi32(f);
                sum = _mm512_add_epi32(sum, v);
                _mm_storeu_si128(reinterpret_cast<__m128i*>(row + k), _mm512_cvtepi32_epi8(v));
            }
            const int32_t gs = _mm512_reduce_add_epi32(sum);
            std::memcpy(row + K + 4 + 4 * (size_t)g, &gs, 4);
        }
    }
}

PK_TGT_AVX2 void quant_avx2(const float* x, int K, int t0, int t1, uint8_t* act) {
    const int G = K / kTernaryGroup;
    const size_t rb = ternary_act_row_bytes(K);
    const __m256 lo = _mm256_set1_ps(-127.0f), hi = _mm256_set1_ps(127.0f);
    const __m256 absm = _mm256_castsi256_ps(_mm256_set1_epi32(0x7fffffff));
    const __m256i perm = _mm256_setr_epi32(0, 4, 1, 5, 2, 6, 3, 7);
    for (int t = t0; t < t1; ++t) {
        const float* xr = x + (size_t)t * K;
        uint8_t* row = act + (size_t)t * rb;
        __m256 m = _mm256_setzero_ps();
        for (int k = 0; k < K; k += 8) m = _mm256_max_ps(_mm256_and_ps(_mm256_loadu_ps(xr + k), absm), m);
        alignas(32) float mv[8];
        _mm256_store_ps(mv, m);
        float amax = 0.0f;
        for (float v : mv) amax = std::max(amax, v);
        const float sc = amax > 0.0f ? amax / 127.0f : 0.0f;
        const float inv = sc > 0.0f ? 1.0f / sc : 0.0f;
        std::memcpy(row + K, &sc, 4);
        const __m256 vinv = _mm256_set1_ps(inv);
        for (int g = 0; g < G; ++g) {
            __m256i sum = _mm256_setzero_si256();
            for (int j = 0; j < kTernaryGroup; j += 32) {
                const int k = g * kTernaryGroup + j;
                __m256i v[4];
                for (int u = 0; u < 4; ++u) {
                    __m256 f = _mm256_mul_ps(_mm256_loadu_ps(xr + k + 8 * u), vinv);
                    f = _mm256_min_ps(_mm256_max_ps(f, lo), hi);
                    v[u] = _mm256_cvtps_epi32(f);
                    sum = _mm256_add_epi32(sum, v[u]);
                }
                // values are within [-127, 127], so the saturating packs are exact
                const __m256i p16a = _mm256_packs_epi32(v[0], v[1]);
                const __m256i p16b = _mm256_packs_epi32(v[2], v[3]);
                const __m256i p8 = _mm256_permutevar8x32_epi32(_mm256_packs_epi16(p16a, p16b), perm);
                _mm256_storeu_si256(reinterpret_cast<__m256i*>(row + k), p8);
            }
            __m128i s4 = _mm_add_epi32(_mm256_castsi256_si128(sum), _mm256_extracti128_si256(sum, 1));
            s4 = _mm_add_epi32(s4, _mm_shuffle_epi32(s4, _MM_SHUFFLE(1, 0, 3, 2)));
            s4 = _mm_add_epi32(s4, _mm_shuffle_epi32(s4, _MM_SHUFFLE(2, 3, 0, 1)));
            const int32_t gs = _mm_cvtsi128_si32(s4);
            std::memcpy(row + K + 4 + 4 * (size_t)g, &gs, 4);
        }
    }
}

}  // namespace

const TernaryKernel* ternary_kernel_x86_vnni() {
    static const TernaryKernel k{"vnni", rows_512};
    __builtin_cpu_init();
    const bool ok = __builtin_cpu_supports("avx512f") && __builtin_cpu_supports("avx512vl") &&
                    __builtin_cpu_supports("avx512vnni");
    return ok ? &k : nullptr;
}

const TernaryKernel* ternary_kernel_x86_avx2() {
    static const TernaryKernel k{"avx2", rows_avx2, rows2_avx2};
    __builtin_cpu_init();
    return __builtin_cpu_supports("avx2") ? &k : nullptr;
}

const TernaryQuant* ternary_quant_x86_avx512() {
    static const TernaryQuant q{"avx512", quant_avx512};
    __builtin_cpu_init();
    return __builtin_cpu_supports("avx512f") ? &q : nullptr;
}

const TernaryQuant* ternary_quant_x86_avx2() {
    static const TernaryQuant q{"avx2", quant_avx2};
    __builtin_cpu_init();
    return __builtin_cpu_supports("avx2") ? &q : nullptr;
}

}  // namespace pk
#else
namespace pk {
const TernaryKernel* ternary_kernel_x86_vnni() { return nullptr; }
const TernaryKernel* ternary_kernel_x86_avx2() { return nullptr; }
const TernaryQuant* ternary_quant_x86_avx512() { return nullptr; }
const TernaryQuant* ternary_quant_x86_avx2() { return nullptr; }
}  // namespace pk
#endif
