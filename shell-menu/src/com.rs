// SPDX-License-Identifier: GPL-3.0-only

//! The COM side of the Windows 11 entry: the two functions every in-process
//! COM server exports, a class factory, and the `IExplorerCommand` object.
//!
//! Sequence, for one right-click on the first level of the menu:
//!  1. File Explorer asks COM for the class named by the package manifest;
//!     COM starts `dllhost.exe`, which loads this DLL and calls
//!     `DllGetClassObject`.
//!  2. File Explorer asks the object its title, its icon and its state for
//!     the selection (`GetState`: the entry is hidden when nothing selected
//!     is a photo or a folder).
//!  3. On a click, `Invoke` receives the whole selection and starts the
//!     window on it, as the classic handler does.

use std::path::PathBuf;

use windows::core::{
    implement, IUnknown, Interface, Ref, Result, BOOL, GUID, HRESULT, PCWSTR, PWSTR,
};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOTIMPL, E_POINTER, HMODULE,
    S_FALSE,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::Com::{IBindCtx, IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::System::SystemServices::{SFGAO_FILESYSTEM, SFGAO_FOLDER};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray, SHStrDupW,
    ECF_DEFAULT, ECS_ENABLED, ECS_HIDDEN, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

use planche_contact_verb::{com::selection_paths, labels, launch, log, plan};

const CLASS: GUID = GUID::from_u128(crate::CLSID);

/// Folder of this DLL: the installation folder, where the window is.
fn install_dir() -> Option<PathBuf> {
    let mut module = HMODULE::default();
    let mut buffer = [0u16; 1024];
    // SAFETY: the address is that of a function of this DLL, which stays
    // loaded while it runs; the buffer outlives the call.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(install_dir as *const () as *const u16),
            &mut module,
        )
        .ok()?;
        let length = GetModuleFileNameW(Some(module), &mut buffer) as usize;
        if length == 0 || length >= buffer.len() {
            return None;
        }
        PathBuf::from(String::from_utf16_lossy(&buffer[..length]))
            .parent()
            .map(PathBuf::from)
    }
}

/// A copy of `text` allocated as the caller of an `IExplorerCommand` method
/// expects (it frees it with `CoTaskMemFree`).
fn com_string(text: &str) -> Result<PWSTR> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: a NUL-terminated string that lives for the duration of the call.
    unsafe { SHStrDupW(PCWSTR(wide.as_ptr())) }
}

/// Path and "is a folder" of each selected item that is on disk.
fn selection_kinds(items: &IShellItemArray) -> Vec<(String, bool)> {
    let mut kinds = Vec::new();
    // SAFETY: COM calls on a valid interface; the returned string is copied
    // then released with the allocator that created it.
    unsafe {
        let count = items.GetCount().unwrap_or(0);
        for index in 0..count {
            let Ok(item) = items.GetItemAt(index) else {
                continue;
            };
            let attributes = item
                .GetAttributes(SFGAO_FOLDER | SFGAO_FILESYSTEM)
                .unwrap_or_default();
            if attributes & SFGAO_FILESYSTEM != SFGAO_FILESYSTEM {
                continue;
            }
            let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) else {
                continue;
            };
            if !name.is_null() {
                kinds.push((
                    String::from_utf16_lossy(name.as_wide()),
                    attributes & SFGAO_FOLDER == SFGAO_FOLDER,
                ));
                windows::Win32::System::Com::CoTaskMemFree(Some(name.0 as *const _));
            }
        }
    }
    kinds
}

#[implement(IExplorerCommand)]
struct OpenCommand;

impl IExplorerCommand_Impl for OpenCommand_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        // The language of Windows, at the moment the menu opens.
        // SAFETY: plain call without arguments.
        let language = unsafe { GetUserDefaultUILanguage() };
        com_string(labels::menu_label(language))
    }

    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        let dir = install_dir().ok_or(E_FAIL)?;
        com_string(&format!("{},0", dir.join(plan::APP_EXE).display()))
    }

    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLASS)
    }

    fn GetState(&self, items: Ref<IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        // No item array: the background of a folder, always offered.
        let shown = match items.as_ref() {
            None => true,
            Some(items) => crate::offers_entry(&selection_kinds(items)),
        };
        Ok(if shown {
            ECS_ENABLED.0 as u32
        } else {
            ECS_HIDDEN.0 as u32
        })
    }

    fn Invoke(&self, items: Ref<IShellItemArray>, _context: Ref<IBindCtx>) -> Result<()> {
        let paths = items.as_ref().map(selection_paths).unwrap_or_default();
        if paths.is_empty() {
            return Ok(());
        }
        let dir = install_dir().ok_or(E_FAIL)?;
        match launch::launch_app_at(&dir.join(plan::APP_EXE), &paths) {
            Ok(launched) => {
                // SAFETY: plain call with a process identifier.
                unsafe {
                    let _ = AllowSetForegroundWindow(launched.pid);
                }
                Ok(())
            }
            Err(e) => {
                log(&format!(
                    "Windows 11 menu: could not start the application: {e}"
                ));
                Err(E_FAIL.into())
            }
        }
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        object: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        if object.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: `object` was checked above.
        unsafe { *object = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = OpenCommand.into();
        // SAFETY: `riid` and `object` come from the caller, per the COM contract.
        unsafe { command.query(riid, object).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

/// Entry point of COM: the class factory of our class.
///
/// # Safety
/// Called by COM with valid pointers, per its contract.
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    riid: *const GUID,
    object: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if clsid.is_null() || riid.is_null() || object.is_null() {
        return E_POINTER;
    }
    // SAFETY: checked above.
    unsafe {
        *object = std::ptr::null_mut();
        if *clsid != CLASS {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        factory.query(riid, object)
    }
}

/// Entry point of COM: whether the DLL may be unloaded. It holds no state, but
/// the surrogate is short-lived anyway: keep it simple and stay loaded.
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}
