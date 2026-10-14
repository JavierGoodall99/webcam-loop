// THROWAWAY (Gate 0). Opens (or creates) the named section; retries are throttled so a missing producer costs nothing.
#pragma once
#include <windows.h>
#include <sddl.h>
#include <cstdint>

namespace gate0 {

class SharedSection {
public:
    ~SharedSection() { Close(); }

    bool EnsureOpen(const wchar_t* name, size_t len, bool createIfMissing) {
        if (m_base) return true;
        const ULONGLONG now = GetTickCount64();
        if (m_lastAttempt && now - m_lastAttempt < 500) return false;
        m_lastAttempt = now;

        m_handle = OpenFileMappingW(FILE_MAP_READ, FALSE, name);
        if (!m_handle && createIfMissing) {
            PSECURITY_DESCRIPTOR sd = nullptr;
            // Everyone: generic all. Spike only; the product tightens this.
            if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"D:(A;;GA;;;WD)", SDDL_REVISION_1, &sd, nullptr)) return false;
            SECURITY_ATTRIBUTES sa{ sizeof(sa), sd, FALSE };
            m_handle = CreateFileMappingW(INVALID_HANDLE_VALUE, &sa, PAGE_READWRITE, 0, static_cast<DWORD>(len), name);
            LocalFree(sd);
        }
        if (!m_handle) { m_lastError = GetLastError(); return false; }
        m_base = static_cast<const uint8_t*>(MapViewOfFile(m_handle, FILE_MAP_READ, 0, 0, len));
        if (!m_base) { m_lastError = GetLastError(); CloseHandle(m_handle); m_handle = nullptr; return false; }
        m_len = len;
        return true;
    }
    const uint8_t* base() const { return m_base; }
    size_t size() const { return m_len; }
    DWORD lastError() const { return m_lastError; }

    void Close() {
        if (m_base) { UnmapViewOfFile(m_base); m_base = nullptr; }
        if (m_handle) { CloseHandle(m_handle); m_handle = nullptr; }
    }

private:
    HANDLE m_handle = nullptr;
    const uint8_t* m_base = nullptr;
    size_t m_len = 0;
    ULONGLONG m_lastAttempt = 0;
    DWORD m_lastError = 0;
};

}  // namespace gate0
