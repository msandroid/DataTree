use crate::model::VolumeInfo;

/// List mounted volumes, skipping virtual filesystems used by the GUI picker.
#[must_use]
pub fn list_volumes() -> Vec<VolumeInfo> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut drives = Vec::new();
    for disk in &disks {
        let mut fs_str = disk.file_system().to_string_lossy().into_owned();
        let fs_lower = fs_str.to_lowercase();
        if matches!(
            fs_lower.as_str(),
            "tmpfs"
                | "proc"
                | "sysfs"
                | "devtmpfs"
                | "devfs"
                | "cgroup"
                | "pstore"
                | "overlay"
                | "squashfs"
                | "nsfs"
                | "ramfs"
        ) {
            continue;
        }

        if fs_lower == "fuseblk"
            && edirstat_core::fs_utils::find_mft_file(disk.mount_point()).is_some()
        {
            fs_str = "ntfs-3g".to_string();
        }

        let mount_str = disk.mount_point().to_string_lossy();
        let name_str = disk.name().to_string_lossy();
        let name = if name_str.trim().is_empty() || name_str == mount_str {
            disk.mount_point()
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| mount_str.into_owned())
        } else {
            name_str.trim().to_string()
        };

        drives.push(VolumeInfo {
            name,
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            fs_type: fs_str,
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
            is_removable: disk.is_removable(),
        });
    }
    drives.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    drives
}
