# Download site

The page behind the QR code on the business card, served from Tight Line's Raspberry Pi (DECISIONS.md, 2026-10-11, "Download delivery"):

1. The owner scans the QR code on a phone. The page explains the tool and asks for a business email.
2. The Pi emails a link from the company address. The link works for `LINK_DAYS` days and allows `MAX_DOWNLOADS` downloads.
3. The owner opens the email on the computer. The page detects Mac or Windows and shows one big button, with the other platform as a small link. It also shows the size, a rough download time and each file's SHA-256. Phones and Linux get a short explanation instead.
4. nginx serves the file and supports resuming. Before each download it asks the Python service whether the link is still good.

| Path | What it is |
| --- | --- |
| `server/spotter_site.py` | The service: pages, the form, the email, link checks. Python standard library only. |
| `static/style.css` | The only other file a page loads. No scripts, no cookies, no fonts from elsewhere. |
| `deploy/` | nginx site and proxy snippet, systemd unit, settings template. |
| `upload.sh` | Publishes one variant of a release run to the Pi. |
| `tests/` | Unit tests, plus an end-to-end test of the real nginx config in Docker. Both run in CI. |

**Privacy.** The site keeps an email address only until a day after its link expires. The exception is an owner who ticks the optional follow-up box, which appears only when `CONSENT_TEXT` is set. nginx keeps no access log, and the service logs no addresses or links. The app itself never touches the network.

## Still needed from Austin

Each of these is marked `TODO(Austin)` in `deploy/`:

- The domain, used for `SITE_URL`, nginx `server_name` and certbot.
- The mail provider's SMTP settings, with SPF and DKIM on the domain so links don't land in spam.
- Whether Tight Line may follow up with the people who download, and the exact wording. This sets `CONSENT_TEXT`.
- How long a link works (`LINK_DAYS`, default 7).
- The contact line (`CONTACT`).
- The Pi's upload speed. See "Bandwidth" below.

## Setting up the Pi

These steps assume Raspberry Pi OS (Debian 12 or 13) on a wired connection, with ports 80 and 443 forwarded from the router to the Pi and the domain's DNS A record pointing at the office's public address.

```sh
sudo apt install nginx python3 certbot python3-certbot-nginx rsync
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin spotter-site
sudo mkdir -p /opt/schedule-spotter-site /srv/schedule-spotter/releases /etc/schedule-spotter-site
# From a checkout of this repo:
sudo rsync -a site/server site/static /opt/schedule-spotter-site/
sudo cp site/deploy/site.env.example /etc/schedule-spotter-site/site.env
sudo chmod 600 /etc/schedule-spotter-site/site.env      # then fill in every TODO(Austin)
sudo cp site/deploy/schedule-spotter-site.service /etc/systemd/system/
sudo systemctl enable --now schedule-spotter-site
sudo cp site/deploy/schedule-spotter-proxy.conf /etc/nginx/snippets/
sudo cp site/deploy/nginx.conf /etc/nginx/sites-available/schedule-spotter  # set the domain
sudo ln -s /etc/nginx/sites-available/schedule-spotter /etc/nginx/sites-enabled/
sudo certbot certonly --webroot -w /var/www/html -d <domain>
sudo nginx -t && sudo systemctl reload nginx
sudo chown "$USER" /srv/schedule-spotter/releases           # so upload.sh can write
```

**Trying it without sending mail.** Set `MAIL_DIR=/var/lib/schedule-spotter-site/mail` in `site.env`. Each email is then written there as an `.eml` file instead of being sent. Remove the line to go live.

**The QR code** is the plain site address, so the card never goes out of date: `uvx segno "https://<domain>/" --scale 10 -o card-qr.png`.

## Publishing a release

`RELEASE.md` covers the whole release. The last step is:

```sh
site/upload.sh --run <release run id> --variant <C|A|B-E2B|off> --host pi@<pi address> --prune
```

The script refuses a file that failed an install test or one that isn't signed. `--allow-unsigned` overrides the signing check for an internal test site.

## Bandwidth

The Pi's upload speed sets how long each download takes, and downloads running at the same time share it. Rough figures at 20 Mbit/s upload:

| Build | Size | One download |
| --- | --- | --- |
| Model off | about 15 MB (Windows) to 30 MB (Mac) | seconds |
| C (Llama 3.2 1B) | about 1 GB | about 7 minutes |
| A (Phi-4-mini) | about 2.7 GB | about 18 minutes |
| B-E2B (Gemma 4 E2B) | about 3.6 GB | about 24 minutes |

If many owners download at once, or the office upload is slower, the multi-GB builds may need a host with more bandwidth. That would be Cloudflare R2, as the spec first planned. Only `upload.sh` and the `alias` in the nginx config would change.
