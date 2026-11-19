#include "exact_matvec.hpp"
#include "backend.hpp"
#include "ggml-cpu.h"

#include <algorithm>
#include <cstdint>

namespace pk {
namespace {

// Op 1: convert each activation column to the weight's vec_dot_type, the same
// per-row from_float call ggml makes before its N = 1 matmul. The conversion is
// independent per block, so splitting by column gives the same bytes as ggml's
// split by block range.
void op_convert(ggml_tensor* dst, int ith, int nth, void*) {
    const ggml_tensor* x = dst->src[0];
    const ggml_tensor* W = dst->src[1];
    const ggml_type vdt = ggml_get_type_traits_cpu(W->type)->vec_dot_type;
    const ggml_from_float_t from_float = ggml_get_type_traits_cpu(vdt)->from_float;
    const int64_t K = x->ne[0], N = x->ne[1];
    const size_t row = ggml_row_size(vdt, K);
    for (int64_t c = ith; c < N; c += nth)
        from_float(reinterpret_cast<const float*>(static_cast<const char*>(x->data) + c * x->nb[1]),
                   static_cast<char*>(dst->data) + c * row, K);
}

// Op 2: dst[r, c] = vec_dot(W row r, activation column c). Threads split the
// weight rows, so each row is read once for all columns.
void op_matmul(ggml_tensor* dst, int ith, int nth, void*) {
    const ggml_tensor* act = dst->src[0];
    const ggml_tensor* W = dst->src[1];
    const ggml_type_traits_cpu* tr = ggml_get_type_traits_cpu(W->type);
    const int64_t K = W->ne[0], M = W->ne[1], N = dst->ne[1];
    const bool converted = W->type != tr->vec_dot_type;
    const size_t act_stride = converted ? ggml_row_size(tr->vec_dot_type, K) : act->nb[1];
    const int64_t r0 = M * ith / nth, r1 = M * (ith + 1) / nth;
    constexpr int64_t kBlock = 16;
    for (int64_t rb = r0; rb < r1; rb += kBlock) {
        const int64_t re = std::min(rb + kBlock, r1);
        for (int64_t c = 0; c < N; ++c) {
            const char* a = static_cast<const char*>(act->data) + c * act_stride;
            float* out = reinterpret_cast<float*>(static_cast<char*>(dst->data) + c * dst->nb[1]);
            for (int64_t r = rb; r < re; ++r)
                tr->vec_dot((int)K, out + r, 0, static_cast<const char*>(W->data) + r * W->nb[1], 0, a, 0, 1);
        }
    }
}

} // namespace

ggml_tensor* mul_mat_cols_exact(ggml_context* ctx, ggml_tensor* W, ggml_tensor* x) {
    const bool plain = x->type == GGML_TYPE_F32 && x->ne[1] > 1 && ggml_n_dims(x) == 2
                    && ggml_n_dims(W) == 2 && W->ne[0] == x->ne[0] && ggml_is_contiguous(x)
                    && ggml_is_contiguous_rows(W) && W->nb[0] == ggml_type_size(W->type)
                    && ggml_get_type_traits_cpu(W->type)->vec_dot != nullptr
                    && x->ne[0] % ggml_blck_size(ggml_get_type_traits_cpu(W->type)->vec_dot_type) == 0
                    && active_backend_is_cpu();
    if (!plain) return ggml_mul_mat(ctx, W, x);
    const ggml_type vdt = ggml_get_type_traits_cpu(W->type)->vec_dot_type;
    ggml_tensor* act = x;
    if (vdt != GGML_TYPE_F32) {
        ggml_tensor* args[2] = {x, W};
        act = ggml_custom_4d(ctx, GGML_TYPE_I8, (int64_t)ggml_row_size(vdt, x->ne[0]), x->ne[1], 1, 1,
                             args, 2, op_convert, GGML_N_TASKS_MAX, nullptr);
    }
    ggml_tensor* args[2] = {act, W};
    return ggml_custom_4d(ctx, GGML_TYPE_F32, W->ne[1], x->ne[1], 1, 1, args, 2, op_matmul,
                          GGML_N_TASKS_MAX, nullptr);
}

} // namespace pk
