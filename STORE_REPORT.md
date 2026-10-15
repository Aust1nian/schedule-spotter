# What a Microsoft Store listing would take

v1 ships without the Store (DECISIONS.md, 2026-10-10). This note is the spec's "short report on what a Microsoft Store listing would take". Checked 2026-10-12.

## Why it would matter

Apps installed from the Store never show the SmartScreen "Windows protected your PC" warning. A direct download shows it until the release builds reputation, and an unsigned build shows "Unknown publisher" as well ([Microsoft: SmartScreen reputation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)). For a non-technical owner, the Store is the smoothest Windows install there is.

## Two routes

| | Route 1: list the existing installer (EXE/MSI) | Route 2: MSIX package |
| --- | --- | --- |
| What Microsoft gets | A Store page that links to our installer, hosted at a stable, versioned HTTPS address (`get.tightline.systems` would do) | The app itself, which Microsoft hosts and delivers |
| Signing | We sign the installer and every .exe and .dll with a certificate from the Microsoft Trusted Root Program; self-signed doesn't count ([Microsoft: code signing options](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)) | Microsoft re-signs the package after certification, so no certificate is needed |
| Tauri support | Built in: Tauri documents this route ([Tauri: Microsoft Store](https://v2.tauri.app/distribute/microsoft-store/)) | Not built in: Tauri produces NSIS and MSI, so MSIX would need Microsoft's packaging tools and its own install tests |
| Our 2 GB problem | Unchanged: it's our installer, model appended after the NSIS data | Gone: an MSIX holds the model as an ordinary file |
| Cost | Signing: Azure Artifact Signing about $9.99/month (US/Canada individuals), or an OV certificate at about $150–300/year | None beyond the account |

## What either route needs

- **A Partner Center developer account.** Individual developers have joined free since September 2025, after an ID check with a government ID and a selfie ([Windows Developer Blog](https://blogs.windows.com/windowsdeveloper/?p=57557)). A company account used to cost a one-time fee. One third-party report says that fee was dropped in 2026; this is unconfirmed.
- **A silent install.** The Store requires it ([Tauri](https://v2.tauri.app/distribute/microsoft-store/)). Ours already has one: the install test runs `/S` as a standard user.
- **The WebView2 offline installer** embedded, which we already do.
- **A publisher name different from the product name.** Ours is "Tight Line" and the product is "Schedule Spotter", so this is fine.
- **Updates through the installer.** Tauri's guide says the linked installer must "handle auto-updates". Schedule Spotter never checks for updates, by design (spec: "Updates"). Route 1 would need Microsoft's certification team to accept an app that only updates when the owner reinstalls. Route 2 avoids the question, because the Store updates MSIX apps itself.
- **A Store listing:** screenshots, a description, a privacy policy URL (the download site can host one), and an age rating questionnaire. Certification usually takes a few days per submission.

## Recommendation for a later version

If Windows reach matters, Route 2 (MSIX) fits Schedule Spotter best. It needs no certificate, the Store handles updates, and the 2 GB installer problem disappears. The cost is building and testing an MSIX package alongside the NSIS installer. Route 1 is quicker to set up, but it brings back the Windows signing Austin decided against, plus a monthly fee.
