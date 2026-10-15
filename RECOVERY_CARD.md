# Schedule Spotter: recovery card

For Tight Line salespeople helping an owner install Schedule Spotter. The PDF copy is `RECOVERY_CARD.pdf` (`python3 tools/release/recovery_card.py` makes it from this file).

## Mac

- **"Schedule Spotter is an app downloaded from the Internet. Are you sure you want to open it?"** This is normal and appears once. Choose **Open**.
- **"Apple could not verify Schedule Spotter is free of malware"** (a test build without Apple's signature). Choose **Done**. Open **System Settings → Privacy & Security**, scroll down to "Schedule Spotter was blocked", choose **Open Anyway** and enter the Mac's password. Do this once.
- **The app won't open from the disk image.** Drag Schedule Spotter into **Applications** first, then open it from there.
- **Needs macOS 11 (Big Sur) or later.** It runs on Apple Silicon and Intel Macs.

## Windows

- **The browser warns that the file "isn't commonly downloaded".** In Edge, choose **…** next to the download, then **Keep**, then **Show more**, then **Keep anyway**. In Chrome, choose **Keep**.
- **"Windows protected your PC"** (SmartScreen). Choose **More info**, check that the app is `Schedule-Spotter-…-setup.exe`, then choose **Run anyway**. The publisher shows as "Unknown publisher", because Windows builds are unsigned for now (Austin's decision). A signed build would show Austin Burnes.
- **No administrator password is needed.** If Windows asks for one, cancel: the owner has the wrong file.
- **Windows 10 (22H2) or Windows 11, 64-bit.** Snapdragon (Arm) laptops run it, but more slowly, and ask the owner more questions.

## A slow download

- The download is at **tightline.systems/schedule**, and the page shows its size. Sizes run from about 1 GB for the build with the smallest model to about 3.6 GB for the largest.
- Office Wi-Fi is often slow. A wired connection, or a time with fewer people online, helps.
- If it stops, choose **Resume** in the browser's downloads list, or start it again from the page.

## When the app misbehaves: the self-test

The app checks itself when started with `--self-test`. It opens its window, checks the built-in example, loads the model if this build has one, and prints `"self_test":"pass"` or `"fail"` with the reasons.

- **Mac:** open **Terminal** and paste:
  `"/Applications/Schedule Spotter.app/Contents/MacOS/schedule-spotter" --self-test`
- **Windows:** press **Windows+R** and paste:
  `"%LOCALAPPDATA%\Schedule Spotter\schedule-spotter.exe" --self-test --json-out "%USERPROFILE%\Desktop\spotter-self-test.txt"`
  The result appears on the desktop as `spotter-self-test.txt`.

What the result means:

- **`pass`.** The app is fine. Look at what the owner is doing instead: the file they opened, or what they pasted.
- **"model did not load"** or **"the model file is missing"**. The download was damaged. The app still works, but asks the owner more questions. Download it again and reinstall.
- **"page did not report back"** or **"no result"**, or a blank window. On Windows, reinstall: the installer also installs Microsoft's WebView2 runtime if it's missing. On a Mac, check that the macOS version is 11 or later.
- **Anything else.** Send the result to Austin at Tight Line. It contains no names or schedule data. In the app, **About → Save diagnostics…** writes a log with every name replaced by placeholders, and shows it before saving.

**Nothing leaves the owner's computer.** If an owner asks, have them turn Wi-Fi off: everything still works.
