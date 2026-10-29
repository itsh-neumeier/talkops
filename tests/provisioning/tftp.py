"""Exercise the shipped tftpd-hpa policy over UDP, started as root like the sidecar."""
import os
import importlib.util
import pwd
import shutil
import socket
import struct
import subprocess
import tempfile
import time
import unittest
from pathlib import Path


class CiscoTftpPolicy(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = shutil.which("in.tftpd") or "/usr/sbin/in.tftpd"
        if not Path(cls.binary).exists():
            raise RuntimeError("Install tftpd-hpa before running this protocol test")
        if os.getuid() != 0:
            # in.tftpd needs CAP_SETGID to drop to --user for each request.
            raise RuntimeError("Run this protocol test as root, like the sidecar")
        cls.temp = tempfile.TemporaryDirectory(prefix="talkops-tftp-test-")
        cls.root = Path(cls.temp.name)
        cls.user = pwd.getpwnam("nobody")
        cls.files = {
            "127.0.0.2/SEP805EC0010203.cnf.xml": b"<device>first-private-secret</device>" * 40,
            "127.0.0.2/dialplan.xml": b"<DIALTEMPLATE/>",
            "127.0.0.3/SEP805EC0040506.cnf.xml": b"<device>second-private-secret</device>",
            "127.0.0.2/Ringlist-wb.xml": b"<CiscoIPPhoneRingList/>",
            "127.0.0.2/talkops-0123abcd.rwb": b"\x01\x00" * 600,
            "127.0.0.3/talkops-89abcdef.rwb": b"\x02\x00" * 300,
        }
        for name, data in cls.files.items():
            target = cls.root / "active" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            target.chmod(0o600)
        mapping = Path(__file__).resolve().parents[2] / "docker/talkops/cisco-tftp.map"
        policy = cls.root / "policy.map"
        policy.write_text(mapping.read_text().replace("/run/talkops-tftp", str(cls.root)))
        for path in [cls.root, *cls.root.rglob("*")]:
            if path.is_dir():
                path.chmod(0o700)
            os.chown(path, cls.user.pw_uid, cls.user.pw_gid)
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
            probe.bind(("127.0.0.1", 0))
            cls.port = probe.getsockname()[1]
        cls.server = subprocess.Popen([
            cls.binary, "--foreground", "--ipv4", f"--address=127.0.0.1:{cls.port}",
            f"--user={cls.user.pw_name}", "--permissive", f"--map-file={policy}", str(cls.root)
        ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        # Wait for an actual TFTP response; process startup is not socket readiness.
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as ready:
            ready.bind(("127.0.0.2", 0))
            ready.settimeout(0.2)
            for _ in range(50):
                ready.sendto(b"\x00\x01__ready__\x00octet\x00", ("127.0.0.1", cls.port))
                try:
                    ready.recvfrom(1024)
                    break
                except socket.timeout:
                    if cls.server.poll() is not None:
                        raise RuntimeError(cls.server.stderr.read().decode())
            else:
                raise RuntimeError("TFTP listener did not become ready")
        if cls.server.poll() is not None:
            raise RuntimeError(cls.server.stderr.read().decode())

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        cls.server.communicate(timeout=5)
        cls.temp.cleanup()

    def request(self, source, filename, opcode=1):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
            client.bind((source, 0))
            client.settimeout(3)
            client.sendto(struct.pack("!H", opcode) + filename.encode() + b"\0octet\0", ("127.0.0.1", self.port))
            output = b""
            while True:
                packet, peer = client.recvfrom(4096)
                code, block = struct.unpack("!HH", packet[:4])
                if os.environ.get("TALKOPS_TFTP_TEST_DEBUG"): print(filename, code, block, len(packet), peer, flush=True)
                if code == 5:
                    return code, packet[4:]
                self.assertEqual(code, 3)
                output += packet[4:]
                client.sendto(struct.pack("!HH", 4, block), peer)
                if len(packet) < 516:
                    return code, output

    def test_exact_private_file_multiblock_transfer_and_declared_dialplan(self):
        for filename in ["SEP805EC0010203.cnf.xml", "dialplan.xml"]:
            code, body = self.request("127.0.0.2", filename)
            self.assertEqual(code, 3)
            self.assertEqual(body, self.files["127.0.0.2/" + filename])

    def test_ring_list_ringtones_and_leading_slash_names(self):
        for filename in ["Ringlist-wb.xml", "talkops-0123abcd.rwb", "/dialplan.xml", "/talkops-0123abcd.rwb"]:
            code, body = self.request("127.0.0.2", filename)
            self.assertEqual(code, 3, filename)
            self.assertEqual(body, self.files["127.0.0.2/" + filename.lstrip("/")])
        for name in ["talkops-89abcdef.rwb", "talkops-0123ABCD.rwb", "talkops-0123abc.rwb", "ringlist-wb.xml", "Ringlist.xml", "//dialplan.xml"]:
            self.assertEqual(self.request("127.0.0.2", name)[0], 5, name)

    def test_other_phone_unknown_source_signed_and_trust_files_are_denied(self):
        for source, name in [
            ("127.0.0.2", "SEP805EC0040506.cnf.xml"),
            ("127.0.0.3", "SEP805EC0010203.cnf.xml"),
            ("127.0.0.4", "SEP805EC0010203.cnf.xml"),
            ("127.0.0.2", "SEP805EC0010203.cnf.xml.sgn"),
            ("127.0.0.2", "ITLFile.tlv"), ("127.0.0.2", "XMLDefault.cnf.xml"),
            ("127.0.0.2", "firmware.loads"),
        ]:
            self.assertEqual(self.request(source, name)[0], 5, (source, name))

    def test_path_traversal_and_absolute_paths_cannot_bypass_source_mapping(self):
        for name in ["../127.0.0.3/SEP805EC0040506.cnf.xml", "127.0.0.3/SEP805EC0040506.cnf.xml", str(self.root / "active/127.0.0.3/SEP805EC0040506.cnf.xml"), "/etc/passwd", ".owner", "SEP805EC0010203.cnf.xml/../dialplan.xml"]:
            self.assertEqual(self.request("127.0.0.2", name)[0], 5, name)

    def test_uploads_cannot_overwrite_even_an_existing_own_file(self):
        for name in ["SEP805EC0010203.cnf.xml", "new.xml", "dialplan.xml", "talkops-0123abcd.rwb", "/Ringlist-wb.xml"]:
            self.assertEqual(self.request("127.0.0.2", name, opcode=2)[0], 5)
        self.assertEqual((self.root / "active/127.0.0.2/dialplan.xml").read_bytes(), b"<DIALTEMPLATE/>")



class SnapshotLease(unittest.TestCase):
    def test_only_recent_numeric_nonlinked_snapshot_leases_are_accepted(self):
        script = Path(__file__).resolve().parents[2] / "docker/talkops/cisco-tftp.py"
        spec = importlib.util.spec_from_file_location("cisco_tftp_guard", script)
        guard = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(guard)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "active").mkdir()
            lease = root / "active/.lease"
            for value, expected in [("1090", True), ("1000", False), ("999", False), ("1091", False), ("nan", False), ("inf", False), ("invalid", False)]:
                lease.write_text(value)
                self.assertEqual(guard.lease_is_valid(root, now=1000), expected, value)
            lease.unlink()
            self.assertFalse(guard.lease_is_valid(root, now=1000))
            target = root / "other-lease"
            target.write_text("1090")
            lease.symlink_to(target)
            self.assertFalse(guard.lease_is_valid(root, now=1000))
            lease.unlink()
            lease.write_text("1" * 65)
            self.assertFalse(guard.lease_is_valid(root, now=1000))

if __name__ == "__main__":
    unittest.main(verbosity=2)
