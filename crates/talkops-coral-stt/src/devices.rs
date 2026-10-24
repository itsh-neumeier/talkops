//! Edge TPU discovery from sysfs and /dev; no driver calls.
//!
//! USB: the accelerator enumerates as `1a6e:089a` (Global Unichip) until the
//! runtime loads its firmware. Then it resets and enumerates again as
//! `18d1:9302` (Google), often with a new bus device number. libedgetpu
//! addresses USB devices by index (`usb:0`), so the renumbering does not
//! change the identity used here. PCIe and M.2 cards appear as
//! `/dev/apex_N` (gasket/apex driver).

use std::path::Path;

use serde::Serialize;

const USB_BOOT: (&str, &str) = ("1a6e", "089a");
const USB_READY: (&str, &str) = ("18d1", "9302");

/// A device that a backend can open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceSpec {
    /// libedgetpu device string (`usb:0`, `pci:0`), `cpu` or `mock:N`.
    pub id: String,
    /// `usb`, `pcie`, `cpu` or `mock`.
    pub kind: String,
    /// Device node, e.g. `/dev/bus/usb/002/007` or `/dev/apex_0`.
    pub node: Option<String>,
    /// `firmware-not-loaded` for a USB device before its first use.
    pub note: Option<String>,
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
}

/// USB accelerators, sorted by sysfs name (bus topology), so the index is
/// stable while the devices stay plugged in.
pub fn usb(sysfs: &Path) -> Vec<DeviceSpec> {
    let Ok(entries) = std::fs::read_dir(sysfs) else {
        return Vec::new();
    };
    let mut found: Vec<(String, bool, Option<String>)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let dir = e.path();
            let ids = (read(&dir.join("idVendor"))?, read(&dir.join("idProduct"))?);
            let ready = (ids.0.as_str(), ids.1.as_str()) == USB_READY;
            if !ready && (ids.0.as_str(), ids.1.as_str()) != USB_BOOT {
                return None;
            }
            let node = match (
                read(&dir.join("busnum")).and_then(|b| b.parse::<u32>().ok()),
                read(&dir.join("devnum")).and_then(|d| d.parse::<u32>().ok()),
            ) {
                (Some(b), Some(d)) => Some(format!("/dev/bus/usb/{b:03}/{d:03}")),
                _ => None,
            };
            Some((e.file_name().to_string_lossy().into_owned(), ready, node))
        })
        .collect();
    found.sort();
    found
        .into_iter()
        .enumerate()
        .map(|(i, (_, ready, node))| DeviceSpec {
            id: format!("usb:{i}"),
            kind: "usb".into(),
            node,
            note: (!ready).then(|| "firmware-not-loaded".into()),
        })
        .collect()
}

/// PCIe/M.2 cards (`/dev/apex_N`), sorted by number.
pub fn pcie(dev: &Path) -> Vec<DeviceSpec> {
    let Ok(entries) = std::fs::read_dir(dev) else {
        return Vec::new();
    };
    let mut nodes: Vec<(u32, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let n = name.strip_prefix("apex_")?.parse().ok()?;
            Some((n, e.path().to_string_lossy().into_owned()))
        })
        .collect();
    nodes.sort();
    nodes
        .into_iter()
        .enumerate()
        .map(|(i, (_, node))| DeviceSpec {
            id: format!("pci:{i}"),
            kind: "pcie".into(),
            node: Some(node),
            note: None,
        })
        .collect()
}

/// All Edge TPUs: USB first, then PCIe.
pub fn discover(sysfs: &Path, dev: &Path) -> Vec<DeviceSpec> {
    let mut all = usb(sysfs);
    all.extend(pcie(dev));
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb_dir(root: &Path, name: &str, ids: (&str, &str), bus: u32, dev: u32) {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        for (k, v) in [
            ("idVendor", ids.0.to_owned()),
            ("idProduct", ids.1.to_owned()),
            ("busnum", bus.to_string()),
            ("devnum", dev.to_string()),
        ] {
            std::fs::write(d.join(k), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn finds_usb_in_both_states_and_pcie() {
        let root = std::env::temp_dir().join(format!("coral-dev-{}", std::process::id()));
        let (sysfs, dev) = (root.join("sys"), root.join("dev"));
        std::fs::create_dir_all(&dev).unwrap();
        usb_dir(&sysfs, "2-1", USB_READY, 2, 7);
        usb_dir(&sysfs, "2-2", USB_BOOT, 2, 3);
        usb_dir(&sysfs, "1-1", ("046d", "c52b"), 1, 2);
        std::fs::write(dev.join("apex_1"), "").unwrap();
        std::fs::write(dev.join("apex_0"), "").unwrap();
        std::fs::write(dev.join("null"), "").unwrap();
        let found = discover(&sysfs, &dev);
        let ids: Vec<&str> = found.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["usb:0", "usb:1", "pci:0", "pci:1"]);
        assert_eq!(found[0].node.as_deref(), Some("/dev/bus/usb/002/007"));
        assert_eq!(found[0].note, None);
        assert_eq!(found[1].note.as_deref(), Some("firmware-not-loaded"));
        assert!(found[2].node.as_deref().unwrap().ends_with("apex_0"));
        // After the firmware load the second stick has a new device number:
        // the index (and thus the identity) stays.
        std::fs::remove_dir_all(sysfs.join("2-2")).unwrap();
        usb_dir(&sysfs, "2-2", USB_READY, 2, 9);
        let again = usb(&sysfs);
        assert_eq!(again[1].id, "usb:1");
        assert_eq!(again[1].node.as_deref(), Some("/dev/bus/usb/002/009"));
        assert_eq!(again[1].note, None);
        assert!(discover(&root.join("missing"), &root.join("missing")).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
