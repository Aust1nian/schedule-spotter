# Releasing Schedule Spotter

How to build, check and publish a new version. One release run builds every variant. Each client downloads one build of one variant, chosen by Austin.

| What | Where |
| --- | --- |
| Build, sign, install-test every variant | `.github/workflows/release.yml` (on a `v*` tag, or by hand) |
| Download site and upload script | `site/` (`site/README.md`) |
| What the salesperson hands an owner who gets stuck | `RECOVERY_CARD.md`, `RECOVERY_CARD.pdf` |

## Once: signing secrets

Each platform signs only when its secrets exist. Until then, its files are named `…-UNSIGNED` and are for internal testing only.

**macOS (Apple Developer Program, Austin's account).**

1. In Xcode or at developer.apple.com, create a **Developer ID Application** certificate. Export it from Keychain Access as a `.p12` with a password.
2. In App Store Connect, open **Users and Access → Integrations → App Store Connect API**. Create a **Team Key** with the Developer role, then download the `.p8` file. Note its Key ID and the Issuer ID.
3. Store them (`gh` must be logged in to the repo):

```sh
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE
gh secret set APPLE_CERTIFICATE_PASSWORD           # the .p12 password
gh secret set APPLE_SIGNING_IDENTITY --body "Developer ID Application: Austin Burnes (TEAMID)"
gh secret set APPLE_API_ISSUER --body "<issuer id>"
gh secret set APPLE_API_KEY --body "<key id>"
gh secret set APPLE_API_KEY_P8 < AuthKey_<key id>.p8
```

**Windows: off by Austin's decision (DECISIONS.md, 2026-10-11).** To turn it on later, set up Azure Artifact Signing and add `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`, `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT` and `AZURE_SIGNING_PROFILE`. The workflow then signs:
- the app and installer, through Tauri's `signCommand`;
- the llama.cpp DLLs;
- the installer again after the model is appended.

This path hasn't run yet, so its first signed run needs watching.

## Each release

1. **Version.** Set the same version in `Cargo.toml` (`[workspace.package]`), `app/src-tauri/tauri.conf.json` and `app/package.json`. The workflow refuses a tag that doesn't match.
2. **Release candidate.** From the main branch:

   ```sh
   gh workflow run release.yml --ref main            # all variants: off, C, A, B-E2B
   gh workflow run release.yml --ref main -f variants='["C"]'   # or just some
   ```

   The run page's summary has a table with each file's size, whether it's signed, and the install test result on every runner. `windows-11-arm` is logged but doesn't block. Signed macOS builds also report how long notarization took.

   The release-manifest artifact holds `releases.json` and `SHA256SUMS`.
3. **Manual checks.** CI can't do these. Use one real Mac, ideally an older 8 GB Apple Silicon one, and one Windows 10 22H2 or 11 PC, each on ordinary office internet. Download through the site as an owner would, so the browser marks the file:
   - **First launch.**
     - Mac: only the one "downloaded from the Internet" prompt (signed), or the Privacy & Security "Open Anyway" route (unsigned). Compare it with the recovery card.
     - Windows: the SmartScreen prompt and its publisher line. Compare it with the recovery card.
   - **Open file…:** pick an `.xlsx` and a `.csv` from Documents. Then drag a file onto the window.
   - **Paste:** copy cells in Excel or Numbers and paste them.
   - **Save…:**
     - Save the workbook to Documents under a name with spaces and an accent (`Semana de José.xlsx`), then reopen it: every answer comes back.
     - Choose CSV in the same dialog.
     - On the Mac this also checks the App Sandbox, which allows writing only the picked file.
   - **About:**
     - The version and the model are right, and smart matching says ready.
     - The Licenses folder is there. Mac: right-click the app, choose **Show Package Contents**, then go to Contents/Resources/Licenses. Windows: `%LOCALAPPDATA%\Schedule Spotter\Licenses`.
     - **Save diagnostics…** shows placeholders only.
   - **Wi-Fi off:** everything still works.
   - **Uninstall:**
     - Windows: Settings → Apps → Schedule Spotter → Uninstall. The folder is gone afterwards.
     - Mac: drag the app to the Trash. macOS keeps the sandbox container in `~/Library/Containers/com.tightline.schedulespotter`; it holds no schedule data.
4. **Tag.** `git tag v<version> && git push origin v<version>`. This runs the same workflow, so its files are the ones that get published.
5. **Publish** the variant Austin picked to the Pi:

   ```sh
   site/upload.sh --run <run id> --variant <C|A|B-E2B|off> --host pi@<pi address> --prune
   ```

   It refuses files that failed an install test or aren't signed (`--allow-unsigned` for an internal test site). It checks every SHA-256, then switches the site over in one step.
6. **Tell the salespeople.** The app never checks for updates (spec), so the salesperson tells clients about a new version. About shows the version a client has. Hand out `RECOVERY_CARD.pdf` with any change to the prompts.

## What a build contains

- **macOS:** one universal app (Apple Silicon and Intel). It has a hardened runtime and the App Sandbox. The model is in `Contents/Resources/model/` and the licenses are in `Contents/Resources/Licenses/`. It ships in a disk image with a link to Applications, made by `tools/release/make-dmg.sh`.
  - **Signed builds:** Tauri signs and notarizes the app and staples its ticket, so Gatekeeper can check it offline. The workflow then signs, notarizes and staples the disk image.
- **Windows:** Tauri's NSIS installer, per user and needing no administrator password. It carries Microsoft's WebView2 offline installer and the licenses. NSIS can't hold more than about 2 GB, so the model is appended after the installer's own data (`tools/release/append-model.py`). During install, the app copies it into `model\` and checks its SHA-256.
- **Sizes:** the release summary prints the real numbers. With no model: 16 MB for the Mac and 223 MB for Windows, most of which is Microsoft's WebView2 offline installer. Add the model for the others: about 1 GB for C, 2.7 GB for A, and 3.6 GB for B-E2B.
