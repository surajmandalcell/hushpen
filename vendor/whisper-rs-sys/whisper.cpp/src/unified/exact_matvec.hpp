#pragma once
#include "ggml.h"

namespace pk {

// y = W x for N columns, with every column bitwise equal to the N = 1 ggml_mul_mat.
//
// ggml picks a different matmul kernel when the right-hand side has more than one
// column, so the columns of a batched ggml_mul_mat are only close to the single
// column results. This builds a CPU custom op instead: for each weight row it
// calls the same vec_dot as the N = 1 path, once per column, on the activation
// converted to vec_dot_type exactly as ggml converts it. The weight row is still
// read once for all columns, which is where the batched speedup comes from.
//
//   W: [K, M], any 2D type with a CPU vec_dot (F32, F16, quantized).
//   x: [K, N] F32.
//   returns F32 [M, N].
//
// Falls back to ggml_mul_mat when N == 1 (already the reference), when the
// active backend is not the ggml CPU backend, or when the shapes are not the
// plain 2D case. Must be called from inside a graph build lambda.
ggml_tensor* mul_mat_cols_exact(ggml_context* ctx, ggml_tensor* W, ggml_tensor* x);

} // namespace pk
