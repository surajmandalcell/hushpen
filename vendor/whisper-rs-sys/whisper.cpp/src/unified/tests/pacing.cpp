#include "../buffered_streaming.hpp"

#include <algorithm>
#include <stdexcept>

static void check(bool ok) {
    if (!ok) throw std::logic_error("Unified pacing skipped, repeated, or mis-timed audio");
}

static void covers(int64_t total, int block) {
    pk::BufferedSchedule schedule(70, 7, 7);
    pk::BufferedStepPlan step;
    int64_t committed = 0;
    int64_t window_start = 0;
    auto consume = [&] {
        check(step.win_start >= window_start);
        check(step.win_start + step.left == committed);
        check(step.edge == committed + step.chunk + step.right);
        committed += step.chunk;
        window_start = step.win_start;
    };
    for (int64_t at = 0; at < total;) {
        at = std::min(total, at + block);
        while (schedule.next(at, step)) {
            check(step.right == 7 * 1280 && step.chunk == schedule.chunk_frames() * 1280);
            consume();
            schedule.record_step(10.0);
        }
    }
    if (schedule.finish(total, step)) {
        check(step.is_last && step.right == 0);
        consume();
    }
    check(committed == total && !schedule.finish(total, step));
}

int main() {
    constexpr int frame = 1280;
    pk::BufferedSchedule pace(70, 7, 7);
    pk::BufferedStepPlan step;
    check(!pace.next(14 * frame, step));
    check(pace.next(14 * frame + 1, step) && step.chunk == 7 * frame);
    for (int i = 0; i < 6; ++i) {
        pace.record_step(i % 2 == 0 ? 0.6 : 0.5);  // Single slow steps keep the default.
        check(pace.left_frames() == 70 && pace.chunk_frames() == 7);
    }
    for (int expected : {14, 28, 50, 50}) {
        pace.record_step(10);
        pace.record_step(10);
        check(pace.left_frames() == 28 && pace.chunk_frames() == expected);
    }
    // Stop on each side of a window edge before and after each pace change.
    for (int block : {1, 257, 4096, 1000000}) {
        for (int frames : {0, 14, 21, 35, 49, 77, 105, 155, 205, 350}) {
            for (int tail : {-1, 0, 1, 159}) {
                covers(std::max(0, frames * frame + tail), block);
            }
        }
    }
}
