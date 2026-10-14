// THROWAWAY (Gate 0). Shared-memory frame contract + tear-safe reader.
//
// Layout (little-endian): 64-byte header then pixels.
//   0 u32 magic 0x504C4357   4 u32 version=1   8 u32 width   12 u32 height   16 u32 stride
//   20 u32 format (1 = BGRA32)   24 u32 seq (odd while the writer is updating)   28 reserved
//   32 u64 frame_counter   40 u64 heartbeat_ms (GetTickCount64 at last write)   48..63 reserved
//   64.. pixels (stride * height bytes)
#pragma once
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

namespace gate0 {

constexpr uint32_t kMagic = 0x504C4357;      // "WCLP"
constexpr size_t kHeaderBytes = 64;
// How long a producer may be silent before the output is a slate, and how long a torn read may reuse the last good frame.
// Measured: with 1000 ms the output showed a frozen frame as "live" for up to 0.94 s after the producer died.
constexpr uint64_t kStaleAfterMs = 250;

enum class ReadStatus { Ok, BadHeader, SizeMismatch, Stale, Torn };

inline ReadStatus TryReadFrame(const uint8_t* base, size_t mappedSize, uint64_t nowMs,
                               uint32_t expectW, uint32_t expectH,
                               uint8_t* dst, uint32_t dstStride, uint64_t* frameCounter) {
    if (!base || mappedSize < kHeaderBytes) return ReadStatus::BadHeader;
    auto rd32 = [&](size_t o) { uint32_t v; std::memcpy(&v, base + o, 4); return v; };
    auto rd64 = [&](size_t o) { uint64_t v; std::memcpy(&v, base + o, 8); return v; };
    const auto* seqPtr = reinterpret_cast<const std::atomic<uint32_t>*>(base + 24);

    const uint32_t seq1 = seqPtr->load(std::memory_order_acquire);
    if (seq1 & 1u) return ReadStatus::Torn;

    if (rd32(0) != kMagic || rd32(4) != 1 || rd32(20) != 1) return ReadStatus::BadHeader;
    const uint32_t w = rd32(8), h = rd32(12), stride = rd32(16);
    const uint64_t counter = rd64(32), heartbeat = rd64(40);
    if (w != expectW || h != expectH) return ReadStatus::SizeMismatch;
    if (stride < w * 4u || kHeaderBytes + static_cast<size_t>(stride) * h > mappedSize) return ReadStatus::BadHeader;
    if (nowMs > heartbeat && nowMs - heartbeat > kStaleAfterMs) return ReadStatus::Stale;

    const uint32_t rowBytes = w * 4u;
    for (uint32_t y = 0; y < h; ++y)
        std::memcpy(dst + static_cast<size_t>(y) * dstStride, base + kHeaderBytes + static_cast<size_t>(y) * stride, rowBytes);

    std::atomic_thread_fence(std::memory_order_acquire);
    if (seqPtr->load(std::memory_order_acquire) != seq1) return ReadStatus::Torn;
    *frameCounter = counter;
    return ReadStatus::Ok;
}

// Reads frames from the section and keeps the last good one. A torn read (the writer was mid-update) reuses it for up to
// kStaleAfterMs instead of flashing the slate; every other non-Ok status (stale, wrong size, bad header) returns nullptr so
// the caller shows the slate. Measured: without this the output flashed a one-frame slate every 0.3 to 1 s while live.
class LatestGoodFrame {
public:
    explicit LatestGoodFrame(size_t bytes) : m_tmp(bytes), m_good(bytes) {}

    // Returns the BGRA frame to show (stride = expectW * 4), or nullptr to show the slate.
    const uint8_t* Update(const uint8_t* base, size_t mappedSize, uint64_t nowMs,
                          uint32_t expectW, uint32_t expectH, uint64_t* frameCounter) {
        uint64_t counter = 0;
        ReadStatus st = ReadStatus::Torn;
        for (int attempt = 0; attempt < 3 && st == ReadStatus::Torn; ++attempt)
            st = TryReadFrame(base, mappedSize, nowMs, expectW, expectH, m_tmp.data(), expectW * 4, &counter);

        if (st == ReadStatus::Ok) {
            m_tmp.swap(m_good);
            m_goodMs = nowMs;
            m_goodCounter = counter;
            m_haveGood = true;
        } else if (!(st == ReadStatus::Torn && m_haveGood && nowMs >= m_goodMs && nowMs - m_goodMs <= kStaleAfterMs)) {
            return nullptr;
        }
        *frameCounter = m_goodCounter;
        return m_good.data();
    }

private:
    std::vector<uint8_t> m_tmp, m_good;
    uint64_t m_goodMs = 0, m_goodCounter = 0;
    bool m_haveGood = false;
};

}  // namespace gate0
