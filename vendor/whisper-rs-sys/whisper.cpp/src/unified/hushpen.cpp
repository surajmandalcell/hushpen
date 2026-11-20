#include "parakeet.h"
#include "backend.hpp"
#include "buffered_streaming.hpp"
#include "ggml_graph.hpp"
#include "tokenizer.hpp"
#include "transcription_json.hpp"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <memory>

namespace {
// The calling Rust engine owns the backend and serializes access to it.
// Upstream's graph builders find it on the calling thread, including after an engine move.
thread_local pk::Backend* active_backend = nullptr;
thread_local char error_text[512] = {};

struct BackendScope {
    pk::Backend* previous;
    explicit BackendScope(pk::Backend& backend) : previous(active_backend) { active_backend = &backend; }
    ~BackendScope() { active_backend = previous; }
};

template<class Fn> auto guarded(Fn fn) noexcept -> decltype(fn()) {
    try {
        return fn();
    } catch (const std::exception& e) {
        std::snprintf(error_text, sizeof(error_text), "%s", e.what());
    } catch (...) {
        std::snprintf(error_text, sizeof(error_text), "Unknown Unified inference error");
    }
    return nullptr;
}
}

namespace pk {
Backend& global_backend() {
    if (!active_backend) throw std::logic_error("Unified backend missing");
    return *active_backend;
}
bool run_graph(size_t, int, const std::function<ggml_tensor*(ggml_context*)>& build,
               std::vector<float>& out) {
    if (!global_backend().compute(build, out))
        throw std::runtime_error("Unified ggml graph allocation or computation failed");
    return true;
}
}

struct hushpen_unified_model {
    pk::Backend backend;
    pk::ModelLoader loader;
    std::string result;
    // Decoder state handed from one batch window to the next (see hushpen_unified_batch).
    pk::RnntDecodeState carry;
    hushpen_unified_model(bool gpu, int threads) : backend(threads, !gpu) {}
};

struct hushpen_unified_stream {
    pk::Backend& backend;
    pk::BufferedStreamingSession session;
    std::string result;
    hushpen_unified_stream(hushpen_unified_model& model, const int32_t* preset, const char* lang)
        : backend(model.backend), session(model.loader, preset[0], preset[1], preset[2], lang ? lang : "") {}
};

const char* hushpen_unified_error() { return error_text; }
void hushpen_unified_free(hushpen_unified_model* model) { delete model; }
void hushpen_unified_stream_free(hushpen_unified_stream* stream) { delete stream; }
bool hushpen_unified_gpu(const hushpen_unified_model* model) { return model && !model->backend.is_cpu(); }

hushpen_unified_model* hushpen_unified_load(const char* path, bool use_gpu, int threads) {
    return guarded([&] {
        if (!path || threads < 1) throw std::invalid_argument("Invalid Unified model load parameters");
        auto model = std::make_unique<hushpen_unified_model>(use_gpu, threads);
        if (!model->backend.handle()) throw std::runtime_error("Could not initialize Unified ggml backend");
        if (!model->loader.load(path)) throw std::runtime_error("Could not load Unified GGUF");
        const auto& config = model->loader.config();
        if (!config.unified.present || !config.prompt.present || config.sample_rate != 16000 ||
            !config.tdt_durations.empty())
            throw std::runtime_error("Expected a multilingual Unified RNNT GGUF");
        if (!model->loader.realize_weights(model->backend.handle()))
            throw std::runtime_error("Could not allocate Unified weights on the selected backend");
        return model.release();
    });
}

