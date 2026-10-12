"""Tests for the download site's request handling (no network, no SMTP).

Run: python3 -m unittest discover -s site/tests
"""
import email
import email.policy
import json
import pathlib
import sys
import tempfile
import unittest
import urllib.parse

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "server"))
import spotter_site as s  # noqa: E402

MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"
WIN = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0"
PHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"
LINUX = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0"


class Clock:
    def __init__(self):
        self.now = 1_791_800_000.0

    def __call__(self):
        return self.now


class SiteTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(self.tmp.name)
        (root / "releases").mkdir()
        (root / "releases" / "current.json").write_text(json.dumps({
            "version": "0.1.0",
            "files": [
                {"file": "Schedule-Spotter-0.1.0-C-mac.dmg", "platform": "mac", "bytes": 1_050_000_000, "sha256": "a" * 64, "signed": True},
                {"file": "Schedule-Spotter-0.1.0-C-windows-UNSIGNED-setup.exe", "platform": "windows", "bytes": 1_010_000_000, "sha256": "b" * 64, "signed": False},
            ],
        }))
        self.env = {
            "SECRET": "x" * 40,
            "SITE_URL": "https://spotter.example",
            "DATA_DIR": str(root / "data"),
            "RELEASES_DIR": str(root / "releases"),
            "MAIL_DIR": str(root / "mail"),
        }
        self.mail = root / "mail"
        self.clock = Clock()
        self.site = s.Site(s.Config(self.env), clock=self.clock)

    def tearDown(self):
        self.site.store.db.close()
        self.tmp.cleanup()

    def go(self, method, path, body=b"", ua=MAC, ip="203.0.113.5", **headers):
        h = {"User-Agent": ua, **headers}
        status, hdrs, out = self.site.handle(method, path, h, body, ip)
        return status, hdrs, out.decode()

    def ask(self, address="owner@acme-roofing.com", ip="203.0.113.5", extra=""):
        body = urllib.parse.urlencode({"email": address, "website": ""}).encode() + extra.encode()
        return self.go("POST", "/request", body, ip=ip)

    def last_link(self):
        msgs = sorted(self.mail.glob("*.eml"))
        msg = email.message_from_bytes(msgs[-1].read_bytes(), policy=email.policy.default)
        text = msg.get_body(preferencelist=("plain",)).get_content()
        link = next(line for line in text.splitlines() if line.startswith("https://"))
        return msg, urllib.parse.urlsplit(link).path

    def test_landing_explains_and_asks_for_a_business_email(self):
        status, headers, page = self.go("GET", "/", ua=PHONE)
        self.assertEqual(status, 200)
        self.assertIn("Your business email", page)
        self.assertIn("never leaves your computer", page)
        self.assertIn("We use your email only to send you the download link", page)
        self.assertNotIn("<script", page)
        self.assertNotIn("Set-Cookie", headers)
        self.assertIn("default-src 'none'", headers["Content-Security-Policy"])

    def test_email_carries_a_working_link(self):
        status, _, page = self.ask()
        self.assertEqual(status, 200)
        self.assertIn("Check your email", page)
        msg, path = self.last_link()
        self.assertEqual(msg["To"], "owner@acme-roofing.com")
        self.assertIn("Schedule Spotter", msg["Subject"])
        self.assertTrue(path.startswith("/get/"))
        status, _, page = self.go("GET", path, ua=MAC)
        self.assertEqual(status, 200)
        self.assertIn("Download for Mac", page)
        self.assertIn("1.1 GB", page)
        self.assertIn("Need it for Windows?", page)
        self.assertIn("a" * 64, page)  # SHA-256 listed

    def test_page_fits_the_computer(self):
        self.ask()
        _, path = self.last_link()
        _, _, win = self.go("GET", path, ua=WIN)
        self.assertIn("Download for Windows", win)
        self.assertIn("More info", win)
        self.assertIn("Test build: not yet signed", win)
        _, _, phone = self.go("GET", path, ua=PHONE)
        self.assertIn("Open this email on your computer", phone)
        _, _, linux = self.go("GET", path, ua=LINUX)
        self.assertIn("Mac and Windows only", linux)

    def test_link_expires(self):
        self.ask()
        _, path = self.last_link()
        self.clock.now += 7 * s.DAY + 1
        status, _, page = self.go("GET", path)
        self.assertEqual(status, 410)
        self.assertIn("expired", page)

    def test_tampered_link_is_refused(self):
        self.ask()
        _, path = self.last_link()
        nonce, expires, sig = path[len("/get/"):].split(".")
        longer = f"/get/{nonce}.{int(expires) + 30 * s.DAY}.{sig}"
        self.assertEqual(self.go("GET", longer)[0], 410)

    def test_nginx_auth_counts_fresh_downloads_only(self):
        self.ask()
        _, path = self.last_link()
        token = path[len("/get/"):]
        uri = f"/files/{token}/Schedule-Spotter-0.1.0-C-mac.dmg"
        for _ in range(5):
            self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": uri})[0], 200)
        # Resuming a download doesn't use the link up...
        self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": uri, "Range": "bytes=5000-"})[0], 200)
        # ...but a sixth fresh one does.
        self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": uri})[0], 403)
        self.assertIn("used up", self.go("GET", path)[2])

    def test_nginx_auth_refuses_other_files_and_bad_tokens(self):
        self.ask()
        _, path = self.last_link()
        token = path[len("/get/"):]
        self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": f"/files/{token}/current.json"})[0], 403)
        self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": f"/files/{token}/..%2Fsite.sqlite3"})[0], 403)
        self.assertEqual(self.go("GET", "/_auth", **{"X-Original-URI": "/files/nope/Schedule-Spotter-0.1.0-C-mac.dmg"})[0], 403)
        self.assertEqual(self.go("GET", "/_auth")[0], 403)

    def test_bad_address_is_explained(self):
        status, _, page = self.ask("not an email")
        self.assertEqual(status, 400)
        self.assertIn("look like an email address", page)
        self.assertFalse(self.mail.exists() and list(self.mail.glob("*.eml")))

    def test_personal_domains_only_when_asked(self):
        self.assertEqual(self.ask("owner@gmail.com")[0], 200)
        self.site.c.company_domains_only = True
        status, _, page = self.ask("other@gmail.com")
        self.assertEqual(status, 400)
        self.assertIn("business email", page)

    def test_rate_limits(self):
        for _ in range(3):
            self.assertEqual(self.ask(ip="198.51.100.1")[0], 200)
        self.assertEqual(self.ask(ip="198.51.100.1")[0], 429)  # 3 links a day per address
        self.clock.now += s.DAY + 1
        self.assertEqual(self.ask(ip="198.51.100.2")[0], 200)
        for i in range(10):
            self.ask(f"p{i}@acme-roofing.com", ip="198.51.100.9")
        self.assertEqual(self.ask("late@acme-roofing.com", ip="198.51.100.9")[0], 429)  # 10 an hour per network

    def test_honeypot_sends_nothing(self):
        body = urllib.parse.urlencode({"email": "bot@spam.example", "website": "http://spam"}).encode()
        self.assertEqual(self.go("POST", "/request", body)[0], 200)
        self.assertFalse(self.mail.exists() and list(self.mail.glob("*.eml")))

    def test_addresses_are_forgotten_after_the_link_expires(self):
        self.ask()
        self.clock.now += 9 * s.DAY
        self.ask("someone@else-co.com")
        rows = self.site.store.db.execute("SELECT email FROM links").fetchall()
        self.assertEqual(rows, [("someone@else-co.com",)])

    def test_follow_up_box_only_with_consent_wording(self):
        self.assertNotIn('name="follow_up"', self.go("GET", "/")[2])
        self.site.c.consent_text = "Tight Line may email me about this tool."
        self.assertIn('name="follow_up"', self.go("GET", "/")[2])
        self.ask(extra="&follow_up=1")
        rows = self.site.store.db.execute("SELECT email, wording FROM follow_up").fetchall()
        self.assertEqual(rows, [("owner@acme-roofing.com", "Tight Line may email me about this tool.")])

    def test_no_releases_yet(self):
        (pathlib.Path(self.env["RELEASES_DIR"]) / "current.json").unlink()
        self.ask()
        _, path = self.last_link()
        self.assertEqual(self.go("GET", path)[0], 503)

    def test_secret_is_required(self):
        with self.assertRaises(SystemExit):
            s.Config({**self.env, "SECRET": "short"})


if __name__ == "__main__":
    unittest.main()
