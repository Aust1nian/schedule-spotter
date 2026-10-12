"""Schedule Spotter's download site, run on Tight Line's Raspberry Pi.

The flow (DECISIONS.md, 2026-10-11, "Download delivery"):
  1. The owner scans the QR code on the business card and lands on "/": what
     the tool does, and a form for a business email.
  2. POST /request emails a link that expires: /get/<token>.
  3. On the computer, /get/<token> shows one big download button for that
     computer (Mac or Windows), the other platform as a small link, the size,
     a rough download time and each file's SHA-256.
  4. The files come from nginx at /files/<token>/<name>; nginx asks
     /_auth (auth_request) whether the token is still good.

Python standard library only. Configuration comes from the environment
(deploy/site.env.example). No cookies, no scripts, no analytics. The email
address is kept only as long as its link works, unless the owner ticks the
optional follow-up box (shown only when CONSENT_TEXT is set).
"""
import base64
import hashlib
import hmac
import html
import json
import os
import re
import secrets
import smtplib
import sqlite3
import sys
import threading
import time
import urllib.parse
from email.message import EmailMessage
from email.utils import formatdate, make_msgid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATIC = HERE.parent / "static"
EMAIL_RE = re.compile(r"^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+$")
# Used only when COMPANY_DOMAINS_ONLY=1. Many small contractors run their
# business from one of these, so it is off by default.
PERSONAL_DOMAINS = {
    "gmail.com", "googlemail.com", "yahoo.com", "ymail.com", "hotmail.com", "outlook.com",
    "live.com", "msn.com", "icloud.com", "me.com", "mac.com", "aol.com", "proton.me",
    "protonmail.com", "gmx.com", "mail.com", "yandex.com", "comcast.net", "att.net",
    "verizon.net", "sbcglobal.net", "bellsouth.net", "cox.net", "charter.net",
}
DAY = 86400


class Config:
    def __init__(self, env=None):
        env = os.environ if env is None else env
        g = env.get
        self.site_url = g("SITE_URL", "https://TODO-Austin-domain.example").rstrip("/")
        self.secret = g("SECRET", "")
        if len(self.secret) < 32:
            raise SystemExit("SECRET must be set to at least 32 random characters (deploy/site.env.example)")
        self.data_dir = Path(g("DATA_DIR", "/var/lib/schedule-spotter-site"))
        self.releases_dir = Path(g("RELEASES_DIR", "/srv/schedule-spotter/releases"))
        self.link_days = int(g("LINK_DAYS", "7"))
        self.max_downloads = int(g("MAX_DOWNLOADS", "5"))
        self.links_per_email_per_day = int(g("LINKS_PER_EMAIL_PER_DAY", "3"))
        self.requests_per_ip_per_hour = int(g("REQUESTS_PER_IP_PER_HOUR", "10"))
        self.smtp_host = g("SMTP_HOST", "")
        self.smtp_port = int(g("SMTP_PORT", "587"))
        self.smtp_user = g("SMTP_USER", "")
        self.smtp_password = g("SMTP_PASSWORD", "")
        self.smtp_security = g("SMTP_SECURITY", "starttls")  # starttls | ssl | none
        self.mail_from = g("MAIL_FROM", "Tight Line <TODO-Austin@TODO-Austin-domain.example>")
        self.reply_to = g("REPLY_TO", "")
        self.mail_dir = g("MAIL_DIR", "")  # set: write .eml files instead of sending (testing)
        self.consent_text = g("CONSENT_TEXT", "")  # TODO(Austin): follow-up wording, if any
        self.company_domains_only = g("COMPANY_DOMAINS_ONLY", "0") == "1"
        self.assumed_mbps = float(g("ASSUMED_MBPS", "25"))
        self.contact = g("CONTACT", "")  # TODO(Austin): the one contact line
        self.bind = g("BIND", "127.0.0.1")
        self.port = int(g("PORT", "8088"))


# ---------------------------------------------------------------- tokens

def _b64(b):
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def make_token(secret, nonce, expires):
    sig = hmac.new(secret.encode(), f"{nonce}.{expires}".encode(), hashlib.sha256).digest()
    return f"{nonce}.{expires}.{_b64(sig[:18])}"


def read_token(secret, token, now):
    """(nonce, expires) for a well-formed, correctly signed, unexpired token."""
    parts = token.split(".")
    if len(parts) != 3 or not parts[1].isdigit():
        return None
    nonce, expires = parts[0], int(parts[1])
    if not hmac.compare_digest(make_token(secret, nonce, expires), token):
        return None
    if expires < now:
        return None
    return nonce, expires


