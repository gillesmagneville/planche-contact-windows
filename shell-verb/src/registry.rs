// SPDX-License-Identifier: GPL-3.0-only

//! Applies the registry plan of `plan.rs` under HKEY_CURRENT_USER.

use windows::core::{Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteTreeW, RegOpenKeyExW, RegQueryInfoKeyW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

use crate::{launch, plan};

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle comes from a successful open/create call.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Creates `key` if needed and sets one of its string values (`name` empty:
/// the default value).
fn set_string(key: &str, name: &str, data: &str) -> Result<()> {
    let mut handle = HKEY::default();
    // SAFETY: plain registry calls with valid, NUL-terminated strings that
    // outlive them.
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut handle,
            None,
        )
        .ok()?;
        let handle = Key(handle);

        let wide: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2);
        let name = HSTRING::from(name);
        let name = if name.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(name.as_ptr())
        };
        RegSetValueExW(handle.0, name, None, REG_SZ, Some(bytes)).ok()
    }
}

/// Deletes `key` and everything below it; a key that does not exist is fine.
fn delete_tree(key: &str) -> Result<()> {
    // SAFETY: plain registry call with a valid string.
    let code = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(key)) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    code.ok()?;
    // RegDeleteTreeW empties the key but leaves the key itself.
    // SAFETY: as above.
    let code = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(key)) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    code.ok()
}

/// Deletes `key` only if it has neither values nor subkeys.
fn prune_if_empty(key: &str) {
    let path = HSTRING::from(key);
    let mut handle = HKEY::default();
    // SAFETY: plain registry calls; the counters are valid for writes.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_READ, &mut handle) != ERROR_SUCCESS {
            return;
        }
        let handle = Key(handle);
        let (mut subkeys, mut values) = (0u32, 0u32);
        let code = RegQueryInfoKeyW(
            handle.0,
            None,
            None,
            None,
            Some(&mut subkeys),
            None,
            None,
            Some(&mut values),
            None,
            None,
            None,
            None,
        );
        drop(handle);
        if code == ERROR_SUCCESS && subkeys == 0 && values == 0 {
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, &path);
        }
    }
}

/// Tells File Explorer that the context menus changed.
fn notify_shell() {
    // SAFETY: documented call with no item.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) }
}

/// Registers the handler for the current user, with `label` as menu text.
pub fn register(label: &str) -> Result<()> {
    let dir = launch::install_dir().map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    for value in plan::registration(&dir.to_string_lossy(), label) {
        set_string(&value.key, value.name, &value.data)?;
    }
    notify_shell();
    Ok(())
}

/// Removes everything `register` wrote. Goes on after a failure so that as
/// much as possible is cleaned, and reports the first error.
pub fn unregister() -> Result<()> {
    let mut first_error = Ok(());
    for tree in plan::trees_to_delete() {
        if let Err(e) = delete_tree(&tree) {
            if first_error.is_ok() {
                first_error = Err(e);
            }
        }
    }
    for key in plan::keys_to_prune() {
        prune_if_empty(&key);
    }
    notify_shell();
    first_error
}
