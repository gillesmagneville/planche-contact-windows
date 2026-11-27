// SPDX-License-Identifier: GPL-3.0-only

//! The context-menu handler itself: a COM object that File Explorer obtains
//! from this program, started on demand as a "local server".
//!
//! Sequence, for one right-click → "Open in Planche-Contact":
//!  1. File Explorer asks COM for the class registered under our identifier;
//!     COM starts `planche-contact-verb.exe -Embedding`.
//!  2. `serve` registers the class factory and pumps messages.
//!  3. File Explorer creates the object, gives it the whole selection in one
//!     call (`IObjectWithSelection::SetSelection`), then calls
//!     `IExecuteCommand::Execute`.
//!  4. `Execute` only queues the command and returns at once, so that File
//!     Explorer is never kept waiting. The message loop then starts the
//!     application on the selection.
//!  5. File Explorer releases the object, and the program exits.
//!
//! Nothing of this runs inside explorer.exe: a failure here cannot crash it.
//! This is the scheme of Microsoft's "ExecuteCommandVerb" sample.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID, PCWSTR};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, LPARAM, POINT, WPARAM};
use windows::Win32::System::Com::{
    CoInitializeEx, CoRegisterClassObject, CoRevokeClassObject, CoTaskMemFree, CoUninitialize,
    IClassFactory, IClassFactory_Impl, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
    REGCLS_SINGLEUSE,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{
    IExecuteCommand, IExecuteCommand_Impl, IObjectWithSelection, IObjectWithSelection_Impl,
    IShellItemArray, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, DispatchMessageW, GetMessageW, PostThreadMessageW, SetTimer,
    TranslateMessage, MSG, WM_APP, WM_TIMER,
};

use crate::{launch, plan};

/// Class identifier of the handler, as a COM GUID.
pub const CLSID: GUID = GUID::from_u128(plan::CLSID);

/// Posted to the serving thread by `Execute`: a command is waiting.
const WM_RUN_COMMAND: u32 = WM_APP + 1;

/// Posted to the serving thread when File Explorer has released the object.
const WM_OBJECT_RELEASED: u32 = WM_APP + 2;

/// If File Explorer never calls us (it should within moments), give up.
const STARTUP_TIMEOUT_MS: u32 = 30_000;

/// Normally the program exits as soon as the object is released, right after
/// the command. Should that release never come, exit anyway after this delay.
const AFTER_COMMAND_TIMEOUT_MS: u32 = 10_000;

/// Thread running `message_loop` (0: none, as in most tests).
static SERVING_THREAD: AtomicU32 = AtomicU32::new(0);

/// What `Execute` leaves for the message loop.
struct Command {
    selection: Option<IShellItemArray>,
    directory: Option<String>,
}

// The command queued by `Execute` on the serving thread.
thread_local! {
    static PENDING: RefCell<Option<Command>> = const { RefCell::new(None) };
}

fn on_serving_thread() -> bool {
    // SAFETY: plain call without arguments.
    let current = unsafe { GetCurrentThreadId() };
    SERVING_THREAD.load(Ordering::SeqCst) == current
}

/// Posts `message` to the serving thread — from that thread only: the object
/// lives there, and anything else is a test running without a message loop.
fn notify_serving_thread(message: u32) {
    if on_serving_thread() {
        // SAFETY: posts a message to the current thread's own queue.
        unsafe {
            let _ = PostThreadMessageW(GetCurrentThreadId(), message, WPARAM(0), LPARAM(0));
        }
    }
}

/// File-system paths of the selected items; items that are not files or
/// folders on disk (a library, a phone…) have none and are left out.
fn selection_paths(items: &IShellItemArray) -> Vec<String> {
    let mut paths = Vec::new();
    // SAFETY: COM calls on a valid interface; the returned string is copied
    // then released with the allocator that created it.
    unsafe {
        let count = items.GetCount().unwrap_or(0);
        for index in 0..count {
            let Ok(item) = items.GetItemAt(index) else {
                continue;
            };
            let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) else {
                continue;
            };
            if !name.is_null() {
                paths.push(String::from_utf16_lossy(name.as_wide()));
                CoTaskMemFree(Some(name.0 as *const _));
            }
        }
    }
    paths
}

