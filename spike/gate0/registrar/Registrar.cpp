// THROWAWAY (Gate 0). Minimal virtual camera holder: MFCreateVirtualCamera + Start, then wait until told to stop.
// Skips the sample installer's extra AddProperty bookkeeping (that is what demanded elevation).
//
//   Registrar.exe [--lifetime session|system] [--access user|all] [--name "Gate0 VCam"]   create and hold
//   Registrar.exe --stop                                                                  ask a running instance to remove the camera and exit
//
// Exit codes: 0 ok, 1 usage, otherwise the failing HRESULT.
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfvirtualcamera.h>
#include <cstdio>
#include <cwchar>

#pragma comment(lib, "mfplat.lib")
#pragma comment(lib, "mfsensorgroup.lib")
#pragma comment(lib, "ole32.lib")

static const wchar_t* kStopEvent = L"Local\\gate0-registrar-stop";
static const wchar_t* kClsid = L"{7B89B92E-FE71-42D0-8A41-E137D06EA184}";

static HANDLE g_stop = nullptr;
static BOOL WINAPI OnCtrl(DWORD) { if (g_stop) SetEvent(g_stop); return TRUE; }

int wmain(int argc, wchar_t** argv) {
    MFVirtualCameraLifetime lifetime = MFVirtualCameraLifetime_Session;
    MFVirtualCameraAccess access = MFVirtualCameraAccess_CurrentUser;
    const wchar_t* name = L"Gate0 VCam";

    for (int i = 1; i < argc; ++i) {
        if (!wcscmp(argv[i], L"--stop")) {
            HANDLE e = OpenEventW(EVENT_MODIFY_STATE, FALSE, kStopEvent);
            if (!e) { std::wprintf(L"no running registrar found\n"); return 1; }
            SetEvent(e); CloseHandle(e); std::wprintf(L"stop requested\n"); return 0;
        } else if (!wcscmp(argv[i], L"--lifetime") && i + 1 < argc) {
            lifetime = !wcscmp(argv[++i], L"system") ? MFVirtualCameraLifetime_System : MFVirtualCameraLifetime_Session;
        } else if (!wcscmp(argv[i], L"--access") && i + 1 < argc) {
            access = !wcscmp(argv[++i], L"all") ? MFVirtualCameraAccess_AllUsers : MFVirtualCameraAccess_CurrentUser;
        } else if (!wcscmp(argv[i], L"--name") && i + 1 < argc) {
            name = argv[++i];
        } else { std::wprintf(L"usage: Registrar [--lifetime session|system] [--access user|all] [--name N] | --stop\n"); return 1; }
    }

    HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(hr)) { std::wprintf(L"CoInitializeEx failed 0x%08x\n", hr); return hr; }
    hr = MFStartup(MF_VERSION);
    if (FAILED(hr)) { std::wprintf(L"MFStartup failed 0x%08x\n", hr); return hr; }

    IMFVirtualCamera* cam = nullptr;
    hr = MFCreateVirtualCamera(MFVirtualCameraType_SoftwareCameraSource, lifetime, access, name, kClsid, nullptr, 0, &cam);
    if (FAILED(hr)) { std::wprintf(L"MFCreateVirtualCamera failed 0x%08x\n", hr); return hr; }
    hr = cam->Start(nullptr);
    if (FAILED(hr)) { std::wprintf(L"Start failed 0x%08x\n", hr); cam->Release(); return hr; }

    g_stop = CreateEventW(nullptr, TRUE, FALSE, kStopEvent);
    SetConsoleCtrlHandler(OnCtrl, TRUE);
    std::wprintf(L"camera running: name='%s' lifetime=%d access=%d pid=%lu\n", name, static_cast<int>(lifetime), static_cast<int>(access), GetCurrentProcessId());
    std::fflush(stdout);

    WaitForSingleObject(g_stop, INFINITE);

    std::wprintf(L"stopping: Remove + Shutdown\n");
    hr = cam->Remove();
    if (FAILED(hr)) std::wprintf(L"Remove returned 0x%08x\n", hr);
    cam->Shutdown();
    cam->Release();
    MFShutdown();
    CoUninitialize();
    std::wprintf(L"camera removed\n");
    return 0;
}
