// THROWAWAY (Gate 0). Writes one raw BGRA frame of the test pattern so the Python codec can be cross-checked.
// Usage: PatternDump <width> <height> <frameIndex> <marker> <outfile>
#include <cstdio>
#include <cstdlib>
#include <vector>
#include "TestPattern.h"

int main(int argc, char** argv) {
    if (argc != 6) { std::fprintf(stderr, "usage: PatternDump w h frameIndex marker outfile\n"); return 2; }
    const int w = std::atoi(argv[1]), h = std::atoi(argv[2]);
    const uint32_t frameIndex = static_cast<uint32_t>(std::strtoull(argv[3], nullptr, 10));
    const uint16_t marker = static_cast<uint16_t>(std::strtoul(argv[4], nullptr, 10));
    std::vector<uint8_t> buf(static_cast<size_t>(w) * h * 4);
    gate0::RenderTestPattern(buf.data(), w, h, w * 4, frameIndex, marker);
    FILE* f = std::fopen(argv[5], "wb");
    if (!f) return 1;
    std::fwrite(buf.data(), 1, buf.size(), f);
    std::fclose(f);
    return 0;
}
