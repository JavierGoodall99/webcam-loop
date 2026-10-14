// THROWAWAY (Gate 0). Opens the shared section by name, reads one 1280x720 frame with TryReadFrame, writes it as raw BGRA.
// Usage: ReaderCheck <section-name> <outfile>
// Exit: 0 ok, 2 bad/size/torn, 3 stale, 4 section absent. Prints "counter=<n> status=<name>".
#include <cstdio>
#include <vector>
#include "SharedFrame.h"
#include "SharedSection.h"

int main(int argc, char** argv) {
    if (argc != 3) { std::fprintf(stderr, "usage: ReaderCheck name outfile\n"); return 1; }
    wchar_t name[256];
    if (!MultiByteToWideChar(CP_UTF8, 0, argv[1], -1, name, 256)) return 1;

    constexpr uint32_t W = 1280, H = 720;
    constexpr size_t kMapBytes = gate0::kHeaderBytes + static_cast<size_t>(W) * H * 4;
    gate0::SharedSection section;
    if (!section.EnsureOpen(name, kMapBytes, false)) { std::printf("counter=0 status=absent err=%lu\n", section.lastError()); return 4; }

    std::vector<uint8_t> buf(static_cast<size_t>(W) * H * 4);
    uint64_t counter = 0;
    gate0::ReadStatus st = gate0::ReadStatus::Torn;
    for (int i = 0; i < 3 && st == gate0::ReadStatus::Torn; ++i)
        st = gate0::TryReadFrame(section.base(), section.size(), GetTickCount64(), W, H, buf.data(), W * 4, &counter);

    switch (st) {
    case gate0::ReadStatus::Ok: break;
    case gate0::ReadStatus::Stale: std::printf("counter=0 status=stale\n"); return 3;
    default: std::printf("counter=0 status=bad(%d)\n", static_cast<int>(st)); return 2;
    }
    FILE* f = std::fopen(argv[2], "wb");
    if (!f) return 1;
    std::fwrite(buf.data(), 1, buf.size(), f);
    std::fclose(f);
    std::printf("counter=%llu status=ok\n", static_cast<unsigned long long>(counter));
    return 0;
}
