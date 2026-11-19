#pragma once
#include <vector>

#include "ternary.hpp"

namespace pk {

using TernaryRowsFn = void (*)(const TernaryWeight&, const uint8_t* act, int T, float* y,
                               int r0, int r1);
// Both planes of a five-value linear in one pass: y = rows(lo) + rows(hi), with
// the sum taken per output element after each plane's own scaling, so the bytes
// equal those of two single-plane calls added together. lo and hi have the same
// N and K.
using TernaryRows2Fn = void (*)(const TernaryWeight& lo, const TernaryWeight& hi, const uint8_t* act,
                                int T, float* y, int r0, int r1);
struct TernaryKernel {
    const char* name;
    TernaryRowsFn fn;
    TernaryRows2Fn fn2 = nullptr;   // null: this kernel has no fused two-plane version
};

using TernaryQuantFn = void (*)(const float* x, int K, int t0, int t1, uint8_t* act);
struct TernaryQuant {
    const char* name;
    TernaryQuantFn fn;
};

const TernaryKernel* ternary_kernel_x86_vnni();
const TernaryKernel* ternary_kernel_x86_avx2();
const TernaryKernel* ternary_kernel_neon();
std::vector<const TernaryKernel*> ternary_all_kernels();

const TernaryQuant* ternary_quant_x86_avx512();
const TernaryQuant* ternary_quant_x86_avx2();
std::vector<const TernaryQuant*> ternary_all_quants();

}  // namespace pk
