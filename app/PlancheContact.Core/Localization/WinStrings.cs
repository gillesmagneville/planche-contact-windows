// SPDX-License-Identifier: GPL-3.0-only

namespace PlancheContact.Core.Localization;

/// <summary>
/// Keys of the texts that only the Windows window uses (the "win." keys of
/// <c>locales/windows/</c>). Every other text is shared, word for word,
/// with the Linux window: their keys are written directly where they are used.
/// The tests check that each key used by the window exists in every language.
/// </summary>
public static class WinStrings
{
    public const string EngineNotStarted = "win.engine_not_started";
    public const string EngineError = "win.engine_error";
    public const string QuitRunningTitle = "win.quit_running_title";
    public const string QuitRunningDetail = "win.quit_running_detail";
    public const string QuitAnyway = "win.quit_anyway";
    public const string Cancel = "win.cancel";
    public const string Ok = "win.ok";
    public const string Cancelled = "win.cancelled";
    public const string CancelGeneration = "win.cancel_generation";
}
