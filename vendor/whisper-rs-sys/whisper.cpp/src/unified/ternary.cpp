#include "ternary.hpp"

#include <algorithm>
#include <atomic>
#include <exception>
#include <thread>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <unordered_map>

#include "ggml.h"
#include "common.hpp"
#include "model_loader.hpp"
#include "ternary_kernels.hpp"

namespace pk {

namespace {

// For every byte value 0..255 its five base-3 digits, least significant first,
// computed like the scalar reference (v % 3, v /= 3). Bytes 243..255 are not a
// valid pack of five digits; ternary_repack rejects them, so their rows in this
// table are never used.
struct TritTable {
    uint8_t d[256][5];
    TritTable() {
        for (int b = 0; b < 256; ++b) {
            int v = b;
            for (int i = 0; i < 5; ++i) {
                d[b][i] = (uint8_t)(v % 3);
                v /= 3;
            }
        }
    }
};

const TritTable& trit_table() {
    static const TritTable t;  // thread safe initialization
    return t;
}

}  // namespace

namespace {

// Writes the codes of one row (K digits, followed by at least 8 spare bytes) into
// the kernel layout of row n. Shared by the upstream byte repack and the five-value split.
void repack_row_codes(const uint8_t* row, int n, int G, TernaryWeight& out) {
    const int b = n / kTernaryRowBlock, i = n % kTernaryRowBlock;
    for (int g = 0; g < G; ++g) {
        const uint8_t* src = row + (size_t)g * kTernaryGroup;
        uint8_t* dst = out.planes.data() + ((size_t)b * G + g) * 512 + 4 * i;
        // step st, byte j: the four codes at 16*st + 4*p + j, p = 0..3, in bit pairs
        for (int st = 0; st < 8; ++st) {
            const uint8_t* e = src + st * 16;
            for (int j = 0; j < 4; ++j)
                dst[st * 64 + j] = (uint8_t)(e[j] | (e[4 + j] << 2) | (e[8 + j] << 4) | (e[12 + j] << 6));
        }
    }
}

void alloc_weight(TernaryWeight& out, int N, int K) {
    out.N = N;
    out.K = K;
    const int G = K / kTernaryGroup;
    const int B = out.row_blocks();
    out.planes.assign((size_t)B * G * 512, 0);
    out.scales.assign((size_t)B * G * kTernaryRowBlock, 0.0f);
}

}  // namespace

void ternary_repack(const uint8_t* q, const uint16_t* s, int N, int K, TernaryWeight& out) {
    GGML_ASSERT(K > 0 && K % kTernaryGroup == 0);
    const int G = K / kTernaryGroup;
    const int nb = (K + 4) / 5;
    alloc_weight(out, N, K);
    const TritTable& tab = trit_table();
    // 5 spare bytes: the last upstream byte may hold digits past K, written and ignored
    std::vector<uint8_t> row((size_t)K + 8);
    for (int n = 0; n < N; ++n) {
        const int b = n / kTernaryRowBlock, i = n % kTernaryRowBlock;
        for (int g = 0; g < G; ++g)
            out.scales[((size_t)b * G + g) * kTernaryRowBlock + i] = ggml_fp16_to_fp32(s[(size_t)n * G + g]);
        const uint8_t* qr = q + (size_t)n * nb;
        uint8_t bad = 0;
        for (int bb = 0; bb < nb; ++bb) {
            bad = std::max(bad, qr[bb]);
            std::memcpy(&row[(size_t)bb * 5], tab.d[qr[bb]], 5);
        }
        // Five base-3 digits give 0..242. Larger bytes are a corrupt file, and the
        // converter rejects them too.
        if (bad >= 243)
            throw std::runtime_error("packed byte " + std::to_string((int)bad) + " in row " + std::to_string(n) +
                                     " is not valid (must be below 243 = 3^5)");
        repack_row_codes(row.data(), n, G, out);
    }
}

void fivevalue_split(const uint8_t* codes, const uint8_t* hibits, size_t hibits_bytes,
                     const uint16_t* lo_f16, const uint16_t* hi_f16, int N, int K,
                     TernaryWeight& plane_lo, TernaryWeight& plane_hi) {
    GGML_ASSERT(K > 0 && K % kTernaryGroup == 0);
    const int G = K / kTernaryGroup;
    const int nb = (K + 4) / 5;
    alloc_weight(plane_lo, N, K);
    alloc_weight(plane_hi, N, K);
    const TritTable& tab = trit_table();
    std::vector<uint8_t> row((size_t)K + 8), a((size_t)K + 8, 1), b((size_t)K + 8, 1);
    uint64_t bitpos = 0;
    const uint64_t nbits = (uint64_t)hibits_bytes * 8;
    for (int n = 0; n < N; ++n) {
        const int blk = n / kTernaryRowBlock, i = n % kTernaryRowBlock;
        const float lo = ggml_fp16_to_fp32(lo_f16[n]), hi = ggml_fp16_to_fp32(hi_f16[n]);
        if (!std::isfinite(lo) || !std::isfinite(hi))
            throw std::runtime_error("scale of row " + std::to_string(n) + " is not finite");
        for (int g = 0; g < G; ++g) {
            plane_lo.scales[((size_t)blk * G + g) * kTernaryRowBlock + i] = lo;
            plane_hi.scales[((size_t)blk * G + g) * kTernaryRowBlock + i] = hi;
        }
        const uint8_t* qr = codes + (size_t)n * nb;
        uint8_t bad = 0;
        for (int bb = 0; bb < nb; ++bb) {
            bad = std::max(bad, qr[bb]);
            std::memcpy(&row[(size_t)bb * 5], tab.d[qr[bb]], 5);
        }
        if (bad >= 243)
            throw std::runtime_error("packed byte " + std::to_string((int)bad) + " in row " + std::to_string(n) +
                                     " is not valid (must be below 243 = 3^5)");
        for (int k = 0; k < K; ++k) {
            const uint8_t c = row[k];
            if (c == 1) {   // zero weight: no flag bit, zero in both planes
                a[k] = 1;
                b[k] = 1;
                continue;
            }
            if (bitpos >= nbits) throw std::runtime_error("hi-flag stream is shorter than the number of nonzero weights");
            const bool hi_flag = (hibits[bitpos >> 3] >> (bitpos & 7)) & 1;
            ++bitpos;
            a[k] = hi_flag ? 1 : c;
            b[k] = hi_flag ? c : 1;
        }
        repack_row_codes(a.data(), n, G, plane_lo);
        repack_row_codes(b.data(), n, G, plane_hi);
    }
    if ((bitpos + 7) / 8 != hibits_bytes)
        throw std::runtime_error("hi-flag stream has " + std::to_string(hibits_bytes) + " bytes for " +
                                 std::to_string((unsigned long long)bitpos) + " nonzero weights");
}

void fivevalue_dequant(const uint8_t* codes, const uint8_t* hibits, size_t hibits_bytes,
                       const uint16_t* lo_f16, const uint16_t* hi_f16, int N, int K,
                       std::vector<float>& W) {
    const int nb = (K + 4) / 5;
    W.assign((size_t)N * K, 0.0f);
    uint64_t bitpos = 0;
    for (int n = 0; n < N; ++n) {
        const float lo = ggml_fp16_to_fp32(lo_f16[n]), hi = ggml_fp16_to_fp32(hi_f16[n]);
        for (int bb = 0; bb < nb; ++bb) {
            int v = codes[(size_t)n * nb + bb];
            for (int d = 0; d < 5; ++d, v /= 3) {
                const int k = bb * 5 + d;
                if (k >= K) continue;
                const int c = v % 3;
                if (c == 1) continue;
                const bool h = bitpos / 8 < hibits_bytes && ((hibits[bitpos >> 3] >> (bitpos & 7)) & 1);
                ++bitpos;
                W[(size_t)n * K + k] = (float)(c - 1) * (h ? hi : lo);
            }
        }
    }
}

void ternary_dequant(const uint8_t* q, const uint16_t* s, int N, int K, std::vector<float>& W) {
    const int G = (K + kTernaryGroup - 1) / kTernaryGroup;
    const int nb = (K + 4) / 5;
    W.assign((size_t)N * K, 0.0f);
    for (int n = 0; n < N; ++n)
        for (int b = 0; b < nb; ++b) {
            int v = q[(size_t)n * nb + b];
            for (int d = 0; d < 5; ++d) {
                const int i = b * 5 + d;
                if (i < K)
                    W[(size_t)n * K + i] =
                        ggml_fp16_to_fp32(s[(size_t)n * G + i / kTernaryGroup]) * (float)(v % 3 - 1);
                v /= 3;
            }
        }
}

void ternary_quant_rows_ref(const float* x, int K, int t0, int t1, uint8_t* act) {
    const int G = K / kTernaryGroup;
    const size_t rb = ternary_act_row_bytes(K);
    for (int t = t0; t < t1; ++t) {
        const float* xr = x + (size_t)t * K;
        uint8_t* row = act + (size_t)t * rb;
        int8_t* q = reinterpret_cast<int8_t*>(row);
        float* sc = reinterpret_cast<float*>(row + K);
        int32_t* gs = reinterpret_cast<int32_t*>(row + K + 4);
        float amax = 0.0f;
        for (int k = 0; k < K; ++k) amax = std::max(amax, std::fabs(xr[k]));
        const float s = amax > 0.0f ? amax / 127.0f : 0.0f;
        const float inv = s > 0.0f ? 1.0f / s : 0.0f;
        *sc = s;
        for (int g = 0; g < G; ++g) {
            int32_t sum = 0;
            for (int j = 0; j < kTernaryGroup; ++j) {
                // clamp before rounding (same result, since +-127 are
                // integers), which also maps a NaN input to -127
                const float f = std::min(127.0f, std::max(-127.0f, xr[g * kTernaryGroup + j] * inv));
                const int v = (int)std::lrintf(f);
                q[g * kTernaryGroup + j] = (int8_t)v;
                sum += v;
            }
            gs[g] = sum;
        }
    }
}

void ternary_matmul_rows_ref(const TernaryWeight& w, const uint8_t* act, int T, float* y,
                             int r0, int r1) {
    const int N = w.N, K = w.K, G = w.groups();
    const size_t rb = ternary_act_row_bytes(K);
    for (int n = r0; n < r1; ++n)
        for (int t = 0; t < T; ++t) {
            const uint8_t* row = act + (size_t)t * rb;
            const int8_t* q = reinterpret_cast<const int8_t*>(row);
            const float sa = *reinterpret_cast<const float*>(row + K);
            const int32_t* gs = reinterpret_cast<const int32_t*>(row + K + 4);
            float acc = 0.0f;
            for (int g = 0; g < G; ++g) {
                int32_t s = 0;
                for (int j = 0; j < kTernaryGroup; ++j)
                    s += w.code(n, g * kTernaryGroup + j) * (int32_t)q[g * kTernaryGroup + j];
                acc += w.scale(n, g) * (float)(s - gs[g]);
            }
            y[(size_t)t * N + n] = acc * sa;
        }
}

namespace {

const TernaryKernel kScalarKernel{"scalar", ternary_matmul_rows_ref};

const TernaryKernel* pick_kernel() {
    static const TernaryKernel* chosen = [] {
        if (const char* e = std::getenv("PARAKEET_TERNARY_KERNEL")) {
            const TernaryKernel* want = nullptr;
            if (!std::strcmp(e, "scalar")) want = &kScalarKernel;
            else if (!std::strcmp(e, "avx2")) want = ternary_kernel_x86_avx2();
            else if (!std::strcmp(e, "vnni")) want = ternary_kernel_x86_vnni();
            else if (!std::strcmp(e, "neon")) want = ternary_kernel_neon();
            if (want) return want;
            PK_LOG("PARAKEET_TERNARY_KERNEL=%s is unavailable here; using automatic selection", e);
        }
        if (const TernaryKernel* k = ternary_kernel_x86_vnni()) return k;
        if (const TernaryKernel* k = ternary_kernel_neon()) return k;
        if (const TernaryKernel* k = ternary_kernel_x86_avx2()) return k;
        return &kScalarKernel;
    }();
    return chosen;
}

}  // namespace

std::vector<const TernaryKernel*> ternary_all_kernels() {
    std::vector<const TernaryKernel*> v{&kScalarKernel};
    for (const TernaryKernel* k : {ternary_kernel_x86_avx2(), ternary_kernel_x86_vnni(), ternary_kernel_neon()})
        if (k) v.push_back(k);
    return v;
}

std::vector<const TernaryQuant*> ternary_all_quants() {
    static const TernaryQuant ref{"scalar", ternary_quant_rows_ref};
    std::vector<const TernaryQuant*> v{&ref};
    for (const TernaryQuant* q : {ternary_quant_x86_avx2(), ternary_quant_x86_avx512()})
        if (q) v.push_back(q);
    return v;
}

void ternary_quant_rows(const float* x, int K, int t0, int t1, uint8_t* act) {
    static const TernaryQuantFn fn = ternary_all_quants().back()->fn;
    fn(x, K, t0, t1, act);
}

void ternary_matmul_rows(const TernaryWeight& w, const uint8_t* act, int T, float* y, int r0, int r1) {
    pick_kernel()->fn(w, act, T, y, r0, r1);
}

const char* ternary_kernel_name() { return pick_kernel()->name; }

bool ternary_fused_available() {
    static const bool on = pick_kernel()->fn2 != nullptr && std::getenv("PARAKEET_TERNARY_FUSED") != nullptr;
    return on;
}

void ternary_matmul2_rows_ref(const TernaryWeight& lo, const TernaryWeight& hi, const uint8_t* act, int T,
                              float* y, int r0, int r1) {
    const size_t n = (size_t)T * lo.N;
    std::vector<float> b(n);
    ternary_matmul_rows_ref(lo, act, T, y, r0, r1);
    ternary_matmul_rows_ref(hi, act, T, b.data(), r0, r1);
    for (int t = 0; t < T; ++t)
        for (int i = r0; i < r1; ++i) y[(size_t)t * lo.N + i] += b[(size_t)t * lo.N + i];
}

void ternary_matmul2_rows(const TernaryWeight& lo, const TernaryWeight& hi, const uint8_t* act, int T,
                          float* y, int r0, int r1) {
    const TernaryKernel* k = pick_kernel();
    if (k->fn2) {
        k->fn2(lo, hi, act, T, y, r0, r1);
        return;
    }
    const size_t n = (size_t)T * lo.N;
    std::vector<float> b(n);
    k->fn(lo, act, T, y, r0, r1);
    k->fn(hi, act, T, b.data(), r0, r1);
    for (int t = 0; t < T; ++t)
        for (int i = r0; i < r1; ++i) y[(size_t)t * lo.N + i] += b[(size_t)t * lo.N + i];
}

namespace {

// The two planes of one five-value linear: lo holds the weights that use the
// row's lo scale, hi those that use its hi scale.
struct FiveValuePair {
    std::unique_ptr<TernaryWeight> lo, hi;
};

struct TernaryStore {
    std::mutex mu;
    std::unordered_map<std::string, std::unique_ptr<TernaryWeight>> weights;
    std::unordered_map<std::string, FiveValuePair> five;
};

// Validates the packed tensors of one linear before any read of their data.
// expN / expK are the shapes the model config demands (or -1 to skip).
void validate_ternary_tensors(const ggml_tensor* q, const ggml_tensor* s, const std::string& base,
                              int64_t expN, int64_t expK) {
    const std::string who = "ternary: " + base + ": ";
    if (q->type != GGML_TYPE_I8) throw std::runtime_error(who + "qweight is not I8");
    if (s->type != GGML_TYPE_F16) throw std::runtime_error(who + "scales is not F16");
    if (q->ne[2] != 1 || q->ne[3] != 1 || s->ne[2] != 1 || s->ne[3] != 1)
        throw std::runtime_error(who + "tensors must be 2-D");
    const int64_t kMax = (int64_t)1 << 24;  // far above any real layer, keeps all products in range
    const int64_t N = q->ne[1];
    const int64_t G = s->ne[0];
    if (N <= 0 || G <= 0 || N > kMax || G > kMax)
        throw std::runtime_error(who + "invalid dimensions");
    const int64_t K = G * kTernaryGroup;
    if (K % kTernaryGroup != 0) throw std::runtime_error(who + "K is not a multiple of 128");
    if (s->ne[1] != N) throw std::runtime_error(who + "scales rows differ from qweight rows");
    const int64_t nb = (K + 4) / 5;
    if (q->ne[0] != nb) throw std::runtime_error(who + "qweight row length does not match K");
    if ((uint64_t)ggml_nbytes(q) != (uint64_t)N * (uint64_t)nb)
        throw std::runtime_error(who + "qweight byte size mismatch");
    if ((uint64_t)ggml_nbytes(s) != (uint64_t)N * (uint64_t)G * 2u)
        throw std::runtime_error(who + "scales byte size mismatch");
    if (expN >= 0 && (N != expN || K != expK))
        throw std::runtime_error(who + "shape " + std::to_string(N) + "x" + std::to_string(K) +
                                 " does not match the model config (" + std::to_string(expN) + "x" +
                                 std::to_string(expK) + ")");
}

TernaryStore& store_of(const ModelLoader& ml) {
    static std::mutex init_mu;
    std::lock_guard<std::mutex> lk(init_mu);
    auto& slot = ml.ternary_store();
    if (!slot) slot = std::make_shared<TernaryStore>();
    return *static_cast<TernaryStore*>(slot.get());
}

// Validates the three tensors of one five-value linear before any read of their data.
void validate_fivevalue_tensors(const ggml_tensor* c, const ggml_tensor* h, const ggml_tensor* sc,
                                const std::string& base, int64_t expN, int64_t expK) {
    const std::string who = "five-value: " + base + ": ";
    if (c->type != GGML_TYPE_I8) throw std::runtime_error(who + "fvcodes is not I8");
    if (h->type != GGML_TYPE_I8) throw std::runtime_error(who + "fvhi is not I8");
    if (sc->type != GGML_TYPE_F16) throw std::runtime_error(who + "fvscale is not F16");
    if (c->ne[2] != 1 || c->ne[3] != 1 || h->ne[1] != 1 || h->ne[2] != 1 || h->ne[3] != 1 ||
        sc->ne[2] != 1 || sc->ne[3] != 1)
        throw std::runtime_error(who + "unexpected tensor rank");
    if (expN < 0 || expK <= 0 || expK % kTernaryGroup != 0)
        throw std::runtime_error(who + "no valid shape from the model config");
    const int64_t nb = (expK + 4) / 5;
    if (c->ne[1] != expN || c->ne[0] != nb)
        throw std::runtime_error(who + "fvcodes shape does not match the model config (" + std::to_string(expN) +
                                 "x" + std::to_string(expK) + ")");
    if (sc->ne[0] != expN || sc->ne[1] != 2)
        throw std::runtime_error(who + "fvscale must be [N, 2] (lo then hi)");
    if ((uint64_t)ggml_nbytes(c) != (uint64_t)expN * (uint64_t)nb)
        throw std::runtime_error(who + "fvcodes byte size mismatch");
    // one flag bit per nonzero weight: at most N*K bits
    if ((uint64_t)h->ne[0] > ((uint64_t)expN * (uint64_t)expK + 7) / 8)
        throw std::runtime_error(who + "fvhi is longer than N*K bits");
}

// Looks the tensors of one linear up and validates them; throws on a malformed set.
struct TernaryJob {
    std::string base;
    bool five = false;
    const uint8_t* q = nullptr;     // qweight, or fvcodes
    const uint16_t* s = nullptr;    // scales, or fvscale (lo[N] then hi[N])
    const uint8_t* hib = nullptr;   // fvhi
    size_t hib_bytes = 0;
    int N = 0, K = 0;
    std::unique_ptr<TernaryWeight> out, out_hi;
    std::exception_ptr err;
    void run() {
        try {
            out = std::make_unique<TernaryWeight>();
            if (five) {
                out_hi = std::make_unique<TernaryWeight>();
                fivevalue_split(q, hib, hib_bytes, s, s + N, N, K, *out, *out_hi);
            } else {
                ternary_repack(q, s, N, K, *out);
            }
        } catch (const std::exception& e) {
            err = std::make_exception_ptr(std::runtime_error(std::string(five ? "five-value: " : "ternary: ") +
                                                             base + ": " + e.what()));
        } catch (...) {
            err = std::current_exception();
        }
    }
};

TernaryJob make_job(const ModelLoader& ml, const std::string& base, int64_t expN, int64_t expK) {
    const ggml_tensor* q = ml.tensor(base + ".qweight");
    const ggml_tensor* s = ml.tensor(base + ".scales");
    if (!q || !s) throw std::runtime_error("ternary: missing " + base + ".qweight/.scales");
    validate_ternary_tensors(q, s, base, expN, expK);
    TernaryJob j;
    j.base = base;
    j.q = static_cast<const uint8_t*>(q->data);
    j.s = static_cast<const uint16_t*>(s->data);
    j.N = (int)q->ne[1];
    j.K = (int)s->ne[0] * kTernaryGroup;
    return j;
}

TernaryJob make_five_job(const ModelLoader& ml, const std::string& base, int64_t expN, int64_t expK) {
    const ggml_tensor* c = ml.tensor(base + ".fvcodes");
    const ggml_tensor* h = ml.tensor(base + ".fvhi");
    const ggml_tensor* sc = ml.tensor(base + ".fvscale");
    if (!c || !h || !sc) throw std::runtime_error("five-value: missing " + base + ".fvcodes/.fvhi/.fvscale");
    validate_fivevalue_tensors(c, h, sc, base, expN, expK);
    TernaryJob j;
    j.base = base;
    j.five = true;
    j.q = static_cast<const uint8_t*>(c->data);
    j.hib = static_cast<const uint8_t*>(h->data);
    j.hib_bytes = (size_t)h->ne[0];
    j.s = static_cast<const uint16_t*>(sc->data);
    j.N = (int)expN;
    j.K = (int)expK;
    return j;
}

// The model's encoder linears, as (name, N, K).
struct LinearShape { const char* name; int64_t N, K; };
std::vector<LinearShape> encoder_linears(const ParakeetConfig& cfg) {
    const int64_t d = cfg.d_model, ff = cfg.ff_dim;
    return {
        {"feed_forward1.linear1", ff, d},  {"feed_forward1.linear2", d, ff},
        {"feed_forward2.linear1", ff, d},  {"feed_forward2.linear2", d, ff},
        {"self_attn.linear_q", d, d},      {"self_attn.linear_k", d, d},
        {"self_attn.linear_v", d, d},      {"self_attn.linear_out", d, d},
        {"self_attn.linear_pos", d, d},    {"conv.pointwise_conv1", 2 * d, d},
        {"conv.pointwise_conv2", d, d}};
}

const TernaryWeight& weight_for(const ModelLoader& ml, const std::string& base,
                                int64_t expN = -1, int64_t expK = -1) {
    TernaryStore& store = store_of(ml);
    std::lock_guard<std::mutex> lk(store.mu);
    auto it = store.weights.find(base);
    if (it != store.weights.end()) return *it->second;
    TernaryJob j = make_job(ml, base, expN, expK);
    j.run();
    if (j.err) std::rethrow_exception(j.err);
    return *store.weights.emplace(base, std::move(j.out)).first->second;
}

// Both planes of a five-value linear, split on first use when ternary_prepare did not already.
const FiveValuePair& five_for(const ModelLoader& ml, const std::string& base) {
    TernaryStore& store = store_of(ml);
    std::lock_guard<std::mutex> lk(store.mu);
    auto it = store.five.find(base);
    if (it != store.five.end()) return it->second;
    const ggml_tensor* c = ml.tensor(base + ".fvcodes");
    const ggml_tensor* sc = ml.tensor(base + ".fvscale");
    if (!c || !sc) throw std::runtime_error("five-value: missing tensors for " + base);
    // The shape comes from the model config: the layer name picks the linear.
    int64_t N = -1, K = -1;
    for (const LinearShape& l : encoder_linears(ml.config())) {
        const std::string suffix = std::string(".") + l.name;
        if (base.size() > suffix.size() && base.compare(base.size() - suffix.size(), suffix.size(), suffix) == 0) {
            N = l.N;
            K = l.K;
        }
    }
    TernaryJob j = make_five_job(ml, base, N, K);
    j.run();
    if (j.err) std::rethrow_exception(j.err);
    FiveValuePair p;
    p.lo = std::move(j.out);
    p.hi = std::move(j.out_hi);
    return store.five.emplace(base, std::move(p)).first->second;
}

// The (N, K) shape of an encoder linear from its layer name; -1 when the name matches none.
void shape_of_linear(const ModelLoader& ml, const std::string& base, int64_t& N, int64_t& K) {
    N = -1;
    K = -1;
    for (const LinearShape& l : encoder_linears(ml.config())) {
        const std::string suffix = std::string(".") + l.name;
        if (base.size() > suffix.size() && base.compare(base.size() - suffix.size(), suffix.size(), suffix) == 0) {
            N = l.N;
            K = l.K;
        }
    }
}

// Worker count for the load-time repack: PARAKEET_REPACK_THREADS, else
// min(hardware_concurrency, 8). 1 selects the serial path.
int repack_threads() {
    if (const char* e = std::getenv("PARAKEET_REPACK_THREADS")) {
        char* end = nullptr;
        const long v = std::strtol(e, &end, 10);
        if (end != e && v >= 1) return (int)std::min<long>(v, 256);
    }
    const unsigned hc = std::thread::hardware_concurrency();
    return (int)std::min(hc ? hc : 1u, 8u);
}

// Op 1: per-row int8 quantization of the activations. dst is I8, ne0 = row bytes.
void op_quant(ggml_tensor* dst, int ith, int nth, void*) {
    const ggml_tensor* x = dst->src[0];
    const int K = (int)x->ne[0];
    const int T = (int)ggml_nrows(x);
    const int t0 = (int)((int64_t)T * ith / nth);
    const int t1 = (int)((int64_t)T * (ith + 1) / nth);
    ternary_quant_rows(static_cast<const float*>(x->data), K, t0, t1, static_cast<uint8_t*>(dst->data));
}

// Op 2: ternary matmul. src0 is the quantized activation tensor from op 1,
// userdata is the TernaryWeight. Threads split the output rows.
void op_matmul(ggml_tensor* dst, int ith, int nth, void* ud) {
    const TernaryWeight& w = *static_cast<const TernaryWeight*>(ud);
    const int T = (int)ggml_nrows(dst);
    const int r0 = (int)((int64_t)w.N * ith / nth);
    const int r1 = (int)((int64_t)w.N * (ith + 1) / nth);
    ternary_matmul_rows(w, static_cast<const uint8_t*>(dst->src[0]->data), T,
                        static_cast<float*>(dst->data), r0, r1);
}

// Op 2 for five-value linears: both planes in one pass. src0 is the quantized
// activation tensor, userdata points to the two planes.
void op_matmul2(ggml_tensor* dst, int ith, int nth, void* ud) {
    const FiveValuePair& p = *static_cast<const FiveValuePair*>(ud);
    const TernaryWeight& w = *p.lo;
    const int T = (int)ggml_nrows(dst);
    const int r0 = (int)((int64_t)w.N * ith / nth);
    const int r1 = (int)((int64_t)w.N * (ith + 1) / nth);
    ternary_matmul2_rows(w, *p.hi, static_cast<const uint8_t*>(dst->src[0]->data), T,
                         static_cast<float*>(dst->data), r0, r1);
}

}  // namespace

std::string ternary_flag_consistency_error(const ModelLoader& ml) {
    const bool packed = ml.has_tensor_with_suffix(".qweight");
    const bool flag = ml.config().ternary.present;
    if (packed && !flag)
        return "this GGUF holds packed ternary tensors (.qweight) but parakeet.ternary.present is not set";
    if (flag && !packed)
        return "parakeet.ternary.present is set but the GGUF holds no packed ternary tensors (.qweight)";
    const bool five = ml.has_tensor_with_suffix(".fvcodes");
    const bool fflag = ml.config().fivevalue.present;
    if (five && !fflag)
        return "this GGUF holds five-value tensors (.fvcodes) but parakeet.fivevalue.present is not set";
    if (fflag && !five)
        return "parakeet.fivevalue.present is set but the GGUF holds no five-value tensors (.fvcodes)";
    return "";
}

void fivevalue_linear_shape(const ModelLoader& ml, const std::string& base, int64_t& N, int64_t& K) {
    shape_of_linear(ml, base, N, K);
}

bool fivevalue_linear_is_conv(const std::string& base) {
    for (const char* suf : {".conv.pointwise_conv1", ".conv.pointwise_conv2"}) {
        const size_t l = std::strlen(suf);
        if (base.size() > l && base.compare(base.size() - l, l, suf) == 0) return true;
    }
    return false;
}

void fivevalue_linear_dense(const ModelLoader& ml, const std::string& base, bool as_f32,
                            std::vector<uint8_t>& out) {
    int64_t N = -1, K = -1;
    shape_of_linear(ml, base, N, K);
    TernaryJob j = make_five_job(ml, base, N, K);  // validates the three tensors first
    std::vector<float> w;
    fivevalue_dequant(j.q, j.hib, j.hib_bytes, j.s, j.s + j.N, j.N, j.K, w);
    if (as_f32) {
        out.resize(w.size() * sizeof(float));
        std::memcpy(out.data(), w.data(), out.size());
    } else {
        out.resize(w.size() * sizeof(ggml_fp16_t));
        ggml_fp32_to_fp16_row(w.data(), reinterpret_cast<ggml_fp16_t*>(out.data()), (int64_t)w.size());
    }
}

bool has_ternary(const ModelLoader& ml, const std::string& base) {
    return ml.tensor(base + ".qweight") != nullptr || ml.tensor(base + ".fvcodes") != nullptr;
}

ggml_tensor* ternary_linear(ggml_context* ctx, const ModelLoader& ml, const std::string& base,
                            ggml_tensor* x) {
    const bool five = ml.tensor(base + ".fvcodes") != nullptr && ml.tensor(base + ".qweight") == nullptr;
    const FiveValuePair* fv = five ? &five_for(ml, base) : nullptr;
    const TernaryWeight& w = five ? *fv->lo : weight_for(ml, base);
    if (x->type != GGML_TYPE_F32 || (int)x->ne[0] != w.K)
        throw std::runtime_error("ternary: input shape mismatch for " + base);
    if (!ggml_is_contiguous(x)) x = ggml_cont(ctx, x);
    const int64_t T = ggml_nrows(x);
    ggml_tensor* a1[1] = {x};
    ggml_tensor* act = ggml_custom_4d(ctx, GGML_TYPE_I8, (int64_t)ternary_act_row_bytes(w.K), T, 1, 1,
                                      a1, 1, op_quant, GGML_N_TASKS_MAX, nullptr);
    ggml_tensor* a2[1] = {act};
    if (five && ternary_fused_available())
        return ggml_custom_4d(ctx, GGML_TYPE_F32, w.N, x->ne[1], x->ne[2], x->ne[3], a2, 1, op_matmul2,
                              GGML_N_TASKS_MAX, const_cast<FiveValuePair*>(fv));
    ggml_tensor* y = ggml_custom_4d(ctx, GGML_TYPE_F32, w.N, x->ne[1], x->ne[2], x->ne[3], a2, 1, op_matmul,
                                    GGML_N_TASKS_MAX, const_cast<TernaryWeight*>(&w));
    if (!five) return y;
    // The activations are quantized once; the second plane reads the same bytes.
    ggml_tensor* y2 = ggml_custom_4d(ctx, GGML_TYPE_F32, w.N, x->ne[1], x->ne[2], x->ne[3], a2, 1, op_matmul,
                                     GGML_N_TASKS_MAX, const_cast<TernaryWeight*>(fv->hi.get()));
    return ggml_add(ctx, y, y2);
}

void ternary_prepare(const ModelLoader& ml) {
    const ParakeetConfig& cfg = ml.config();
    const int64_t d = cfg.d_model, ff = cfg.ff_dim;
    if (cfg.ternary.present && cfg.ternary.group_size != (uint32_t)kTernaryGroup)
        throw std::runtime_error("ternary: group_size must be 128");
    if (d <= 0 || ff <= 0) throw std::runtime_error("ternary: invalid d_model / ff_dim");
    const std::vector<LinearShape> kLinears = encoder_linears(cfg);
    const int n_layers = (int)cfg.n_layers;
    // Validate everything serially and in layer order (the first error is the
    // one reported), then repack the queued weights on a small thread pool.
    TernaryStore& store = store_of(ml);
    std::vector<TernaryJob> jobs;
    for (int i = 0; i < n_layers; ++i) {
        for (const LinearShape& l : kLinears) {
            const std::string base = "encoder.layers." + std::to_string(i) + "." + l.name;
            if (ml.tensor(base + ".qweight")) {
                {
                    std::lock_guard<std::mutex> lk(store.mu);
                    if (store.weights.count(base)) continue;
                }
                jobs.push_back(make_job(ml, base, l.N, l.K));
            } else if (ml.tensor(base + ".fvcodes")) {
                {
                    std::lock_guard<std::mutex> lk(store.mu);
                    if (store.five.count(base)) continue;
                }
                jobs.push_back(make_five_job(ml, base, l.N, l.K));
            } else if (!ml.tensor(base + ".weight")) {
                throw std::runtime_error("ternary: missing weight " + base);
            }
        }
    }
    std::atomic<size_t> next{0};
    auto worker = [&jobs, &next] {
        for (size_t k; (k = next.fetch_add(1)) < jobs.size();) jobs[k].run();
    };
    const int nthreads = (int)std::min<size_t>((size_t)repack_threads(), jobs.size());
    std::vector<std::thread> pool;
    for (int t = 1; t < nthreads; ++t) {
        try {
            pool.emplace_back(worker);
        } catch (...) {
            break;  // could not start more threads: the ones running (and this one) drain the queue
        }
    }
    worker();
    for (std::thread& th : pool) th.join();
    for (TernaryJob& j : jobs)
        if (j.err) std::rethrow_exception(j.err);
    std::lock_guard<std::mutex> lk(store.mu);
    for (TernaryJob& j : jobs) {
        if (j.five) {
            FiveValuePair p;
            p.lo = std::move(j.out);
            p.hi = std::move(j.out_hi);
            store.five.emplace(j.base, std::move(p));
        } else {
            store.weights.emplace(j.base, std::move(j.out));
        }
    }
    if (std::strcmp(ternary_kernel_name(), "scalar") == 0)
        PK_LOG("packed ternary weights are running on the slow scalar kernel on this CPU/build "
               "(about 1 GMAC/s); re-convert with --ternary dequant for speed");
}

}  // namespace pk