/// Starts the application on what was selected. A right-click on the
/// background of a folder has no selection: the folder itself is the target.
fn run_command(command: Command) -> std::io::Result<()> {
    let mut paths = command
        .selection
        .as_ref()
        .map(selection_paths)
        .unwrap_or_default();
    if paths.is_empty() {
        if let Some(directory) = command.directory {
            if !directory.is_empty() {
                paths.push(directory);
            }
        }
    }
    if paths.is_empty() {
        return Ok(());
    }
    let launched = launch::launch_app(&paths)?;
    // File Explorer allowed us to take the foreground; pass that right on to
    // the application, so that its window opens in front.
    // SAFETY: plain call with a process identifier.
    unsafe {
        let _ = AllowSetForegroundWindow(launched.pid);
    }
    Ok(())
}

#[implement(IExecuteCommand, IObjectWithSelection)]
struct OpenVerb {
    selection: RefCell<Option<IShellItemArray>>,
    directory: RefCell<Option<String>>,
}

impl Drop for OpenVerb {
    fn drop(&mut self) {
        notify_serving_thread(WM_OBJECT_RELEASED);
    }
}

impl IObjectWithSelection_Impl for OpenVerb_Impl {
    fn SetSelection(&self, psia: Ref<IShellItemArray>) -> Result<()> {
        *self.selection.borrow_mut() = psia.cloned();
        Ok(())
    }

    fn GetSelection(&self, riid: *const GUID, ppv: *mut *mut core::ffi::c_void) -> Result<()> {
        if ppv.is_null() {
            return Err(E_POINTER.into());
        }
        match self.selection.borrow().as_ref() {
            // SAFETY: `riid` and `ppv` come from the caller, per the COM contract.
            Some(items) => unsafe { items.query(riid, ppv).ok() },
            None => {
                // SAFETY: `ppv` was checked above.
                unsafe { *ppv = std::ptr::null_mut() };
                Err(E_FAIL.into())
            }
        }
    }
}

impl IExecuteCommand_Impl for OpenVerb_Impl {
    fn SetKeyState(&self, _grfkeystate: u32) -> Result<()> {
        Ok(())
    }

