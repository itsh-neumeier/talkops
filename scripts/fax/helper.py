#!/usr/bin/env python3
"""Bounded format and IMAPS adapters. Business policy remains in Rust."""
import base64
import email.policy
import hashlib
import imaplib
import io
import json
import os
from pathlib import Path
import re
import ssl
import subprocess
import sys
import tempfile
from email.parser import BytesParser

RAW_LIMIT = 30 * 1024 * 1024
INPUT_LIMIT = 20 * 1024 * 1024
PAGE_LIMIT = 50
PIXEL_LIMIT = 40_000_000
WIDTH, HEIGHT = 1728, 2292


def parse_mail(raw):
    if len(raw) > RAW_LIMIT:
        raise ValueError("Message exceeds 30 MiB.")
    msg = BytesParser(policy=email.policy.default).parsebytes(raw)
    senders = msg.get_all("From", [])
    try:
        addresses = senders[0].addresses if len(senders) == 1 and not senders[0].defects else []
    except (ValueError, AttributeError):
        addresses = []
    sender = addresses[0].addr_spec if len(addresses) == 1 else ""
    subject = str(msg.get("Subject", "")).strip() if len(msg.get_all("Subject", [])) == 1 else ""
    auto = str(msg.get("Auto-Submitted", "no")).lower() != "no"
    auto = auto or msg.get_content_type() == "multipart/report"
    auto = auto or str(msg.get("Return-Path", "")).strip() == "<>"
    attachments, total = [], 0
    error = ""
    def file_parts(part, depth=0):
        if depth > 30:
            raise ValueError("MIME nesting limit exceeded.")
        if part.get_content_disposition() == "attachment":
            yield part
        elif part.is_multipart() and part.get_content_disposition() != "inline":
            for child in part.iter_parts():
                yield from file_parts(child, depth + 1)
    for part in file_parts(msg):
        if part.is_multipart():
            error = "Attached messages and nested MIME documents are not supported."
            break
        if part.get_content_disposition() != "attachment":
            continue
        data = part.get_payload(decode=True) or b""
        total += len(data)
        if total > INPUT_LIMIT or len(attachments) >= 10:
            error = "Attach at most 10 files and 20 MiB."
            break
        attachments.append({"name": str(part.get_filename() or "attachment")[:200],
                            "data": base64.b64encode(data).decode("ascii")})
    if not attachments and not error:
        error = "Attach at least one PDF or image file. Mail body content is not faxed."
    return {"sender": sender, "subject": subject, "automatic": auto,
            "files": attachments, "error": error,
            "fingerprint": hashlib.sha256(raw).hexdigest()}


def mailbox(config):
    # Always verify the certificate and hostname. There is no insecure mode.
    with imaplib.IMAP4_SSL(config["host"], config["port"],
                          ssl_context=ssl.create_default_context(), timeout=20) as conn:
        conn.login(config["username"], config["password"])
        status, _ = conn.select('"' + config["folder"].replace('\\', '\\\\').replace('"', '\\"') + '"')
        if status != "OK":
            raise ValueError("Cannot select fax mailbox.")
        validity = (conn.response("UIDVALIDITY")[1] or [b""])[0].decode("ascii")
        if config.get("ack"):
            if config["validity"] != validity:
                raise ValueError("Mailbox UIDVALIDITY changed before acknowledgement.")
            conn.uid("store", str(int(config["ack"])), "+FLAGS.SILENT", "(\\Seen)")
            return {"acknowledged": True}
        status, data = conn.uid("search", None, "UNSEEN")
        if status != "OK" or not data[0]:
            return None
        uid = data[0].split()[0].decode("ascii")
        status, size = conn.uid("fetch", uid, "(RFC822.SIZE)")
        size_text = b" ".join(p for p in size if isinstance(p, bytes))
        match = re.search(rb"RFC822.SIZE (\d+)", size_text)
        if not match or int(match[1]) > RAW_LIMIT:
            # Do not repeatedly fetch an oversized message. Record an input error.
            return {"uid": uid, "validity": validity, "sender": "", "subject": "",
                    "automatic": True, "files": [], "error": "Message exceeds 30 MiB.",
                    "fingerprint": hashlib.sha256((validity + ":" + uid).encode()).hexdigest()}
        status, data = conn.uid("fetch", uid, "(BODY.PEEK[])")
        chunks = [p[1] for p in data if isinstance(p, tuple)]
        if status != "OK" or len(chunks) != 1:
            raise ValueError("Cannot read fax mail.")
        try:
            result = parse_mail(chunks[0])
        except (ValueError, RecursionError):
            result = {"sender": "", "subject": "", "automatic": True, "files": [],
                      "error": "Invalid MIME message.", "fingerprint": hashlib.sha256(chunks[0]).hexdigest()}
        result.update(uid=uid, validity=validity)
        return result


def limit_resources():
    try:
        import resource
        resource.setrlimit(resource.RLIMIT_AS, (768 * 1024**2, 768 * 1024**2))
        resource.setrlimit(resource.RLIMIT_CPU, (90, 90))
        resource.setrlimit(resource.RLIMIT_FSIZE, (100 * 1024**2, 100 * 1024**2))
    except ImportError:
        pass


