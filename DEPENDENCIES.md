# Dependency log

Every third-party component, its license and where it came from. Updated whenever a dependency is added.
"Verified" means I read the license text or machine-readable metadata, not that I assumed it.

| Component | Version | License | Status | Source | Used for | Ships in product? |
|---|---|---|---|---|---|---|
| microsoft/Windows-Camera `Samples/VirtualCamera` | commit 626f8b19c5f367602f2e89c6b314573d3776c9df | MIT | Verified (LICENSE file kept in `spike/gate0/vcam-sample/LICENSE-microsoft-windows-camera`) | https://github.com/microsoft/Windows-Camera | Gate 0 spike base only; modified in `SimpleMediaStream.cpp` and `SimpleFrameGenerator.*` | No decision yet. MIT would allow reuse with the notice kept; Plan 3 decides. |
| Microsoft.Windows.CppWinRT (NuGet) | 3.0.260520.1 | MIT | Verified (LICENSE in package) | NuGet | Build dependency of the sample | Undecided |
| Microsoft.Windows.ImplementationLibrary / WIL (NuGet) | 1.0.260126.7 | MIT | Verified (LICENSE in package) | NuGet | Build dependency of the sample | Undecided |
| Microsoft.VCRTForwarders.140 (NuGet) | 1.1.0 | MIT expected | **Not verified**: package metadata only gives a license URL (github.com/Microsoft/vcrt-forwarders/blob/master/LICENSE) | NuGet | Build dependency of the sample's installer project only; not used by the media source DLL | No (not used in the spike's DLL) |
| windows (windows-rs) and its `windows-*` crates | 0.58.0 (windows-core 0.58.0, windows-result 0.2.0, windows-strings 0.1.0, windows-targets 0.52.6) | MIT OR Apache-2.0 | Verified (`cargo metadata`) | crates.io | Spike producer | Undecided |
| numpy | 2.5.4 | BSD-3-Clause | Per project page (pip metadata license field is empty) | PyPI | Probe (measurement only) | No |
| opencv-python | 5.0.0.93 | Apache-2.0 | pip metadata | PyPI | Probe (measurement only) | No |
| pytest | 9.1.1 | MIT | Per project page (pip metadata license field is empty) | PyPI | Probe and server tests | No |
| Python standard library (`http.server`) | 3.13.2 | PSF | Standard | python.org | Browser-probe result server | No |
| Node.js built-in test runner | 24.11.0 | MIT | Standard | nodejs.org | JS codec tests | No |
| Windows SDK / MSVC Build Tools | 10.0.26100.0 / 19.44.35220 | Microsoft license terms | Toolchain, not redistributed | Visual Studio Installer | Build | n/a |

Not used, by policy (see the spec, section 2): the OBS virtual camera, `pyvirtualcam`, and any source from `haxybaxy/video-pedal`.
