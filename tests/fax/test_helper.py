"""Mail/body isolation and real document conversion fixtures."""
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch
from email.message import EmailMessage

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/fax/helper.py"
spec = importlib.util.spec_from_file_location("fax_helper", SCRIPT)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


class MailTests(unittest.TestCase):
    def message(self):
        msg = EmailMessage()
        msg["From"] = "Sender <Allowed@Example.com>"
        msg["To"] = "faxin@example.com"
        msg["Subject"] = "+49123456"
        msg.set_content("PRIVATE MAIL BODY MUST NEVER APPEAR ON FAX")
        msg.add_alternative("<p>PRIVATE HTML MUST NEVER APPEAR</p>", subtype="html")
        return msg

    def test_only_attachments_in_order(self):
        msg = self.message()
        msg.add_attachment(b"%PDF-test", maintype="application", subtype="pdf", filename="one.pdf")
        msg.add_attachment(b"image", maintype="image", subtype="png", filename="two.png")
        parsed = helper.parse_mail(msg.as_bytes())
        self.assertEqual(parsed["sender"], "Allowed@Example.com")
        self.assertEqual(parsed["subject"], "+49123456")
        self.assertEqual([f["name"] for f in parsed["files"]], ["one.pdf", "two.png"])
        self.assertNotIn("PRIVATE", json.dumps(parsed))

    def test_inline_signature_and_body_are_not_files(self):
        msg = self.message()
        msg.get_payload()[1].add_related(b"signature", maintype="image", subtype="png", cid="<sig>", disposition="inline")
        parsed = helper.parse_mail(msg.as_bytes())
        self.assertEqual(parsed["files"], [])
        self.assertIn("Attach", parsed["error"])

    def test_ambiguous_and_automatic_mail(self):
        msg = self.message()
        msg.replace_header("From", "a@example.com, b@example.com")
        msg["Auto-Submitted"] = "auto-replied"
        parsed = helper.parse_mail(msg.as_bytes())
        self.assertEqual(parsed["sender"], "")
        self.assertTrue(parsed["automatic"])

    def test_attachment_limit_rejects_entire_submission(self):
        msg = self.message()
        for i in range(11):
            msg.add_attachment(b"file", maintype="image", subtype="png", filename=str(i))
        self.assertIn("10 files", helper.parse_mail(msg.as_bytes())["error"])


class MailboxTests(unittest.TestCase):
    def setUp(self):
        self.config = {"host": "mail.example.com", "port": 993, "username": "fax",
                       "password": "fixture", "folder": "INBOX"}
        self.conn = MagicMock()
        self.conn.__enter__.return_value = self.conn
        self.conn.select.return_value = ("OK", [b"1"])
        self.conn.response.return_value = ("UIDVALIDITY", [b"42"])

    def test_peek_does_not_ack_and_tls_is_verified(self):
        raw = b"From: allowed@example.com\r\nSubject: +49123456\r\n\r\nprivate body"
        self.conn.uid.side_effect = [("OK", [b"7"]), ("OK", [b"7 (RFC822.SIZE 70)"]),
                                     ("OK", [(b"7", raw)])]
        with patch.object(helper.imaplib, "IMAP4_SSL", return_value=self.conn) as connect:
            result = helper.mailbox(self.config)
            context = connect.call_args.kwargs["ssl_context"]
            self.assertTrue(context.check_hostname)
            self.assertEqual(context.verify_mode, helper.ssl.CERT_REQUIRED)
        self.assertEqual(result["uid"], "7")
        self.assertEqual(result["validity"], "42")
        self.assertEqual(self.conn.uid.call_args.args, ("fetch", "7", "(BODY.PEEK[])"))
        self.assertNotIn("private body", json.dumps(result))

    def test_ack_checks_validity_and_marks_seen(self):
        config = dict(self.config, ack="7", validity="41")
        with patch.object(helper.imaplib, "IMAP4_SSL", return_value=self.conn):
            with self.assertRaises(ValueError):
                helper.mailbox(config)
            self.conn.uid.assert_not_called()
            config["validity"] = "42"
            self.assertTrue(helper.mailbox(config)["acknowledged"])
        self.conn.uid.assert_called_once_with("store", "7", "+FLAGS.SILENT", "(\\Seen)")

    def test_oversized_message_is_not_downloaded(self):
        self.conn.uid.side_effect = [("OK", [b"8"]), ("OK", [b"8 (RFC822.SIZE 999999999)"])]
        with patch.object(helper.imaplib, "IMAP4_SSL", return_value=self.conn):
            result = helper.mailbox(self.config)
        self.assertTrue(result["automatic"])
        self.assertIn("30 MiB", result["error"])
        self.assertEqual(self.conn.uid.call_count, 2)


class ConversionTests(unittest.TestCase):
    def run_adapter(self, config):
        return subprocess.run([sys.executable, str(SCRIPT)], input=json.dumps(config),
                              text=True, capture_output=True, timeout=130)

    def test_images_pdf_and_preview_use_same_page_order(self):
        from PIL import Image
        from reportlab.pdfgen import canvas
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, color, fmt in [("one", "red", "PNG"), ("two", "blue", "JPEG")]:
                Image.new("RGB", (90, 150), color).save(root / name, format=fmt)
            config = {"mode": "convert", "directory": directory, "files": ["one", "two"],
                      "header": "allowed@example.com", "footer": "Footer {page}/{pages}", "sender": "+49123456"}
            result = self.run_adapter(config)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(json.loads(result.stdout)["pages"], 2)
            with Image.open(root / "document.tif") as fax:
                self.assertEqual(fax.n_frames, 2)
                self.assertEqual(fax.mode, "1")
                self.assertEqual(fax.size, (1728, 2292))
                self.assertEqual(fax.tag_v2[259], 4)  # CCITT Group 4
                self.assertEqual(tuple(map(round, fax.info["dpi"])), (204, 196))
            self.assertTrue((root / "document.pdf").read_bytes().startswith(b"%PDF-"))
            if shutil.which("pdftoppm"):
                pdf = canvas.Canvas(str(root / "three")); pdf.drawString(20, 700, "Third attachment"); pdf.showPage(); pdf.save()
                config["files"].append("three")
                result = self.run_adapter(config)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(json.loads(result.stdout)["pages"], 3)

    def test_invalid_attachment_does_not_produce_partial_fax(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Image.new("RGB", (50, 50), "white").save(root / "good", format="PNG")
            (root / "bad").write_bytes(b"not an image")
            result = self.run_adapter({"mode": "convert", "directory": directory, "files": ["good", "bad"]})
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "document.tif").exists())

    def test_multipage_tiff_and_animated_image_rejection(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            one, two = Image.new("RGB", (60, 80), "white"), Image.new("RGB", (80, 60), "black")
            one.save(root / "pages", format="TIFF", save_all=True, append_images=[two])
            result = self.run_adapter({"mode": "convert", "directory": directory, "files": ["pages"]})
            self.assertEqual(json.loads(result.stdout)["pages"], 2)
            one.save(root / "animated", format="PNG", save_all=True, append_images=[two])
            result = self.run_adapter({"mode": "convert", "directory": directory, "files": ["animated"]})
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
