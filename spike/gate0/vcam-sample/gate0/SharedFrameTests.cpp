#include <cassert>
#include <cstdio>
#include <cstring>
#include <vector>
#include <atomic>
#include "SharedFrame.h"
using namespace gate0;

static std::vector<uint8_t> MakeBuffer(uint32_t w, uint32_t h, uint64_t counter, uint64_t heartbeat, uint32_t seq = 2) {
    std::vector<uint8_t> b(64 + static_cast<size_t>(w) * 4 * h, 0);
    auto put32 = [&](size_t o, uint32_t v) { std::memcpy(&b[o], &v, 4); };
    auto put64 = [&](size_t o, uint64_t v) { std::memcpy(&b[o], &v, 8); };
    put32(0, kMagic); put32(4, 1); put32(8, w); put32(12, h); put32(16, w * 4); put32(20, 1); put32(24, seq);
    put64(32, counter); put64(40, heartbeat);
    for (size_t i = 64; i < b.size(); ++i) b[i] = static_cast<uint8_t>(i);
    return b;
}

int main() {
    std::vector<uint8_t> dst(4 * 4 * 2);
    uint64_t counter = 0;

    { auto b = MakeBuffer(4, 2, 77, 1000);
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::Ok);
      assert(counter == 77 && dst[0] == 64 && dst[31] == 95); }

    { auto b = MakeBuffer(4, 2, 1, 1000); b[0] ^= 0xFF;
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { auto b = MakeBuffer(8, 2, 1, 1000);
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::SizeMismatch); }

    { auto b = MakeBuffer(4, 2, 1, 1000);
      assert(TryReadFrame(b.data(), b.size(), 2501, 4, 2, dst.data(), 16, &counter) == ReadStatus::Stale); }

    { auto b = MakeBuffer(4, 2, 1, 1000, /*seq=*/3);   // odd = writer mid-update
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::Torn); }

    { auto b = MakeBuffer(4, 2, 1, 1000);               // mapping shorter than the header claims
      assert(TryReadFrame(b.data(), b.size() - 8, 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { assert(TryReadFrame(nullptr, 0, 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { auto b = MakeBuffer(4, 2, 1, 5000);                // heartbeat in the future must not underflow into "stale"
      assert(TryReadFrame(b.data(), b.size(), 1000, 4, 2, dst.data(), 16, &counter) == ReadStatus::Ok); }

    // --- LatestGoodFrame: a torn read must reuse the previous good frame briefly (no slate flash), but never for long ---
    {   LatestGoodFrame holder(4 * 4 * 2);
        auto good = MakeBuffer(4, 2, 10, 1000);
        uint64_t c = 0;
        const uint8_t* f = holder.Update(good.data(), good.size(), 1100, 4, 2, &c);
        assert(f && c == 10 && f[0] == 64);

        auto torn = MakeBuffer(4, 2, 11, 1100, /*seq=*/3);
        f = holder.Update(torn.data(), torn.size(), 1133, 4, 2, &c);
        assert(f && c == 10 && f[0] == 64);                       // previous good frame, not a torn mixture

        f = holder.Update(torn.data(), torn.size(), 1100 + kStaleAfterMs + 1, 4, 2, &c);
        assert(f == nullptr);                                     // torn for too long -> slate
    }
    {   LatestGoodFrame holder(4 * 4 * 2);                        // nothing good yet: torn must not invent a frame
        auto torn = MakeBuffer(4, 2, 1, 1000, 3);
        uint64_t c = 0;
        assert(holder.Update(torn.data(), torn.size(), 1010, 4, 2, &c) == nullptr);
    }
    {   LatestGoodFrame holder(4 * 4 * 2);                        // a size change is not a torn read: never hold across it
        auto good = MakeBuffer(4, 2, 10, 1000);
        auto wrong = MakeBuffer(8, 2, 11, 1010);
        uint64_t c = 0;
        assert(holder.Update(good.data(), good.size(), 1020, 4, 2, &c) != nullptr);
        assert(holder.Update(wrong.data(), wrong.size(), 1030, 4, 2, &c) == nullptr);
    }
    {   LatestGoodFrame holder(4 * 4 * 2);                        // producer stopped: stale means slate even though a good frame exists
        auto good = MakeBuffer(4, 2, 10, 1000);
        uint64_t c = 0;
        assert(holder.Update(good.data(), good.size(), 1020, 4, 2, &c) != nullptr);
        assert(holder.Update(good.data(), good.size(), 1020 + kStaleAfterMs + 50, 4, 2, &c) == nullptr);
    }

    std::puts("SharedFrameTests: all passed");
    return 0;
}
