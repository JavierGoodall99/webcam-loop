// THROWAWAY (Gate 0). Test pattern with the machine-readable counter strip described in probe/counter_codec.py.
#pragma once
#include <cstdint>

namespace gate0 {

constexpr int kBlock = 8;
constexpr int kBits = 48;
constexpr uint16_t kMarkerLive = 0xA5C3;
constexpr uint16_t kMarkerSlate = 0x5A3C;

inline void PutPixel(uint8_t* row, int x, uint8_t r, uint8_t g, uint8_t b) {
    uint8_t* p = row + x * 4;
    p[0] = b; p[1] = g; p[2] = r; p[3] = 255;
}

// Draws only the counter strip at the top-left (48 blocks of 8x8 px, MSB first: 16 marker bits then 32 counter bits).
inline void DrawCounterStrip(uint8_t* bgra, int w, int h, int stride, uint32_t counter, uint16_t marker) {
    const uint64_t value = (static_cast<uint64_t>(marker) << 32) | counter;
    for (int i = 0; i < kBits && (i + 1) * kBlock <= w; ++i) {
        const uint8_t v = ((value >> (kBits - 1 - i)) & 1) ? 255 : 0;
        for (int y = 0; y < kBlock && y < h; ++y) {
            uint8_t* row = bgra + static_cast<size_t>(y) * stride;
            for (int x = i * kBlock; x < (i + 1) * kBlock; ++x) PutPixel(row, x, v, v, v);
        }
    }
}

// Draws 8 colour bars, a vertical sweep line that moves 8 px per frame, and the counter strip.
// A slate (marker == kMarkerSlate) is a flat dark grey frame with the strip, so it can never be mistaken for live video.
// Output is 32-bit BGRA, top-down, `stride` bytes per row.
inline void RenderTestPattern(uint8_t* bgra, int w, int h, int stride, uint32_t frameIndex, uint16_t marker) {
    static const uint8_t bars[8][3] = {
        {255,255,255},{255,255,0},{0,255,255},{0,255,0},{255,0,255},{255,0,0},{0,0,255},{0,0,0}};
    const bool slate = (marker == kMarkerSlate);
    const int sweepX = static_cast<int>((static_cast<uint64_t>(frameIndex) * 8) % static_cast<uint32_t>(w));
    for (int y = 0; y < h; ++y) {
        uint8_t* row = bgra + static_cast<size_t>(y) * stride;
        for (int x = 0; x < w; ++x) {
            if (slate) { PutPixel(row, x, 40, 40, 40); continue; }
            const uint8_t* c = bars[x * 8 / w];
            // Inverted bar colour, so the sweep is visible against every bar (white on the white bar would vanish).
            if (y >= kBlock && x >= sweepX && x < sweepX + 6) PutPixel(row, x, 255 - c[0], 255 - c[1], 255 - c[2]);
            else PutPixel(row, x, c[0], c[1], c[2]);
        }
    }
    DrawCounterStrip(bgra, w, h, stride, frameIndex, marker);
}

}  // namespace gate0
