#!/usr/bin/env python3
"""Run tftpd-hpa only while TalkOps supplies a recent volatile snapshot."""
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time


def lease_is_valid(root, now=None):
    now = time.time() if now is None else now
    try:
        lease = root / "active" / ".lease"
        if lease.is_symlink() or lease.stat().st_size > 64:
            return False
        expiry = float(lease.read_text(encoding="ascii"))
        return math.isfinite(expiry) and 0 < expiry - now <= 90
    except (OSError, ValueError, UnicodeError):
        return False


def stop(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except ProcessLookupError:
        pass
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)


def main():
    if os.geteuid() != 0:
        # in.tftpd drops to --user per request and exits silently without
        # CAP_SETGID, so a non-root start would accept but never answer.
        sys.exit("cisco-tftp must start as root with SETUID, SETGID and DAC_READ_SEARCH")
    root = Path("/run/talkops-tftp")
    done = threading.Event()
    for sig in [signal.SIGINT, signal.SIGTERM]:
        signal.signal(sig, lambda *_: done.set())
    process = None
    try:
        while not done.is_set():
            valid = lease_is_valid(root)
            if process is not None and (not valid or process.poll() is not None):
                stop(process)
                process = None
            if valid and process is None:
                process = subprocess.Popen(["/usr/sbin/in.tftpd", *sys.argv[1:]], start_new_session=True)
            done.wait(1)
    finally:
        if process is not None:
            stop(process)


if __name__ == "__main__":
    main()
