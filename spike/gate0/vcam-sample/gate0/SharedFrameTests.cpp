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

    std::puts("SharedFrameTests: all passed");
    return 0;
}
