# Opt-in install, update, uninstall and rollback protocol

**Status:** PROPOSED policy; no executable installer, uninstaller or release exists in this workstream. Always require review and user consent before changing host state.

## Preflight and manifest

1. Identify host OS, architecture, minimum OS/kernel, user identity and free space without requiring admin privileges. Confirm Linux distribution/GLIBC, macOS Apple Silicon, native Windows ARM64 vs x64, or Raspberry Pi OS 64-bit. Report unsupported targets; never silently select an emulator binary.
2. Display artifact version, immutable commit, architecture, exact install and config paths, publisher signature, SHA-256 checksum and required operating-system dependencies. Abort if signature/checksum or architecture does not match.
3. Offer a user-chosen install directory and model library location. **Never** traverse, edit, migrate, overwrite or remove Jan/Ollama/other apps' models or service configuration implicitly. No auto-discovery outside consented read scopes.
4. Reserve an atomic staging directory on the same volume where possible. Validate space/permissions and reject symlink traversal out of installation scope, including rollback/cleanup paths. Preserve an immutable rollback snapshot of only TIHULU-owned application binaries/metadata (not foreign model stores).
5. Default to **On-Demand / Background OFF / no keep-warm models**. No systemd/launchd/Windows Service or Scheduled Task registration; no auto-start login item; no listener/firewall rule; no unrestricted hardware access. Independent GPU-host service is a **separate explicit opt-in** with ownership and impact shown.

## Proposed OS packaging and registration

| OS | User-mode initial choice | Later managed bundle | Privilege / service policy |
| --- | --- | --- | --- |
| Linux x64/ARM64 | CLI under user-owned directory or reviewed .deb | Reviewed distro-specific .deb / AppImage for actual Tauri app | No `sudo` in a generic fetched pipe; package scripts cannot stop existing readsb/tar1090 or enable daemon |
| macOS ARM64 | Signed .app after app exists | Notarized .dmg/.pkg only after Gatekeeper tests | Quit through native app delegate before replace/removal; no launch agent by default |
| Windows x64/ARM64 | User-scope signed package | MSIX/NSIS after per-ISA native tests | UAC only for explicitly selected machine-wide install; no service/task by default |
| Pi 5/AI HAT+ 2 | ARM64 CLI or reviewed Pi OS package | Optional headless UI; Hailo adapter separate | No PCIe config, DKMS, driver/HailoRT install or system service changes automatically |

**Version pin:** Store package manifest, source commit, selected optional features, installation directory and checksum. Hailo runtime, NVIDIA driver, model files and external providers are separately owned prerequisites; do not treat them as package contents.

## Normal Quit before update/removal

- Stop accepting work, cancel app-owned requests and remote client leases, wait a bounded grace period, then terminate **only retained app-owned child handles/process groups/jobs**. Release owned models, GPU contexts, audio buffers, sockets, captures and SDR/USB handles; record uncertain residual allocations for user review. This is a runtime contract, not something a generic uninstaller can accomplish by killing process names.
- Preserve independent hosts used by other authorized sessions; never terminate Jan, Ollama, readsb, tar1090, external GPU host or other apps. Require explicit separate 'Quit All TIHULU-Owned Services' authorization for eligible independent services.
- If the app still runs or owns leases, offer Cancel / retry after graceful Quit. No forced global service stop. Abrupt OS crash is covered only after separate supervised orphan/TTL acceptance.

## Transactional update, uninstall and rollback

1. Stop only the versioned app instance; verify handles and ownership records; retain old executable and manifest.
2. Install to staged versioned path; verify integrity; switch only the TIHULU-owned launcher/registration atomically where supported; preserve old binary until smoke check and rollback window finish.
3. On smoke failure, restore prior known-good version and its launcher/registration; restore only explicitly migrated TIHULU-owned metadata via version-aware migration/backup. Do not roll back foreign app state.
4. Uninstall removes only verified package-owned binaries, shortcuts and optional service registration **if it was installed by this manifest and no other client owns it**. Prompt separately before deleting user settings or TIHULU-owned downloaded model files; default is **keep**. Never delete external models or recordings.
5. Provide dry-run operation list and human-readable audit result (paths, hashes, handles, remaining leases, skipped foreign items); do not mark successful cleanup if a child, model context or package file remains.

## Threat and regression checks

- No `curl | sh`, broad `sudo rm -rf`, `pkill`/`killall`, process-name kills, arbitrary `taskkill /IM`, recursive user-home deletions or unscoped symlink traversal.
- Test partial install, interruption/power loss, rollback, duplicate installs, two active TIHULU clients, a borrowed Jan/Ollama/readsb/tar1090 connection, mounted external SSD removal and read-only filesystem.
- **Pending:** actual signed packages, Tauri app existence, Windows Job Objects, launchd/systemd behavior, Pi + Hailo, CUDA/Metal, SDR handle release and per-distro validation. No installer is run by this PR.
