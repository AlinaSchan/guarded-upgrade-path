Review aid for the Guarded Upgrade Path review draft (October 2026), published by Logos (https://logos.co/).
Runs offline on a deterministic local model. Not production software; no external review has been completed.

**Windows 10/11 (x64):** download the `*_x64-setup.exe` installer (installs per user, no administrator rights) or the `*_x64_en-US.msi`. The installer is unsigned, so SmartScreen may show "Windows protected your PC" once: *More info → Run anyway*.

**macOS:** download `*_aarch64.dmg` for Apple silicon (M1 or later) or `*_x64.dmg` for Intel, and drag the app to Applications. The app is unsigned and not notarized, so the first launch is blocked by Gatekeeper: either *System Settings → Privacy & Security → Open Anyway* after the first attempt, or clear the quarantine flag once with `xattr -dr com.apple.quarantine "/Applications/Guarded Upgrade Path.app"`.

**Linux and web browsers are not supported.** There is no Linux build; on any platform other than Windows or macOS the application refuses to start with an error.

The `gup-verify` CLI is attached separately for out-of-process verification of an exported bundle.
