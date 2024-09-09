use std::fs::Metadata;

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;
#[cfg(target_os = "windows")]
#[inline(always)]
pub fn get_filesize(meta: &Metadata) -> u64 {
    meta.file_size()
}

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(target_os = "linux")]
#[inline(always)]
pub fn get_filesize(meta: &Metadata) -> u64 {
    meta.st_size()
}

