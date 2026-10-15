# Download

Schedule Spotter is downloaded from **https://tightline.systems/schedule**. Both that page and the file hosting at `get.tightline.systems/schedule/<version>/` belong to the Tight Line website, a separate project (`Tight_Line_Website`). The website is served by Caddy on Tight Line's workstation.

There is no email gate: visitors download directly, and the site's `/privacy` page promises no email is needed (Austin, 2026-10-14; DECISIONS.md, Phase 6). The site sets no cookies and keeps no visitor logs.

## Publishing a release

`site/publish.sh` hands one variant of a tested release run to the website's own publish script:

```sh
site/publish.sh --run <release run id> --variant <C|A|B-E2B|off> --dry-run   # check first
site/publish.sh --run <release run id> --variant <C|A|B-E2B|off>
```

The script:
- refuses a file that failed an install test on a gating runner, and a Mac build that isn't signed and notarized;
- checks each SHA-256 against the run's `releases.json`;
- gives the files the names clients see (`Schedule-Spotter-<version>-mac.dmg`, `Schedule-Spotter-<version>-windows-setup.exe`);
- runs the website's `scripts/release.mjs --tool schedule`.

That copies the files to `/srv/tightline/get/schedule/<version>/` and records sizes and SHA-256 in the website's `site/data/releases.json`. The website's own steps follow, and `release.mjs` prints them: deploy, check live, commit.

## Size

The page shows each file's size. Release candidate 0.1.0, Mac / Windows:

| Build | Size | At 25 Mbit/s |
| --- | --- | --- |
| Model off | 16 MB / 223 MB (Windows: Microsoft's WebView2 offline installer) | seconds / about 1 minute |
| C (Llama 3.2 1B) | 0.82 / 1.03 GB | about 5 minutes |
| A (Phi-4-mini) | 2.51 / 2.71 GB | about 14 minutes |
| B-E2B (Gemma 4 E2B) | 3.43 / 3.65 GB | about 19 minutes |

Downloads leave the office over its upload connection, and downloads running at the same time share it.
