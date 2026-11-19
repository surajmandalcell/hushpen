#pragma once
// Native ternary (1.58 bit) linear layers for moondream/parakeet-redux.
// See docs/superpowers/specs/2026-09-29-hf-ternary-vad-design.md.
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

struct ggml_context;
struct ggml_tensor;

namespace pk {

class ModelLoader;

constexpr int kTernaryGroup = 128;
constexpr int kTernaryRowBlock = 16;

// Repacked weight, N output rows by K input columns (K % 128 == 0). Rows are
// padded to a multiple of 16 with zero codes and zero scales, and stored in
// blocks of 16 rows so that a kernel can keep one output row per vector lane.
// planes: per row block b, per group g, per step s (0..7, 16 elements each),
// 64 bytes. Byte 4*i + j of a step holds, in bit pairs 0-1, 2-3, 4-5, 6-7
// (plane p = 0..3), the code (0,1,2) of row 16*b + i at element
// 128*g + 16*s + 4*p + j. scales: per row block, per group, 16 floats (one per
// row of the block). w[n][k] = scale(n, k/128) * (code(n, k) - 1).
struct TernaryWeight {
    int N = 0;
    int K = 0;
    std::vector<uint8_t> planes;   // row_blocks() * groups() * 512 bytes
    std::vector<float>   scales;   // row_blocks() * groups() * 16
    int groups() const { return K / kTernaryGroup; }
    int row_blocks() const { return (N + kTernaryRowBlock - 1) / kTernaryRowBlock; }
    // The layout above, spelled out. The scalar reference uses these.
    int code(int n, int k) const {
        const int b = n / kTernaryRowBlock, i = n % kTernaryRowBlock;
        const int g = k / kTernaryGroup, r = k % kTernaryGroup;
        const int s = r / 16, p = (r % 16) / 4, j = r % 4;
        const uint8_t byte = planes[((size_t)b * groups() + g) * 512 + (size_t)s * 64 + 4 * i + j];
        return (byte >> (2 * p)) & 3;
    }
    float scale(int n, int g) const {
        const int b = n / kTernaryRowBlock, i = n % kTernaryRowBlock;
        return scales[((size_t)b * groups() + g) * kTernaryRowBlock + i];
    }
};

// Upstream packing (5 trits per byte, base 3, LSD first) plus F16 group scales
// to the kernel layout. qweight is N rows of ceil(K/5) bytes.
void ternary_repack(const uint8_t* qweight, const uint16_t* scales_f16, int N, int K,
                    TernaryWeight& out);
// Float reference dequantization, W becomes N*K row-major.
void ternary_dequant(const uint8_t* qweight, const uint16_t* scales_f16, int N, int K,
                     std::vector<float>& W);

// Quantized activations, one row per token: K int8 values, one float scale,
// K/128 int32 group sums of the int8 values.
inline size_t ternary_act_row_bytes(int K) {
    return (size_t)K + 4 + 4 * (size_t)(K / kTernaryGroup);
}
// x is the full [T][K] float input; writes rows [t0, t1) of act. The reference
// defines the bytes; the dispatching entry point picks a vector version for
// this CPU that writes identical bytes.
void ternary_quant_rows_ref(const float* x, int K, int t0, int t1, uint8_t* act);
void ternary_quant_rows(const float* x, int K, int t0, int t1, uint8_t* act);

// y[t*N + n] for n in [r0, r1) and all t < T. The reference defines correctness;
// the dispatching entry point picks the best kernel for this CPU.
void ternary_matmul_rows_ref(const TernaryWeight& w, const uint8_t* act, int T, float* y,
                             int r0, int r1);
void ternary_matmul_rows(const TernaryWeight& w, const uint8_t* act, int T, float* y,
                         int r0, int r1);
const char* ternary_kernel_name();
// Both planes of a five-value linear in one pass. Writes the same bytes as
// ternary_matmul_rows on each plane followed by an add of the two results.
// ternary_fused_available() is true only when the kernel in use has a fused version
// AND PARAKEET_TERNARY_FUSED is set: the fused AVX2 kernel measured no faster end to
// end than two planes plus an add (see docs/phonon.md), so the graph uses it only on
// request. ternary_matmul2_rows runs the two planes one after the other when the
// kernel has no fused version.
bool ternary_fused_available();
void ternary_matmul2_rows_ref(const TernaryWeight& lo, const TernaryWeight& hi, const uint8_t* act, int T,
                              float* y, int r0, int r1);
void ternary_matmul2_rows(const TernaryWeight& lo, const TernaryWeight& hi, const uint8_t* act, int T,
                          float* y, int r0, int r1);

// Five-value linears (Phonon-2.5). Each weight is 0, +-lo[row] or +-hi[row], so
// W = lo * A + hi * B where A and B are ternary matrices that never share a
// nonzero position. The GGUF holds the upstream bytes unchanged:
//   <base>.fvcodes  I8,  N rows of ceil(K/5) bytes (trits, as in qweight; 1 = zero)
//   <base>.fvhi     I8,  ceil(nnz/8) bytes, one bit per nonzero weight in
//                   row-major order, least significant bit first (1 = hi)
//   <base>.fvscale  F16, shape [N, 2]: lo[N] then hi[N]
// At load each matrix is split into the two planes A (scale lo) and B (scale
// hi) as TernaryWeights with the row scale repeated for every group of 128.
//
// Splits the upstream bytes into the two planes. hibits_bytes is the exact size
// of the flag stream. Throws std::runtime_error when the stream length does not
// match the number of nonzero trits, a trit byte is >= 243, or a scale is not finite.
void fivevalue_split(const uint8_t* codes, const uint8_t* hibits, size_t hibits_bytes,
                     const uint16_t* lo_f16, const uint16_t* hi_f16, int N, int K,
                     TernaryWeight& plane_lo, TernaryWeight& plane_hi);
// Float reference: W (N*K row-major) = lo * A + hi * B read straight from the upstream bytes.
void fivevalue_dequant(const uint8_t* codes, const uint8_t* hibits, size_t hibits_bytes,
                       const uint16_t* lo_f16, const uint16_t* hi_f16, int N, int K,
                       std::vector<float>& W);

// The shape (N rows, K columns) of the encoder linear <base> from the model config; N and K
// are -1 when the name is none of the 11 linears of a block.
void fivevalue_linear_shape(const ModelLoader& ml, const std::string& base, int64_t& N, int64_t& K);
// True for the two pointwise convolutions. A dequantized file (convert with --five-value
// dequant) stores those as F32 and every other linear as F16 (with --dtype f16).
bool fivevalue_linear_is_conv(const std::string& base);
// Dequantizes the five-value linear <base> into the bytes a dequantized file would hold for
// its <base>.weight: N rows of K values, row-major, F32 when as_f32 else F16. All values are
// exactly representable in F16, so both are lossless. Used to give a GPU backend dense
// weights, since the packed kernels are CPU only. Throws std::runtime_error on a missing or
// malformed set of tensors.
void fivevalue_linear_dense(const ModelLoader& ml, const std::string& base, bool as_f32,
                            std::vector<uint8_t>& out);

// True iff the GGUF holds <base>.qweight (a packed ternary linear) or <base>.fvcodes
// (a five-value linear). ternary_linear handles both.
bool has_ternary(const ModelLoader& ml, const std::string& base);
// nn.Linear over x (f32, ne = [K, ...]) with the packed weight <base>.qweight /
// <base>.scales, or the five-value weight <base>.fvcodes / .fvhi / .fvscale (the
// activations are quantized once and both planes run on them; the two results are
// added). Returns f32 with ne = [N, ...]. Bias is the caller's job.
ggml_tensor* ternary_linear(ggml_context* ctx, const ModelLoader& ml, const std::string& base,
                            ggml_tensor* x);

// "" when parakeet.ternary.present agrees with the tensors: a packed tensor
// (any name ending in .qweight) exists iff the flag is set, and the same for
// parakeet.fivevalue.present and tensors ending in .fvcodes. Otherwise a message
// naming the mismatch. Scans every tensor, not just layer 0.
std::string ternary_flag_consistency_error(const ModelLoader& ml);

// Validate and repack every packed linear of the encoder up front, so graph
// building never throws. Throws std::runtime_error on a malformed or
// half-converted file.
void ternary_prepare(const ModelLoader& ml);

}  // namespace pk
