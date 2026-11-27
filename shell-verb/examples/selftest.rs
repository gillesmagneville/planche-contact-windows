// SPDX-License-Identifier: GPL-3.0-only

//! Checks the whole chain on a real Windows, the way File Explorer uses it:
//! menu handler → application window.
//!
//!   selftest.exe
//!       On a machine where Planche-Contact is installed: invokes the
//!       installed menu handler on a sample folder, exactly as a right-click →
//!       "Open in Planche-Contact" would, and checks that the window opens and stays
//!       up. The window is then closed.
//!
//!   selftest.exe --dist DIR
//!       For the continuous integration, on the freshly built folder DIR
//!       (needs administrator rights, which a CI machine has): first checks
//!       that DIR holds what the window needs, then starts the window
//!       directly, then goes through DIR's menu handler, registered for the
//!       duration of the test.
//!
//! "The window opens" means: a visible window of the application is on the
//! screen, it is still there some twenty seconds later, and the application
//! logged no error.
//!
//! Exit code 0 when every check passed; otherwise the failed step is printed,
//! with what is known about it (exit code of the application, its error log).
//!
//! Why administrator rights with --dist: a program run as administrator does
//! not see the handlers registered for the current user only (a Windows
//! security rule), and a CI job runs as administrator. The handler is
//! therefore registered machine-wide for the test, then removed. File
//! Explorer itself never runs as administrator and uses the per-user
//! registration made by the installer.

