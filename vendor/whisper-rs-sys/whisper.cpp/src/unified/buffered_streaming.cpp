#include "buffered_streaming.hpp"
#include "tokenizer.hpp"

#include <algorithm>
#include <iomanip>
#include <locale>
#include <sstream>
#include <stdexcept>
#include <string>

namespace pk {

namespace {
// One encoder frame: 8x subsampling of 160-sample mel hops = 1280 samples (80 ms).
constexpr int64_t kFrameSamples = 1280;
}

// ---------------------------------------------------------------------------
// BufferedSchedule
// ---------------------------------------------------------------------------

BufferedSchedule::BufferedSchedule(int left, int chunk, int right)
    : exp_left_((int64_t)left * kFrameSamples),
      exp_chunk_((int64_t)chunk * kFrameSamples),
      exp_right_((int64_t)right * kFrameSamples) {
    next_edge_ = exp_chunk_ + exp_right_;   // NeMo: right_sample = min(chunk + right, len)
}

void BufferedSchedule::add_frames(int64_t num, bool is_last) {
    // Port of NeMo ContextSize.add_frames_get_removed_ (streaming_utils.py).
    if (num > exp_chunk_ + exp_right_)
        throw std::runtime_error("buffered streaming: step is longer than chunk + right context");
    ctx_left_ += ctx_chunk_;
    ctx_chunk_ = 0;
    ctx_right_ += num;
    if (is_last) {
        ctx_chunk_ = ctx_right_;
        ctx_right_ = 0;
    } else if (ctx_right_ > exp_chunk_) {
        ctx_chunk_ = exp_chunk_;
        ctx_right_ -= exp_chunk_;
    }
    const int64_t total = ctx_left_ + ctx_chunk_ + ctx_right_;
    const int64_t exp_total = exp_left_ + exp_chunk_ + exp_right_;
    const int64_t extra = std::max<int64_t>(total - exp_total, 0);
    ctx_left_ -= extra;
}

BufferedStepPlan BufferedSchedule::make(int64_t edge, bool is_last) {
    BufferedStepPlan p;
    p.edge = edge;
    p.num = edge - left_edge_;
    p.is_last = is_last;
    add_frames(p.num, is_last);
    p.left = ctx_left_;
    p.chunk = ctx_chunk_;
    p.right = ctx_right_;
    p.win_start = edge - (p.left + p.chunk + p.right);
    left_edge_ = edge;
    if (is_last) done_ = true;
    else next_edge_ = edge + exp_chunk_;
    return p;
}

void BufferedSchedule::advance(int64_t received, std::vector<BufferedStepPlan>& out) {
    while (!done_ && next_edge_ < received) {
        out.push_back(make(next_edge_, /*is_last=*/false));
    }
}

bool BufferedSchedule::finish(int64_t total, BufferedStepPlan& out) {
    // Precondition: advance(total) has already been called, so every step before
    // the end of the audio has been emitted.
    if (done_ || total <= left_edge_) return false;
    out = make(total, /*is_last=*/true);
    return true;
}

// ---------------------------------------------------------------------------
// BufferedStreamingSession
// ---------------------------------------------------------------------------

namespace {

// Throws std::invalid_argument for a window check_buffered_window refuses; true otherwise.
bool window_or_throw(int left, int chunk, int right) {
    if (const std::string why = check_buffered_window(left, chunk, right); !why.empty())
        throw std::invalid_argument("buffered streaming: " + why);
    return true;
}

} // namespace

BufferedStreamingSession::BufferedStreamingSession(const ModelLoader& ml, int left, int chunk,
                                                   int right, const std::string& target_lang)
    : ml_(ml), left_(left), chunk_(chunk), right_(right),
      window_ok_(window_or_throw(left, chunk, right)),
      mel_(ml), enc_(ml), pred_(ml), joint_(ml), prompt_(ml),
      schedule_(left, chunk, right) {
    const ParakeetConfig& cfg = ml.config();
    if (!cfg.unified.present)
        throw std::runtime_error("buffered streaming needs a unified checkpoint (parakeet.unified.present)");
    d_model_  = (int)cfg.d_model;
    blank_id_ = (int)cfg.blank_id;
    max_symbols_ = (int)cfg.max_symbols;
    if (cfg.prompt.present) {
        // Empty target_lang -> the model default; an unknown locale throws.
        prompt_index_ = cfg.prompt.resolve_index_or_throw(target_lang);
    }
    const double hop = (double)cfg.hop_length;
    const double sub = (double)(cfg.subsampling_factor ? cfg.subsampling_factor : 1);
    const double sr  = (double)(cfg.sample_rate ? cfg.sample_rate : 16000);
    frame_sec_   = (hop * sub) / sr;
    frame_sec_f_ = (float)frame_sec_;
    reset();
}

void BufferedStreamingSession::reset() {
    state_ = rnnt_decode_init(pred_);
    schedule_ = BufferedSchedule(left_, chunk_, right_);
    pcm_.clear();
    origin_ = 0;
    received_ = 0;
    frames_done_ = 0;
    finalized_ = false;
    latch_.clear();
    hyp_.clear();
    text_.clear();
    emitted_.clear();
    word_tokens_.clear();
    words_.clear();
    words_finalized_ = 0;
    words_taken_ = 0;
    trace_.clear();
}

void BufferedStreamingSession::feed(const float* pcm, int n) {
    latch_.check();
    if (finalized_) throw std::runtime_error("buffered streaming: feed after finalize (call reset)");
    if (n < 0 || (!pcm && n > 0)) throw std::invalid_argument("buffered streaming: invalid PCM buffer");
    // The schedule advances before the steps run, so a throwing step leaves planned
    // steps behind: the latch stops the stream instead of letting them be skipped silently.
    latch_.run([&] {
        if (n > 0) {
            pcm_.insert(pcm_.end(), pcm, pcm + n);
            received_ += n;
        }
        std::vector<BufferedStepPlan> plans;
        schedule_.advance(received_, plans);
        for (const BufferedStepPlan& p : plans) run_step(p);
    });
}

void BufferedStreamingSession::finalize() {
    latch_.check();
    if (finalized_) return;
    latch_.run([&] {
        BufferedStepPlan last;
        if (schedule_.finish(received_, last)) run_step(last);
        finalized_ = true;
        regroup_words(/*flush_all=*/true);
    });
}

void BufferedStreamingSession::run_step(const BufferedStepPlan& plan) {
    if (plan.win_start < origin_ || plan.edge > origin_ + (int64_t)pcm_.size())
        throw std::logic_error("buffered streaming: window is not in the audio buffer");
    const int64_t total = plan.left + plan.chunk + plan.right;

    // 1. Log-mel of the whole window, normalized over the window (NeMo preprocessor on buffer.samples).
    std::vector<float> window(pcm_.begin() + (plan.win_start - origin_),
                              pcm_.begin() + (plan.edge - origin_));
    std::vector<float> feats;
    int n_mels = 0, T = 0;
    mel_.compute(window, feats, n_mels, T);   // valid_length gate: floor(samples / hop) frames

    // 2. Encoder over the window, full attention under the window-relative RC mask.
    const RcMask rc{left_, chunk_, right_};
    std::vector<float> enc_cf;               // channels-first [d_model, Tout]
    int d_model = d_model_, Tout = 0;
    enc_.forward_rc(feats, n_mels, T, rc, enc_cf, d_model, Tout);

    // 3. NeMo drops the left-context frames, then decodes the chunk (C frames),
    //    or on the last step everything after the left context.
    const int64_t left_frames = plan.left / kFrameSamples;
    const int64_t chunk_frames = plan.chunk / kFrameSamples;
    int64_t n = plan.is_last ? (int64_t)Tout - left_frames : chunk_frames;
    if (n < 0 || left_frames > Tout) n = 0;
    const int lf = (int)left_frames;
    const int dec = (int)std::min<int64_t>(n, (int64_t)Tout - lf);

    Step st;
    st.plan = plan;
    st.enc_frames = Tout;
    st.dec_frames = dec;

    std::vector<int32_t> emitted;
    std::vector<float> tm;                   // time-major decoded frames [dec, d_model]
    if (dec > 0) {
        // 4. Slice the chunk frames, apply the prompt kernel per frame, transpose to time-major.
        std::vector<float> cf((size_t)d_model * dec);   // [d_model, dec]
        for (int c = 0; c < d_model; ++c)
            for (int t = 0; t < dec; ++t)
                cf[(size_t)c * dec + t] = enc_cf[(size_t)c * Tout + (lf + t)];
        std::vector<float> projected;
        if (prompt_.present()) {
            prompt_.apply(cf, d_model, dec, prompt_index_, projected);
        } else {
            projected = std::move(cf);
        }
        tm.assign((size_t)dec * d_model, 0.0f);
        for (int t = 0; t < dec; ++t)
            for (int c = 0; c < d_model; ++c)
                tm[(size_t)t * d_model + c] = projected[(size_t)c * dec + t];

        // 5. RNN-T greedy over the chunk frames, carrying the decoder state.
        std::vector<TokenInfo> tokens;
        emitted = rnnt_decode_frames(pred_, joint_, tm, dec, d_model, state_,
                                     blank_id_, max_symbols_, nullptr, &tokens);
        for (size_t i = 0; i < emitted.size(); ++i) {
            TokenInfo ti = tokens[i];
            ti.frame += (int32_t)frames_done_;   // local -> absolute encoder frame
            word_tokens_.push_back(ti);
            hyp_.push_back(emitted[i]);
        }
        text_ = detokenize(ml_.config().tokenizer_pieces, hyp_);
        regroup_words(/*flush_all=*/false);
    }
    frames_done_ += dec;
    st.tokens = emitted;
    if (trace_on_) {
        st.enc_tm = std::move(tm);
        trace_.push_back(std::move(st));
    }

    // 6. Samples before this window's start are never read again (window starts only move forward).
    if (plan.win_start > origin_) {
        const size_t drop = (size_t)std::min<int64_t>(plan.win_start - origin_, (int64_t)pcm_.size());
        pcm_.erase(pcm_.begin(), pcm_.begin() + drop);
        origin_ += (int64_t)drop;
    }
}

void BufferedStreamingSession::regroup_words(bool flush_all) {
    words_ = group_words(word_tokens_, ml_.config().tokenizer_pieces, frame_sec_f_);
    if (words_.empty()) {
        words_finalized_ = 0;
    } else {
        words_finalized_ = flush_all ? words_.size() : (words_.size() - 1);
    }
    if (words_finalized_ < words_taken_) words_finalized_ = words_taken_;
}

namespace {

// End of the longest prefix of s[from..] that does not end inside a UTF-8 sequence.
// A lead byte whose sequence is not complete yet moves the end back to it.
size_t utf8_complete_end(const std::string& s, size_t from) {
    const size_t n = s.size();
    for (size_t back = 1; back <= 3 && back <= n - from; ++back) {
        const unsigned char c = (unsigned char)s[n - back];
        if ((c & 0xC0) == 0x80) continue;   // continuation byte: the lead byte is further back
        size_t need = 1;
        if (c >= 0xF0 && c <= 0xF4)      need = 4;
        else if (c >= 0xE0 && c <= 0xEF) need = 3;
        else if (c >= 0xC2 && c <= 0xDF) need = 2;
        return need > back ? n - back : n;
    }
    return n;
}

} // namespace

std::string check_buffered_window(int64_t left, int64_t chunk, int64_t right) {
    if (left < 0 || chunk < 1 || right < 0)
        return "stream window needs left >= 0, chunk >= 1 and right >= 0 (80 ms frames)";
    const int64_t max = kMaxBufferedWindowFrames;
    if (left > max || chunk > max || right > max || left + chunk + right > max)
        return "stream window is larger than 4000 frames (320 s) in total, left + chunk + right";
    return std::string();
}

std::string format_fixed(double v, int precision) {
    std::ostringstream os;
    os.imbue(std::locale::classic());
    os << std::fixed << std::setprecision(precision) << v;
    return os.str();
}

std::string buffered_info_json(int left, int chunk, int right, double frame_sec) {
    const int64_t first = ((int64_t)chunk + right) * kFrameSamples;
    const int64_t step = (int64_t)chunk * kFrameSamples;
    return "{\"kind\":\"buffered\",\"left\":" + std::to_string(left) +
           ",\"chunk\":" + std::to_string(chunk) + ",\"right\":" + std::to_string(right) +
           ",\"frame_sec\":" + format_fixed(frame_sec, 6) +
           ",\"latency_sec\":" + format_fixed((chunk + right) * frame_sec, 6) +
           ",\"step_sec\":" + format_fixed(chunk * frame_sec, 6) +
           ",\"first_step_samples\":" + std::to_string(first) +
           ",\"step_samples\":" + std::to_string(step) + "}";
}

std::string buffered_preset_json(unsigned index, int left, int chunk, int right,
                                 double frame_sec, bool is_default) {
    return "{\"preset\":" + std::to_string(index) + ",\"left\":" + std::to_string(left) +
           ",\"chunk\":" + std::to_string(chunk) + ",\"right\":" + std::to_string(right) +
           ",\"latency_sec\":" + format_fixed((chunk + right) * frame_sec, 2) +
           ",\"default\":" + (is_default ? "true" : "false") + "}";
}

std::string take_text_delta(std::string& emitted, const std::string& current, bool flush) {
    size_t k = 0;   // length of the common prefix of emitted and current
    const size_t lim = std::min(emitted.size(), current.size());
    while (k < lim && emitted[k] == current[k]) ++k;
    if (k < emitted.size()) return std::string();   // revised under emitted bytes: hold back
    const size_t end = flush ? current.size() : utf8_complete_end(current, k);
    if (end <= k) return std::string();
    std::string delta = current.substr(k, end - k);
    emitted += delta;
    return delta;
}

std::string BufferedStreamingSession::take_new_text() {
    latch_.check();
    return take_text_delta(emitted_, text_, finalized_);
}

std::vector<Word> BufferedStreamingSession::drain_words() {
    latch_.check();
    std::vector<Word> out;
    for (size_t i = words_taken_; i < words_finalized_ && i < words_.size(); ++i)
        out.push_back(words_[i]);
    words_taken_ = words_finalized_;
    return out;
}

} // namespace pk