def convert(config):
    from PIL import Image, ImageDraw, ImageFont, ImageOps, ImageSequence
    from reportlab.pdfgen import canvas
    from reportlab.lib.utils import ImageReader
    import warnings
    warnings.simplefilter("error", Image.DecompressionBombWarning)
    Image.MAX_IMAGE_PIXELS = PIXEL_LIMIT
    limit_resources()
    root = Path(config["directory"]).resolve()
    root.mkdir(parents=True, exist_ok=True)
    frames = []

    def add(image):
        if len(frames) >= PAGE_LIMIT:
            raise ValueError("Fax exceeds 50 pages.")
        if image.width * image.height > PIXEL_LIMIT or max(image.size) > 20000:
            raise ValueError("Image exceeds pixel limits.")
        image.load()
        normalized = ImageOps.exif_transpose(image)
        normalized.thumbnail((WIDTH, HEIGHT))
        if normalized.mode in ("RGBA", "LA") or "transparency" in normalized.info:
            rgba = normalized.convert("RGBA")
            normalized = Image.new("RGBA", rgba.size, "white")
            normalized.alpha_composite(rgba)
        frames.append(normalized.convert("L").copy())

    with tempfile.TemporaryDirectory(dir=root) as work:
        total = 0
        for file in config["files"]:
            path = (root / file).resolve()
            if path.parent != root or not path.is_file():
                raise ValueError("Invalid fax file path.")
            total += path.stat().st_size
            if total > (100 * 1024**2 if config.get("receive") else INPUT_LIMIT):
                raise ValueError("Fax input exceeds size limit.")
            with path.open("rb") as stream:
                magic = stream.read(8)
            if magic.startswith(b"%PDF-"):
                info = subprocess.run(["pdfinfo", str(path)], capture_output=True, timeout=10, check=True).stdout
                if re.search(rb"Encrypted:\s+yes", info):
                    raise ValueError("Encrypted PDFs are not supported.")
                pages = re.search(rb"Pages:\s+(\d+)", info)
                if not pages or len(frames) + int(pages[1]) > PAGE_LIMIT:
                    raise ValueError("Fax exceeds 50 pages.")
                for number in range(1, int(pages[1]) + 1):
                    out = str(Path(work) / "page")
                    subprocess.run(["pdftoppm", "-f", str(number), "-l", str(number),
                                    "-singlefile", "-scale-to-x", str(WIDTH), "-scale-to-y", str(HEIGHT),
                                    "-png", str(path), out], capture_output=True, check=True, timeout=20)
                    with Image.open(out + ".png") as image:
                        add(image)
            else:
                with Image.open(path) as image:
                    if image.format not in ("JPEG", "PNG", "TIFF", "BMP", "WEBP"):
                        raise ValueError("Attach PDF, JPEG, PNG, TIFF, BMP or WebP files.")
                    if getattr(image, "n_frames", 1) > 1 and image.format != "TIFF":
                        raise ValueError("Animated images are not supported.")
                    for frame in ImageSequence.Iterator(image):
                        add(frame)
        if not frames:
            raise ValueError("No fax pages.")
        try:
            font = ImageFont.truetype(config.get("font", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"), 23)
        except OSError:
            font = ImageFont.load_default(size=23)
        pages = []
        for i, frame in enumerate(frames):
            receive = config.get("receive", False)
            page = Image.new("RGB", (WIDTH, HEIGHT), "white")
            margin = 0 if receive else 65
            fitted = ImageOps.contain(frame, (WIDTH - 40, HEIGHT - margin * 2))
            page.paste(fitted, ((WIDTH - fitted.width) // 2, margin + (HEIGHT - margin * 2 - fitted.height) // 2))
            if not receive:
                draw = ImageDraw.Draw(page)
                for key, y in (("header", 18), ("footer", HEIGHT - 45)):
                    text = config.get(key, "")
                    for token, value in {"sender": config.get("sender", ""), "recipient": config.get("recipient", ""),
                                         "date": config.get("date", ""), "page": str(i + 1), "pages": str(len(frames))}.items():
                        if key != "header" or not config.get("literal_header"):
                            text = text.replace("{" + token + "}", value)
                    # Shrink bounded text to fit; never crop a long sender address.
                    line_font = font
                    if draw.textlength(text, font=line_font) > WIDTH - 40:
                        size = max(8, int(23 * (WIDTH - 40) / draw.textlength(text, font=line_font)))
                        line_font = ImageFont.truetype(font.path, size) if hasattr(font, "path") else font
                    draw.text((20, y), text, font=line_font, fill="black")
            pages.append(page.convert("1"))
        tiff = root / "document.tif.tmp"
        pages[0].save(tiff, format="TIFF", save_all=True, append_images=pages[1:], compression="group4", dpi=(204,196))
        pdf = root / "document.pdf.tmp"
        doc = canvas.Canvas(str(pdf), pagesize=(595.276, 841.89))
        for page in pages:
            doc.drawImage(ImageReader(page.convert("RGB")), 0, 0, width=595.276, height=841.89)
            doc.showPage()
        doc.save()
        os.replace(tiff, root / "document.tif")
        os.replace(pdf, root / "document.pdf")
    return {"pages": len(pages)}


if __name__ == "__main__":
    try:
        config = json.load(sys.stdin)
        result = mailbox(config) if config["mode"] == "mail" else convert(config)
        print(json.dumps(result))
    except Exception:
        # Never echo credentials, message content or raw parser diagnostics.
        print(json.dumps({"error": "Fax adapter failed. Check the mailbox connection or document format and limits."}))
        sys.exit(1)
