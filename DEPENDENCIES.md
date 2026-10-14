# Dependency log

Every third-party component, its license and where it came from. Updated whenever a dependency is added.

| Component | Version | License | Source | Used for | Ships in product? |
|---|---|---|---|---|---|
| microsoft/Windows-Camera `Samples/VirtualCamera` | commit 626f8b19c5f367602f2e89c6b314573d3776c9df | MIT (confirmed; LICENSE kept in `vcam-sample/LICENSE-microsoft-windows-camera`) | https://github.com/microsoft/Windows-Camera | Gate 0 spike base only | No (decision after the spike) |
| Microsoft.Windows.CppWinRT | 3.0.260520.1 | MIT (NuGet metadata; verify) | NuGet | Build dependency of the sample | Undecided |
| Microsoft.Windows.ImplementationLibrary (WIL) | 1.0.260126.7 | MIT (NuGet metadata; verify) | NuGet | Build dependency of the sample | Undecided |
| Microsoft.VCRTForwarders.140 | 1.1.0 | MIT (NuGet metadata; verify) | NuGet | Build dependency of the sample installer | Undecided |
| windows (windows-rs) | 0.58 | MIT OR Apache-2.0 (verify in `cargo metadata`) | crates.io | Spike producer | Undecided |
| numpy | 2.5.4 | BSD-3-Clause (pip metadata license field empty; per project page) | PyPI | Probe (measurement only) | No |
| opencv-python | 5.0.0.93 | Apache-2.0 (pip metadata) | PyPI | Probe (measurement only) | No |
| pytest | 9.1.1 | MIT (pip metadata license field empty; per project page) | PyPI | Probe tests | No |
