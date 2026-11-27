// SPDX-License-Identifier: GPL-3.0-only

//! Development tool: does what File Explorer does when "Open in Planche-Contact" is
//! chosen on a selection — without File Explorer.
//!
//!   simulate_explorer.exe PATH [PATH...]
//!
//! It asks COM for the registered handler (which starts
//! `planche-contact-verb.exe -Embedding`), gives it the paths as one
//! selection, and executes the command. The handler must have been registered
//! first (`planche-contact-verb.exe --register`).
//!
//! Handy to test the whole chain on a real Windows, and under Wine on Linux.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use planche_contact_verb::com::CLSID;
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IExecuteCommand, ILCreateFromPathW, ILFree, IObjectWithSelection,
        SHCreateShellItemArrayFromIDLists,
    };

    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: simulate_explorer PATH [PATH...]");
        std::process::exit(2);
    }

    // SAFETY: straightforward COM client sequence; the item identifier lists
    // are freed after use.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        {
            let mut ids: Vec<*const ITEMIDLIST> = Vec::new();
            for path in &paths {
                let id = ILCreateFromPathW(&HSTRING::from(path.as_str()));
                if id.is_null() {
                    eprintln!("skipped (not found): {path}");
                } else {
                    ids.push(id);
                }
            }
            let selection = SHCreateShellItemArrayFromIDLists(&ids)?;

            let command: IExecuteCommand = CoCreateInstance(&CLSID, None, CLSCTX_LOCAL_SERVER)?;
            command
                .cast::<IObjectWithSelection>()?
                .SetSelection(&selection)?;
            command.Execute()?;
            println!("command executed on {} item(s)", ids.len());

            for id in ids {
                ILFree(Some(id));
            }
        }
        CoUninitialize();
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("simulate_explorer is a Windows program; build it for a Windows target.");
}
