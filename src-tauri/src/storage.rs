use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let serialized = serde_json::to_vec_pretty(value)?;
    write_atomically(path, &serialized)
}

pub fn write_atomically(path: &Path, contents: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::message(format!("写入目标缺少父目录: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let temporary_path = sibling_temporary_path(path, "tmp");
    let write_result = (|| -> AppResult<()> {
        let mut temporary_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        temporary_file.write_all(contents)?;
        temporary_file.flush()?;
        temporary_file.sync_all()?;
        drop(temporary_file);
        replace_destination(&temporary_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn sibling_temporary_path(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());
    path.with_file_name(format!(".{file_name}.{}.{suffix}", Uuid::new_v4()))
}

#[cfg(not(windows))]
fn replace_destination(temporary_path: &Path, destination_path: &Path) -> AppResult<()> {
    fs::rename(temporary_path, destination_path)?;
    Ok(())
}

#[cfg(windows)]
fn replace_destination(temporary_path: &Path, destination_path: &Path) -> AppResult<()> {
    if !destination_path.exists() {
        fs::rename(temporary_path, destination_path)?;
        return Ok(());
    }

    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
    }

    let destination_wide: Vec<u16> = destination_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let temporary_wide: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    const REPLACEFILE_WRITE_THROUGH: u32 = 0x0000_0001;
    let replaced = unsafe {
        ReplaceFileW(
            destination_wide.as_ptr(),
            temporary_wide.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_WRITE_THROUGH,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if replaced == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomically_replaces_existing_contents() {
        let test_directory =
            std::env::temp_dir().join(format!("video-tool-storage-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&test_directory).expect("create test directory");
        let destination_path = test_directory.join("state.json");
        fs::write(&destination_path, b"old").expect("write old contents");

        write_atomically(&destination_path, b"new").expect("replace contents");

        assert_eq!(fs::read(&destination_path).expect("read contents"), b"new");
        let remaining_names: Vec<String> = fs::read_dir(&test_directory)
            .expect("read test directory")
            .map(|entry| {
                entry
                    .expect("read test entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(remaining_names, vec!["state.json"]);

        fs::remove_dir_all(test_directory).expect("remove test directory");
    }
}