#[cfg(windows)]
mod windows_only {
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    use planche_contact_verb::com::CLSID;
    use planche_contact_verb::plan;
    use windows::core::{Interface, BOOL, HSTRING, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IExecuteCommand, ILCreateFromPathW, ILFree, IObjectWithSelection, IsUserAnAdmin,
        SHCreateShellItemArrayFromIDLists,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    /// How long the window must stay up, once shown, to be considered
    /// started. When the window system hits an error at start-up, the program
    /// only disappears some ten seconds later: the wait must be clearly longer.
    const STAY_UP: Duration = Duration::from_secs(20);

    /// How long the application has to show its window.
    const START_TIMEOUT: Duration = Duration::from_secs(40);

    /// The engine, which the window runs.
    const ENGINE_EXE: &str = r"engine\planche-contact-engine.exe";

    /// A one-pixel, 24-bit BMP image.
    const PIXEL_BMP: [u8; 58] = [
        66, 77, 58, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
        24, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255,
        255, 0,
    ];

    fn machine_class_key() -> String {
        format!(r"HKLM\Software\Classes\CLSID\{}", plan::clsid_braced())
    }

    /// Runs a command-line tool and returns what it printed.
    fn output_of(program: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("could not run {program}: {e}"))?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr))
    }

    fn app_is_running() -> bool {
        let filter = format!("IMAGENAME eq {}", plan::APP_EXE);
        output_of("tasklist", &["/FI", &filter, "/NH"])
            .map(|out| out.to_lowercase().contains(&plan::APP_EXE.to_lowercase()))
            .unwrap_or(false)
    }

    /// Stops the application, however it was started, and waits until it is
    /// really gone, so that the next step does not see its last moments.
    fn close_app(child: Option<&mut Child>) {
        let _ = output_of("taskkill", &["/F", "/IM", plan::APP_EXE]);
        if let Some(child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let start = Instant::now();
        while app_is_running() && start.elapsed() < Duration::from_secs(15) {
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// The file name of the program a window belongs to, in lower case.
    fn program_of_window(window: HWND) -> Option<String> {
        let mut pid = 0u32;
        // SAFETY: `pid` outlives the call; the process handle is closed
        // before returning; the buffer length passed is the real one.
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 1024];
            let mut length = buffer.len() as u32;
            let named = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(process);
            named.ok()?;
            let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
            Some(path.file_name()?.to_string_lossy().to_lowercase())
        }
    }

    /// Called by `EnumWindows` for each top-level window: notes the title of
    /// the visible, titled windows of the application.
    unsafe extern "system" fn note_app_window(window: HWND, titles: LPARAM) -> BOOL {
        // SAFETY: `titles` is the address of the Vec<String> owned by
        // `visible_app_windows`, which is waiting for `EnumWindows` to return.
        let titles = &mut *(titles.0 as *mut Vec<String>);
        if IsWindowVisible(window).as_bool() {
            let mut buffer = [0u16; 512];
            let length = GetWindowTextW(window, &mut buffer).max(0) as usize;
            if length > 0
                && program_of_window(window).as_deref() == Some(&plan::APP_EXE.to_lowercase())
            {
                titles.push(String::from_utf16_lossy(&buffer[..length]));
            }
        }
        BOOL::from(true) // go on with the next window
    }

    /// The titles of the application's windows currently on the screen.
    fn visible_app_windows() -> Vec<String> {
        let mut titles: Vec<String> = Vec::new();
        // SAFETY: the callback only uses the pointer during this call.
        unsafe {
            let _ = EnumWindows(
                Some(note_app_window),
                LPARAM(&mut titles as *mut Vec<String> as isize),
            );
        }
        titles
    }

    /// The application appends the errors nobody handled to this file.
    fn crash_log() -> PathBuf {
        std::env::temp_dir().join("planche-contact-app.log")
    }

    fn check_no_crash_logged() -> Result<(), String> {
        match std::fs::read_to_string(crash_log()) {
            Ok(content) if !content.trim().is_empty() => Err(format!(
                "the application logged an unhandled error:\n{content}"
            )),
            _ => Ok(()),
        }
    }

    /// The exit code of the application when it has stopped; `None` while it
    /// runs, and for an application this program did not start itself.
    fn exit_code(child: &mut Option<&mut Child>) -> Option<u32> {
        let status = child.as_mut()?.try_wait().ok()??;
        // The code as Windows shows it (0xC0000005, not a negative number).
        Some(status.code().unwrap_or(-1) as u32)
    }

    /// Everything known about an application that stopped by itself.
    fn stopped(started_by: &str, when: &str, code: Option<u32>) -> String {
        let code = match code {
            Some(code @ 0xC000_027B) => format!(
                " Exit code {code:#010X}: an error inside the window system (WinUI), \
                 typically a resource file missing from the folder of the application."
            ),
            Some(code @ 0xE043_4352) => {
                format!(" Exit code {code:#010X}: an unhandled .NET error.")
            }
            Some(code) => format!(" Exit code {code:#010X}."),
            None => String::new(),
        };
        let logged = match check_no_crash_logged() {
            Err(logged) => format!("\n{logged}"),
            Ok(()) => " It logged no error.".to_string(),
        };
        format!(
            "{started_by}: {} stopped by itself {when}.{code}{logged}",
            plan::APP_EXE
        )
    }

    /// Waits for a window of the application to be on the screen (within
    /// `START_TIMEOUT`), then checks that it is still there after `STAY_UP`
    /// and that nothing was logged. `child` is the application when this
    /// program started it itself.
    fn expect_window_up(started_by: &str, mut child: Option<&mut Child>) -> Result<(), String> {
        let start = Instant::now();
        let mut seen_running = false;
        let title = loop {
            if let Some(code) = exit_code(&mut child) {
                return Err(stopped(started_by, "before showing a window", Some(code)));
            }
            if let Some(title) = visible_app_windows().into_iter().next() {
                break title;
            }
            let running = app_is_running();
            if seen_running && !running {
                return Err(stopped(started_by, "before showing a window", None));
            }
            seen_running |= running;
            if start.elapsed() > START_TIMEOUT {
                return Err(if running {
                    format!(
                        "{started_by}: {} is running but showed no window within {} s",
                        plan::APP_EXE,
                        START_TIMEOUT.as_secs()
                    )
                } else {
                    format!(
                        "{started_by}: {} did not start within {} s (see \
                         planche-contact-verb.log in the temporary folder)",
                        plan::APP_EXE,
                        START_TIMEOUT.as_secs()
                    )
                });
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        println!(
            "    window \"{title}\" on the screen after {:.1} s",
            start.elapsed().as_secs_f32()
        );

        let shown = Instant::now();
        while shown.elapsed() < STAY_UP {
            if let Some(code) = exit_code(&mut child) {
                return Err(stopped(started_by, "after showing its window", Some(code)));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if visible_app_windows().is_empty() {
            return Err(if app_is_running() {
                format!("{started_by}: the window disappeared, although the application still runs")
            } else {
                stopped(started_by, "after showing its window", None)
            });
        }
        check_no_crash_logged()
    }

    /// Checks that the published folder holds what the window needs to start.
    /// The resource index (.pri) is the file "dotnet publish" forgets for an
    /// application without MSIX packaging: see
    /// app/PlancheContact/PublishWinUIResources.targets.
    fn check_folder(dist: &Path) -> Result<(), String> {
        let resource_index = Path::new(plan::APP_EXE).with_extension("pri");
        let needed = [
            (PathBuf::from(plan::APP_EXE), "the window"),
            (
                resource_index,
                "the resource index of the window, without which it cannot start \
                 (see app/PlancheContact/PublishWinUIResources.targets)",
            ),
            (PathBuf::from(ENGINE_EXE), "the engine"),
            (PathBuf::from(plan::VERB_EXE), "the menu handler"),
        ];
        for (file, role) in needed {
            if !dist.join(&file).is_file() {
                return Err(format!(
                    "{} does not hold {}: {role}",
                    dist.display(),
                    file.display()
                ));
            }
        }
        Ok(())
    }

    /// What File Explorer does on "Open in Planche-Contact": obtains the handler from
    /// COM, gives it the selection, executes the command.
    fn invoke_menu_handler(folder: &Path) -> Result<(), String> {
        // SAFETY: straightforward COM client sequence; the item identifier
        // list is freed after use.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| format!("COM initialization: {e}"))?;
            let result = (|| {
                let id = ILCreateFromPathW(&HSTRING::from(folder.as_os_str()));
                if id.is_null() {
                    return Err(format!("the shell does not know {}", folder.display()));
                }
                let ids = [id as *const ITEMIDLIST];
                let selection = SHCreateShellItemArrayFromIDLists(&ids);
                ILFree(Some(id));
                let selection = selection.map_err(|e| format!("building the selection: {e}"))?;

                let command: IExecuteCommand = CoCreateInstance(&CLSID, None, CLSCTX_LOCAL_SERVER)
                    .map_err(|e| {
                        format!("obtaining the menu handler from COM (is it registered?): {e}")
                    })?;
                command
                    .cast::<IObjectWithSelection>()
                    .map_err(|e| format!("the handler does not accept a selection: {e}"))?
                    .SetSelection(&selection)
                    .map_err(|e| format!("passing the selection: {e}"))?;
                command
                    .Execute()
                    .map_err(|e| format!("executing the command: {e}"))
            })();
            CoUninitialize();
            result
        }
    }

    pub fn run() -> Result<(), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let dist = match args.as_slice() {
            [] => None,
            // An absolute path in its usual form (C:\…), fit for the registry.
            [flag, dir] if flag == "--dist" => {
                Some(std::path::absolute(dir).map_err(|e| format!("{dir}: {e}"))?)
            }
            _ => return Err("usage: selftest [--dist DIR]".to_string()),
        };

        if app_is_running() {
            return Err(format!(
                "{} is already running: close it first",
                plan::APP_EXE
            ));
        }
        let _ = std::fs::remove_file(crash_log());

        // Something to open: a folder with one tiny image.
        let folder = std::env::temp_dir().join(format!("pc-selftest-{}", std::process::id()));
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        std::fs::write(folder.join("pixel.bmp"), PIXEL_BMP).map_err(|e| e.to_string())?;

        let outcome = (|| {
            if let Some(dist) = &dist {
                // 1. The folder itself.
                check_folder(dist)?;
                println!("ok: the folder holds the three programs and the window's resources");

                // 2. The window, started directly.
                let app = dist.join(plan::APP_EXE);
                let mut child = Command::new(&app)
                    .arg(&folder)
                    .spawn()
                    .map_err(|e| format!("could not start {}: {e}", app.display()))?;
                let direct = expect_window_up("started directly", Some(&mut child));
                close_app(Some(&mut child));
                direct?;
                println!("ok: the window starts and stays up");

                // 3. The handler of DIR, registered machine-wide for the test.
                // SAFETY: plain call without arguments.
                if !unsafe { IsUserAnAdmin() }.as_bool() {
                    return Err(
                        "--dist needs administrator rights (see the top of selftest.rs)".into(),
                    );
                }
                let server = format!("\"{}\"", dist.join(plan::VERB_EXE).display());
                let key = format!(r"{}\LocalServer32", machine_class_key());
                let added = output_of("reg", &["add", &key, "/ve", "/d", &server, "/f"])?;
                println!("registered for the test: {}", added.trim());
            }

            // 4. The whole chain: menu handler → application window.
            let through_handler = invoke_menu_handler(&folder)
                .and_then(|()| expect_window_up("through the menu handler", None));
            close_app(None);
            through_handler?;
            println!("ok: the menu handler opens the window on the selection");
            Ok(())
        })();

        if dist.is_some() {
            let _ = output_of("reg", &["delete", &machine_class_key(), "/f"]);
        }
        let _ = std::fs::remove_dir_all(&folder);
        outcome
    }
}

#[cfg(windows)]
fn main() {
    match windows_only::run() {
        Ok(()) => println!("All checks passed."),
        Err(message) => {
            eprintln!("FAILED: {message}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("selftest is a Windows program; build it for a Windows target.");
}