# ---------------------------------------------------------------- storage

class Store:
    def __init__(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.lock = threading.Lock()
        with self.lock:
            self.db.executescript(
                """
                PRAGMA journal_mode=WAL;
                CREATE TABLE IF NOT EXISTS links (
                    nonce TEXT PRIMARY KEY, email TEXT NOT NULL, created INTEGER NOT NULL,
                    expires INTEGER NOT NULL, downloads INTEGER NOT NULL DEFAULT 0);
                CREATE INDEX IF NOT EXISTS links_email ON links(email, created);
                CREATE TABLE IF NOT EXISTS follow_up (
                    email TEXT PRIMARY KEY, agreed INTEGER NOT NULL, wording TEXT NOT NULL);
                """
            )

    def add_link(self, nonce, email, now, expires):
        with self.lock:
            self.db.execute("INSERT INTO links VALUES (?, ?, ?, ?, 0)", (nonce, email, now, expires))

    def links_since(self, email, since):
        with self.lock:
            return self.db.execute(
                "SELECT COUNT(*) FROM links WHERE email = ? AND created >= ?", (email, since)
            ).fetchone()[0]

    def link(self, nonce):
        with self.lock:
            return self.db.execute(
                "SELECT email, expires, downloads FROM links WHERE nonce = ?", (nonce,)
            ).fetchone()

    def count_download(self, nonce):
        with self.lock:
            self.db.execute("UPDATE links SET downloads = downloads + 1 WHERE nonce = ?", (nonce,))

    def agree(self, email, now, wording):
        with self.lock:
            self.db.execute(
                "INSERT INTO follow_up VALUES (?, ?, ?) ON CONFLICT(email) DO UPDATE SET agreed = ?, wording = ?",
                (email, now, wording, now, wording),
            )

    def purge(self, now):
        """Forgets links (and with them the email address) a day after they expire."""
        with self.lock:
            self.db.execute("DELETE FROM links WHERE expires < ?", (now - DAY,))


# ---------------------------------------------------------------- the site

class Site:
    def __init__(self, config, clock=time.time):
        self.c = config
        self.clock = clock
        self.store = Store(config.data_dir / "site.sqlite3")
        self.ip_hits = {}
        self.ip_lock = threading.Lock()

    # -- releases --------------------------------------------------------

    def releases(self):
        """What the site offers now: current.json, written by upload.sh."""
        try:
            data = json.loads((self.c.releases_dir / "current.json").read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None, {}
        files = {f["platform"]: f for f in data.get("files", []) if f.get("platform") in ("mac", "windows")}
        return data.get("version"), files

    # -- routing ---------------------------------------------------------

    def handle(self, method, path, headers, body, client_ip):
        """Returns (status, headers, body bytes)."""
        now = int(self.clock())
        url = urllib.parse.urlsplit(path)
        route = url.path
        if method == "GET" and route == "/":
            return self.page(200, "Schedule Spotter", self.landing(headers))
        if method == "POST" and route == "/request":
            return self.request_link(headers, body, client_ip, now)
        if method == "GET" and route.startswith("/get/"):
            return self.download_page(route[len("/get/"):], headers, now)
        if method == "GET" and route == "/_auth":
            return self.auth(headers, now)
        if method == "GET" and route == "/healthz":
            return 200, {"Content-Type": "text/plain"}, b"ok\n"
        if method == "GET" and route == "/static/style.css":
            # nginx serves this in production; here for local runs and tests.
            return 200, {"Content-Type": "text/css; charset=utf-8"}, (STATIC / "style.css").read_bytes()
        return self.page(404, "Not found", "<h1>Not found</h1><p><a href=\"/\">Schedule Spotter</a></p>")

    # -- pages -----------------------------------------------------------

    def page(self, status, title, body):
        contact = f'<p class="contact">{html.escape(self.c.contact)}</p>' if self.c.contact else ""
        doc = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<title>{html.escape(title)}</title>
<link rel="stylesheet" href="/static/style.css">
</head>
<body>
<main>
{body}
</main>
<footer><p>Schedule Spotter is a free tool from Tight Line.</p>{contact}</footer>
</body>
</html>
"""
        return status, {
            "Content-Type": "text/html; charset=utf-8",
            "Cache-Control": "no-store",
            "Content-Security-Policy": "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
            "Referrer-Policy": "no-referrer",
            "X-Content-Type-Options": "nosniff",
        }, doc.encode()

    def landing(self, headers, error="", email=""):
        consent = ""
        if self.c.consent_text:
            consent = (
                '<label class="check"><input type="checkbox" name="follow_up" value="1"> '
                f"{html.escape(self.c.consent_text)}</label>"
            )
        uses = "to send you the download link" + (", and as you choose below" if self.c.consent_text else "")
        err = f'<p class="error" role="alert">{html.escape(error)}</p>' if error else ""
        return f"""
<p class="from">A free tool from Tight Line</p>
<h1>Schedule Spotter</h1>
<p class="lede">Open the crew schedule you already keep, in Excel, Google Sheets, Numbers or a CSV.
Schedule Spotter shows who is double-booked, who is working on a day off, who has nothing booked,
and which jobs have nobody on them.</p>
<ul class="points">
<li>Works in five minutes, with no setup and no account.</li>
<li>Runs offline. Your schedule never leaves your computer.</li>
<li>For Mac and Windows computers.</li>
</ul>
<h2>Get it on your computer</h2>
<p>We'll email you the download link, so you can open it on the computer you'll use it on.</p>
{err}
<form method="post" action="/request">
<label for="email">Your business email</label>
<input id="email" name="email" type="email" autocomplete="email" inputmode="email" required value="{html.escape(email)}">
<div class="hp" aria-hidden="true"><label>Leave this empty <input name="website" tabindex="-1" autocomplete="off"></label></div>
{consent}
<button type="submit">Email me the link</button>
</form>
<p class="small">We use your email only {uses}. The link works for {self.c.link_days} days.</p>
"""

    def request_link(self, headers, body, client_ip, now):
        form = urllib.parse.parse_qs(body.decode("utf-8", "replace"), keep_blank_values=True)
        email = form.get("email", [""])[0].strip()
        if form.get("website", [""])[0]:
            # The hidden field only bots fill in: answer as if it worked.
            return self.page(200, "Check your email", self.sent_page("your inbox"))
        if not self.allow_ip(client_ip, now):
            return self.page(429, "Please wait", "<h1>Please wait a little</h1><p>There have been many requests from your network. Please try again in an hour.</p>")
        if len(email) > 254 or not EMAIL_RE.match(email):
            return self.page(400, "Schedule Spotter", self.landing(headers, "That doesn't look like an email address. Please check it.", email))
        domain = email.rsplit("@", 1)[1].lower()
        if self.c.company_domains_only and domain in PERSONAL_DOMAINS:
            return self.page(400, "Schedule Spotter", self.landing(headers, "Please use your business email address.", email))
        key = email.lower()
        if self.store.links_since(key, now - DAY) >= self.c.links_per_email_per_day:
            return self.page(429, "Check your email", "<h1>We've already sent you links today</h1><p>Please look for the email from Tight Line, including your spam folder. You can ask for another link tomorrow.</p>")
        nonce = secrets.token_urlsafe(12)
        expires = now + self.c.link_days * DAY
        token = make_token(self.c.secret, nonce, expires)
        self.store.add_link(nonce, key, now, expires)
        if self.c.consent_text and form.get("follow_up", [""])[0] == "1":
            self.store.agree(key, now, self.c.consent_text)
        try:
            self.send_link(email, f"{self.c.site_url}/get/{token}")
        except Exception as e:  # noqa: BLE001 - any mail failure gets the same answer
            print(f"mail failed: {type(e).__name__}: {e}", file=sys.stderr)
            return self.page(503, "Try again soon", "<h1>We couldn't send the email just now</h1><p>Please try again in a few minutes.</p>")
        self.store.purge(now)
        return self.page(200, "Check your email", self.sent_page(email))

    def sent_page(self, email):
        return f"""
<h1>Check your email</h1>
<p>We sent the download link to <strong>{html.escape(email)}</strong>.
Open that email on the Mac or Windows computer you'll use Schedule Spotter on.</p>
<p class="small">The link works for {self.c.link_days} days. Not there in a few minutes? Check your spam folder.</p>
"""

    def allow_ip(self, ip, now):
        with self.ip_lock:
            hits = [t for t in self.ip_hits.get(ip, []) if t > now - 3600]
            hits.append(now)
            self.ip_hits[ip] = hits
            if len(self.ip_hits) > 10000:  # forget old networks; memory only
                self.ip_hits = {k: v for k, v in self.ip_hits.items() if v[-1] > now - 3600}
            return len(hits) <= self.c.requests_per_ip_per_hour

    def valid_link(self, token, now):
        parsed = read_token(self.c.secret, token, now)
        if not parsed:
            return None
        row = self.store.link(parsed[0])
        if not row:
            return None
        return parsed[0], row

    def download_page(self, token, headers, now):
        link = self.valid_link(token, now)
        if not link:
            return self.page(410, "Link expired", """
<h1>This link has expired</h1>
<p>Download links work for a limited time. <a href="/">Ask for a new one</a>; it takes a minute.</p>""")
        _, (_, _, downloads) = link
        if downloads >= self.c.max_downloads:
            return self.page(410, "Link used up", """
<h1>This link has been used up</h1>
<p><a href="/">Ask for a new link</a> to download again.</p>""")
        version, files = self.releases()
        if not files:
            return self.page(503, "Back soon", "<h1>Downloads are being updated</h1><p>Please try again in a few minutes.</p>")
        ua = headers.get("User-Agent", "")
        if re.search(r"iPhone|iPad|iPod|Android|Mobile", ua):
            kind = "phone"
        elif "Windows" in ua:
            kind = "windows"
        elif re.search(r"Macintosh|Mac OS X", ua):
            kind = "mac"
        else:
            kind = "other"

        def href(f):
            return f"/files/{token}/{urllib.parse.quote(f['file'])}"

        def size(f):
            gb = f["bytes"] / 1e9
            return f"{gb:.1f} GB" if gb >= 1 else f"{f['bytes'] / 1e6:.0f} MB"

        def minutes(f):
            m = f["bytes"] * 8 / (self.c.assumed_mbps * 1e6) / 60
            return "under a minute" if m < 1 else f"about {round(m)} minute{'s' if round(m) != 1 else ''}"

        names = {"mac": "Mac", "windows": "Windows"}
        steps = {
            "mac": "<ol><li>Open the downloaded file.</li><li>Drag Schedule Spotter into Applications.</li>"
                   "<li>Open it from Applications. The first time, your Mac asks whether to open an app "
                   "downloaded from the internet: choose <strong>Open</strong>.</li></ol>",
            "windows": "<ol><li>Run the downloaded file. It installs for you alone, with no administrator password.</li>"
                       "<li>If Windows shows <strong>Windows protected your PC</strong>, choose <strong>More info</strong>, "
                       "then <strong>Run anyway</strong>.</li><li>Schedule Spotter opens from the Start menu.</li></ol>",
        }
        parts = [f"<h1>Download Schedule Spotter</h1>"]
        if kind in ("mac", "windows") and kind in files:
            f = files[kind]
            other = "windows" if kind == "mac" else "mac"
            unsigned = "" if f.get("signed") else '<p class="warn">Test build: not yet signed. For internal testing only.</p>'
            parts.append(f"""{unsigned}
<p><a class="button" href="{href(f)}" download>Download for {names[kind]}</a></p>
<p class="small">{size(f)}, {minutes(f)} on a typical {self.c.assumed_mbps:g} Mbit/s connection.</p>
{steps[kind]}""")
            if other in files:
                parts.append(f'<p class="small">Need it for {names[other]}? <a href="{href(files[other])}" download>Download for {names[other]}</a> ({size(files[other])}).</p>')
        else:
            if kind == "phone":
                parts.append("<p>Schedule Spotter runs on Mac and Windows computers. Open this email on your computer to download it.</p>")
            else:
                parts.append("<p>Schedule Spotter runs on Mac and Windows only.</p>")
            for p in ("mac", "windows"):
                if p in files:
                    parts.append(f'<p class="small"><a href="{href(files[p])}" download>Download for {names[p]}</a> ({size(files[p])})</p>')
        sums = "".join(
            f"<li>{html.escape(f['file'])}<br><code>{f['sha256']}</code></li>" for f in files.values()
        )
        parts.append(f"""<details><summary>Version {html.escape(str(version))} and file checksums (SHA-256)</summary><ul class="sums">{sums}</ul></details>
<p class="small">This link works {self.c.max_downloads - downloads} more time{'s' if self.c.max_downloads - downloads != 1 else ''} until it expires.</p>""")
        return self.page(200, "Download Schedule Spotter", "\n".join(parts))

    def auth(self, headers, now):
        """nginx auth_request: may this request have this file?"""
        uri = headers.get("X-Original-URI", "")
        m = re.match(r"^/files/([^/]+)/([^/?]+)$", uri)
        deny = (403, {"Content-Type": "text/plain"}, b"no\n")
        if not m:
            return deny
        token, name = m.group(1), urllib.parse.unquote(m.group(2))
        link = self.valid_link(token, now)
        if not link:
            return deny
        nonce, (_, _, downloads) = link
        _, files = self.releases()
        if name not in {f["file"] for f in files.values()}:
            return deny
        rng = headers.get("Range", "")
        fresh = not rng or re.match(r"^bytes=0-", rng)
        if fresh:
            # Resumed downloads (a Range past the start) don't use up the link.
            if downloads >= self.c.max_downloads:
                return deny
            self.store.count_download(nonce)
        return 200, {"Content-Type": "text/plain"}, b"ok\n"

    # -- email -----------------------------------------------------------

    def send_link(self, to, link):
        msg = EmailMessage()
        msg["From"] = self.c.mail_from
        msg["To"] = to
        if self.c.reply_to:
            msg["Reply-To"] = self.c.reply_to
        msg["Subject"] = "Your Schedule Spotter download link"
        msg["Date"] = formatdate(localtime=False)
        msg["Message-ID"] = make_msgid(domain=urllib.parse.urlsplit(self.c.site_url).hostname or None)
        text = f"""Here is your link to download Schedule Spotter, the free schedule checker from Tight Line:

{link}

Open it on the Mac or Windows computer you'll use Schedule Spotter on. The link works for {self.c.link_days} days.

Schedule Spotter runs offline: your schedule never leaves your computer.

You're getting this because someone asked for the link at {self.c.site_url} with this address. If that wasn't you, ignore this email; nothing else will be sent.
"""
        safe = html.escape(link, quote=True)
        rich = f"""<!doctype html><html><body style="font-family: system-ui, sans-serif; font-size: 16px; line-height: 1.5; color: #1b1d1f;">
<p>Here is your link to download <strong>Schedule Spotter</strong>, the free schedule checker from Tight Line.</p>
<p><a href="{safe}" style="display: inline-block; padding: 12px 20px; background: #0b5cad; color: #ffffff; border-radius: 8px; text-decoration: none; font-weight: 600;">Download Schedule Spotter</a></p>
<p>Open it on the Mac or Windows computer you'll use Schedule Spotter on. The link works for {self.c.link_days} days.</p>
<p>Schedule Spotter runs offline: your schedule never leaves your computer.</p>
<p style="color: #555b61; font-size: 14px;">You're getting this because someone asked for the link at {html.escape(self.c.site_url)} with this address. If that wasn't you, ignore this email; nothing else will be sent.</p>
</body></html>"""
        msg.set_content(text)
        msg.add_alternative(rich, subtype="html")
        if self.c.mail_dir:
            out = Path(self.c.mail_dir)
            out.mkdir(parents=True, exist_ok=True)
            (out / f"{int(self.clock() * 1000)}-{secrets.token_hex(4)}.eml").write_bytes(bytes(msg))
            return
        if not self.c.smtp_host:
            raise RuntimeError("SMTP_HOST is not set")
        if self.c.smtp_security == "ssl":
            server = smtplib.SMTP_SSL(self.c.smtp_host, self.c.smtp_port, timeout=20)
        else:
            server = smtplib.SMTP(self.c.smtp_host, self.c.smtp_port, timeout=20)
        with server:
            if self.c.smtp_security == "starttls":
                server.starttls()
            if self.c.smtp_user:
                server.login(self.c.smtp_user, self.c.smtp_password)
            server.send_message(msg)


# ---------------------------------------------------------------- HTTP

def serve(site):
    class Handler(BaseHTTPRequestHandler):
        server_version = "spotter-site"
        sys_version = ""

        def _go(self, method):
            n = min(int(self.headers.get("Content-Length") or 0), 4096)
            body = self.rfile.read(n) if n else b""
            # Behind nginx: the client's address comes from X-Real-IP.
            ip = self.headers.get("X-Real-IP") or self.client_address[0]
            status, headers, out = site.handle(method, self.path, self.headers, body, ip)
            self.send_response(status)
            for k, v in headers.items():
                self.send_header(k, v)
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            if method != "HEAD":
                self.wfile.write(out)

        def do_GET(self):
            self._go("GET")

        def do_POST(self):
            self._go("POST")

        def log_message(self, fmt, *args):
            # No addresses, no paths (tokens): just the status line's status.
            pass

    httpd = ThreadingHTTPServer((site.c.bind, site.c.port), Handler)
    print(f"spotter-site on {site.c.bind}:{site.c.port}", file=sys.stderr)
    httpd.serve_forever()


if __name__ == "__main__":
    serve(Site(Config()))