    fn SetParameters(&self, _pszparameters: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn SetPosition(&self, _pt: &POINT) -> Result<()> {
        Ok(())
    }

    fn SetShowWindow(&self, _nshow: i32) -> Result<()> {
        Ok(())
    }

    fn SetNoShowUI(&self, _fnoshowui: BOOL) -> Result<()> {
        Ok(())
    }

    fn SetDirectory(&self, pszdirectory: &PCWSTR) -> Result<()> {
        let directory = if pszdirectory.is_null() {
            None
        } else {
            // SAFETY: a non-null, NUL-terminated string per the COM contract.
            Some(String::from_utf16_lossy(unsafe { pszdirectory.as_wide() }))
        };
        *self.directory.borrow_mut() = directory;
        Ok(())
    }

    fn Execute(&self) -> Result<()> {
        let command = Command {
            selection: self.selection.borrow().clone(),
            directory: self.directory.borrow().clone(),
        };
        if on_serving_thread() {
            // The normal case: answer File Explorer at once, work afterwards.
            PENDING.with(|pending| *pending.borrow_mut() = Some(command));
            notify_serving_thread(WM_RUN_COMMAND);
            Ok(())
        } else {
            // No message loop to hand the command to: run it right here.
            run_command(command).map_err(|e| {
                crate::log(&format!("could not start the application: {e}"));
                E_FAIL.into()
            })
        }
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        if ppvobject.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: `ppvobject` was checked above.
        unsafe { *ppvobject = std::ptr::null_mut() };
        if !punkouter.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let verb: IUnknown = OpenVerb {
            selection: RefCell::new(None),
            directory: RefCell::new(None),
        }
        .into();
        // SAFETY: `riid` and `ppvobject` come from the caller, per the COM contract.
        unsafe { verb.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, _flock: BOOL) -> Result<()> {
        Ok(())
    }
}

/// The class factory of the handler, as registered with COM by `serve`.
pub fn class_factory() -> IClassFactory {
    Factory.into()
}

/// Pumps the messages of the current thread, which becomes the serving
/// thread, until the queued command has run and the object has been released
/// — or until one of the timeouts elapses.
fn message_loop() {
    // SAFETY: standard message loop on the current thread; every pointer
    // passed is valid for the duration of the call.
    unsafe {
        SERVING_THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
        SetTimer(None, 0, STARTUP_TIMEOUT_MS, None);

        let mut released = false;
        let mut msg = MSG::default();
        // GetMessageW returns 0 on WM_QUIT and -1 on error.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            if msg.hwnd.0.is_null() {
                match msg.message {
                    WM_TIMER => break,
                    WM_RUN_COMMAND => {
                        if let Some(command) = PENDING.with(|pending| pending.borrow_mut().take()) {
                            if let Err(e) = run_command(command) {
                                crate::log(&format!("could not start the application: {e}"));
                            }
                        }
                        if released {
                            break;
                        }
                        SetTimer(None, 0, AFTER_COMMAND_TIMEOUT_MS, None);
                        continue;
                    }
                    WM_OBJECT_RELEASED => {
                        released = true;
                        // A command still queued runs first (see above).
                        if PENDING.with(|pending| pending.borrow().is_none()) {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        SERVING_THREAD.store(0, Ordering::SeqCst);
    }
}

/// Runs as a COM local server for one command (see the module description).
pub fn serve() -> Result<()> {
    // SAFETY: standard single-threaded-apartment COM server sequence.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let factory = class_factory();
        let result =
            match CoRegisterClassObject(&CLSID, &factory, CLSCTX_LOCAL_SERVER, REGCLS_SINGLEUSE) {
                Ok(cookie) => {
                    message_loop();
                    // Already revoked by COM after the single use: ignore the result.
                    let _ = CoRevokeClassObject(cookie);
                    Ok(())
                }
                Err(e) => Err(e),
            };
        drop(factory);
        CoUninitialize();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use windows::core::HSTRING;
    use windows::Win32::System::Com::IPersist;
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHCreateShellItemArrayFromIDLists};

    /// COM must be initialized on each test thread.
    struct ComApartment;

    impl ComApartment {
        fn enter() -> Self {
            // SAFETY: paired with CoUninitialize in Drop, on the same thread.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap() };
            ComApartment
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: see `enter`.
            unsafe { CoUninitialize() };
        }
    }

    fn new_verb() -> IExecuteCommand {
        let factory = class_factory();
        // SAFETY: valid factory, no aggregation.
        unsafe { factory.CreateInstance(None).unwrap() }
    }

    /// A folder with two image-named files, one with accents and a space.
    fn sample_files(tag: &str) -> (PathBuf, Vec<PathBuf>) {
        let dir = std::env::temp_dir().join(format!("pc-verb-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let files = vec![dir.join("plain.jpg"), dir.join("été photo.cr2")];
        for file in &files {
            std::fs::write(file, b"x").unwrap();
        }
        (dir, files)
    }

    fn shell_selection(files: &[PathBuf]) -> IShellItemArray {
        // SAFETY: the identifier lists are valid until freed below, after
        // the array (which copies them) has been created.
        unsafe {
            let ids: Vec<*const ITEMIDLIST> = files
                .iter()
                .map(|f| ILCreateFromPathW(&HSTRING::from(f.as_os_str())) as *const ITEMIDLIST)
                .collect();
            assert!(
                ids.iter().all(|id| !id.is_null()),
                "ILCreateFromPathW failed"
            );
            let array = SHCreateShellItemArrayFromIDLists(&ids).unwrap();
            for id in ids {
                ILFree(Some(id));
            }
            array
        }
    }

    /// Compares paths whatever their spelling (short 8.3 names, letter case).
    fn same_files(found: &[String], expected: &[PathBuf]) -> bool {
        let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap();
        found.len() == expected.len()
            && found
                .iter()
                .zip(expected)
                .all(|(f, e)| canon(std::path::Path::new(f)) == canon(e))
    }

    /// Stand-in for the application: this very test program, which exits at
    /// once when started with arguments it does not know.
    fn use_stand_in_application() {
        std::env::set_var(
            "PLANCHE_CONTACT_APP",
            std::env::current_exe().unwrap(),
        );
    }

    /// Looks, in the temporary folder, for the selection file that this
    /// process wrote for exactly `files`; deletes it and tells if it was there.
    fn take_selection_file(files: &[PathBuf]) -> bool {
        let prefix = format!("planche-contact-selection-{}-", std::process::id());
        let mut found = false;
        for entry in std::fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            if !entry.file_name().to_string_lossy().starts_with(&prefix) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let listed: Vec<String> = content.lines().map(str::to_string).collect();
            if listed.len() == files.len() && same_files(&listed, files) {
                found = true;
                let _ = std::fs::remove_file(entry.path());
            }
        }
        found
    }

    #[test]
    fn factory_creates_an_object_with_both_interfaces() {
        let _com = ComApartment::enter();
        let verb = new_verb();
        assert!(verb.cast::<IObjectWithSelection>().is_ok());
        assert!(verb.cast::<IUnknown>().is_ok());
        assert!(verb.cast::<IPersist>().is_err(), "unexpected interface");
    }

    #[test]
    fn selection_is_returned_as_file_system_paths() {
        let _com = ComApartment::enter();
        let (dir, files) = sample_files("paths");
        let verb = new_verb();
        let with_selection = verb.cast::<IObjectWithSelection>().unwrap();
        // SAFETY: valid interfaces on this thread.
        unsafe {
            assert!(
                with_selection.GetSelection::<IShellItemArray>().is_err(),
                "no selection yet"
            );
            with_selection
                .SetSelection(&shell_selection(&files))
                .unwrap();
            let stored: IShellItemArray = with_selection.GetSelection().unwrap();
            let paths = selection_paths(&stored);
            assert!(same_files(&paths, &files), "{paths:?} != {files:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_without_selection_does_nothing() {
        let _com = ComApartment::enter();
        let verb = new_verb();
        // SAFETY: valid interface on this thread.
        unsafe { verb.Execute().unwrap() };
    }

    #[test]
    fn background_of_a_folder_targets_the_folder() {
        let _com = ComApartment::enter();
        use_stand_in_application();
        let (dir, _files) = sample_files("background");
        let verb = new_verb();
        // SAFETY: valid interface on this thread.
        unsafe {
            verb.SetDirectory(&HSTRING::from(dir.as_os_str())).unwrap();
            verb.Execute().unwrap();
        }
        assert!(
            take_selection_file(std::slice::from_ref(&dir)),
            "no selection file for {dir:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_hands_the_selection_to_the_application() {
        let _com = ComApartment::enter();
        use_stand_in_application();
        let (dir, files) = sample_files("exec");
        let verb = new_verb();
        // SAFETY: valid interfaces on this thread.
        unsafe {
            verb.cast::<IObjectWithSelection>()
                .unwrap()
                .SetSelection(&shell_selection(&files))
                .unwrap();
            verb.Execute().unwrap();
        }
        assert!(
            take_selection_file(&files),
            "no selection file listing {files:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The real sequence of a served command, on one thread: `Execute` only
    /// queues the work, the object is released, and the message loop runs the
    /// command then returns by itself.
    #[test]
    fn served_command_runs_after_execute_returns_then_the_loop_ends() {
        let _com = ComApartment::enter();
        use_stand_in_application();
        let (dir, files) = sample_files("served");

        // SAFETY: plain call without arguments.
        SERVING_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        let verb = new_verb();
        // SAFETY: valid interfaces on this thread.
        unsafe {
            verb.cast::<IObjectWithSelection>()
                .unwrap()
                .SetSelection(&shell_selection(&files))
                .unwrap();
            verb.Execute().unwrap();
        }
        assert!(
            !take_selection_file(&files),
            "Execute must not do the work itself"
        );
        drop(verb);

        let started = std::time::Instant::now();
        message_loop();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the loop waited for a timeout instead of ending by itself"
        );
        assert!(
            take_selection_file(&files),
            "no selection file listing {files:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