const char* hushpen_unified_batch(hushpen_unified_model* model, const float* pcm, int n, const char* lang,
                                  bool resume, int save_at) {
    return guarded([&] {
        if (!model || n < 0 || (n && !pcm)) throw std::invalid_argument("Invalid Unified audio buffer");
        BackendScope scope(model->backend);
        const auto& ml = model->loader;
        const auto& cfg = ml.config();
        const int language = cfg.prompt.resolve_index_or_throw(lang ? lang : "");
        const pk::PredictionNet pred(ml);
        // Copy first: the state saved below replaces model->carry.
        pk::RnntDecodeState st = resume ? model->carry : pk::rnnt_decode_init(pred);
        st.hyp.clear();
        if (n < static_cast<int>(cfg.hop_length)) {
            if (save_at >= 0) model->carry = st;
            model->result = pk::transcription_to_json("", {});
            return model->result.c_str();
        }

        // HUSHPEN_PARAKEET_TIMINGS=1 prints the native time split of each window to stderr.
        static const bool timings = std::getenv("HUSHPEN_PARAKEET_TIMINGS") != nullptr;
        using clock = std::chrono::steady_clock;
        auto ms = [](clock::time_point a, clock::time_point b) {
            return std::chrono::duration<double, std::milli>(b - a).count();
        };
        const auto t0 = clock::now();
        int n_mels, mel_frames, channels, frames;
        std::vector<float> features, encoded, projected;
        pk::MelFrontend(ml).compute(std::vector<float>(pcm, pcm + n), features, n_mels, mel_frames);
        const auto t1 = clock::now();
        pk::Encoder(ml).forward(features, n_mels, mel_frames, encoded, channels, frames);
        pk::PromptKernel(ml).apply(encoded, channels, frames, language, projected);
        const auto t2 = clock::now();
        // The encoder stores [channel, time]. The RNNT decoder consumes one time step per row.
        std::vector<float> rows(projected.size());
        for (int t = 0; t < frames; ++t)
            for (int c = 0; c < channels; ++c)
                rows[(size_t)t * channels + c] = projected[(size_t)c * frames + t];

        // A fresh decoder that starts mid-sentence can emit only blanks for a whole window, so
        // each window resumes from the state its neighbor had at this window's first frame.
        const int frame_samples = (int)(cfg.hop_length * cfg.subsampling_factor);
        const int split = save_at < 0 ? frames : std::min(frames, save_at / frame_samples);
        const pk::Joint joint(ml);
        std::vector<pk::TokenInfo> tokens, later;
        auto decode = [&](int from, int to, std::vector<pk::TokenInfo>* out) {
            if (to <= from) return;
            std::vector<float> part(rows.begin() + (size_t)from * channels, rows.begin() + (size_t)to * channels);
            pk::rnnt_decode_frames(pred, joint, part, to - from, channels, st, cfg.blank_id, cfg.max_symbols,
                                   nullptr, out);
            for (auto& token : *out) token.frame += from;
        };
        decode(0, split, &tokens);
        if (save_at >= 0) model->carry = st;
        decode(split, frames, &later);
        tokens.insert(tokens.end(), later.begin(), later.end());
        const auto& ids = st.hyp;
        if (timings)
            std::fprintf(stderr, "unified_timings: audio %.1f s  mel %.1f ms  encoder %.1f ms  decoder %.1f ms  frames %d  tokens %zu\n",
                         n / 16000.0, ms(t0, t1), ms(t1, t2), ms(t2, clock::now()), frames, ids.size());
        const float frame_sec = (float)cfg.hop_length * cfg.subsampling_factor / cfg.sample_rate;
        const auto& pieces = ml.tokenizer_pieces();
        model->result = pk::transcription_to_json(pk::detokenize(pieces, pk::strip_special_tokens(pieces, ids)),
                                                  pk::group_words(tokens, pieces, frame_sec));
        return model->result.c_str();
    });
}

hushpen_unified_stream* hushpen_unified_begin(hushpen_unified_model* model, const char* lang) {
    return guarded([&] {
        if (!model) throw std::invalid_argument("Missing Unified model");
        BackendScope scope(model->backend);
        const auto& cfg = model->loader.config().unified;
        return new hushpen_unified_stream(*model, &cfg.presets.at(3 * (size_t)cfg.default_preset), lang);
    });
}

const char* hushpen_unified_feed(hushpen_unified_stream* stream, const float* pcm, int n) {
    return guarded([&] {
        if (!stream) throw std::invalid_argument("Missing Unified stream");
        BackendScope scope(stream->backend);
        stream->session.feed(pcm, n);
        stream->result = stream->session.take_new_text();
        return stream->result.c_str();
    });
}

const char* hushpen_unified_finish(hushpen_unified_stream* stream) {
    return guarded([&] {
        if (!stream) throw std::invalid_argument("Missing Unified stream");
        BackendScope scope(stream->backend);
        if (!stream->session.finalized()) {
            stream->session.finalize();
            stream->result = pk::transcription_to_json(stream->session.text(), stream->session.drain_words());
        }
        return stream->result.c_str();
    });
}
