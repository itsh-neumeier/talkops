"""Edge TPU device discovery from sysfs and /dev (no driver calls).

USB: the accelerator enumerates as 1a6e:089a (Global Unichip) until the
runtime loads the firmware. It then resets and enumerates again as 18d1:9302
(Google), often with a new device number. A device that shows only the first
ID is present but not yet initialised; this is normal before first use.
PCIe/M.2: the gasket/apex driver creates /dev/apex_N.
"""
from __future__ import annotations

import os
from pathlib import Path

USB_BOOT = ("1a6e", "089a")
USB_READY = ("18d1", "9302")


def _read(path: Path) -> str | None:
    try:
        return path.read_text().strip()
    except OSError:
        return None


def usb_devices(sysfs: Path = Path("/sys/bus/usb/devices")) -> list[dict]:
    found = []
    if not sysfs.is_dir():
        return found
    for dev in sorted(sysfs.iterdir()):
        ids = (_read(dev / "idVendor"), _read(dev / "idProduct"))
        if ids not in (USB_BOOT, USB_READY):
            continue
        busnum, devnum = _read(dev / "busnum"), _read(dev / "devnum")
        found.append({
            "kind": "usb",
            "sysfs": dev.name,
            "state": "ready" if ids == USB_READY else "firmware-not-loaded",
            "vendor_product": ":".join(ids),
            "speed_mbps": _read(dev / "speed"),
            "node": f"/dev/bus/usb/{int(busnum):03d}/{int(devnum):03d}" if busnum and devnum else None,
        })
    return found


def pcie_devices(dev: Path = Path("/dev")) -> list[dict]:
    found = []
    if not dev.is_dir():
        return found
    for node in sorted(dev.glob("apex_*")):
        found.append({
            "kind": "pcie",
            "node": str(node),
            "accessible": os.access(node, os.R_OK | os.W_OK),
        })
    return found


def delegate_device(entry: dict, index: int) -> str:
    """The `device` option of the libedgetpu delegate for one entry.

    libedgetpu numbers USB and PCIe devices separately ("usb:0", "pci:0").
    """
    return f"{'usb' if entry['kind'] == 'usb' else 'pci'}:{index}"


def discover(sysfs: Path = Path("/sys/bus/usb/devices"), dev: Path = Path("/dev")) -> list[dict]:
    out = []
    usb = usb_devices(sysfs)
    for i, e in enumerate(usb):
        out.append({**e, "delegate_device": delegate_device(e, i)})
    for i, e in enumerate(pcie_devices(dev)):
        out.append({**e, "delegate_device": delegate_device(e, i)})
    return out
