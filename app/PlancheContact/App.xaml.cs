// SPDX-License-Identifier: GPL-3.0-only

using Microsoft.UI.Xaml;
using PlancheContact.Core;

namespace PlancheContact;

/// <summary>
/// Entry point of the application. The Explorer context menu starts it with
/// <c>--paths-from FILE</c> (see shell-verb/); the
/// Start menu shortcut starts it with nothing.
/// </summary>
public partial class App : Application
{
    private Window? _window;

    public App()
    {
        // Before anything else, so that an error at start-up leaves a trace
        // too. The two events cover different places: the first one the code
        // called by the window system, the second one everything else.
        UnhandledException += (_, e) => CrashLog.Write(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => CrashLog.Write(
            e.ExceptionObject as Exception ?? new InvalidOperationException(e.ExceptionObject?.ToString()));
        try
        {
            InitializeComponent();
        }
        catch (Exception exception)
        {
            CrashLog.Write(exception);
            throw;
        }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            // The first element is the path of the program itself.
            var arguments = AppArguments.Parse(Environment.GetCommandLineArgs().Skip(1));
            _window = new MainWindow(arguments);
            _window.Activate();
        }
        catch (Exception exception)
        {
            CrashLog.Write(exception);
            throw;
        }
    }
}

/// <summary>
/// Last-resort trace: an error nobody handled is appended to
/// <c>planche-contact-app.log</c> in the temporary folder of the user before
/// the application stops, so that a crash can be reported with something to
/// go on.
/// </summary>
internal static class CrashLog
{
    private static Exception? _lastWritten;

    public static void Write(Exception exception)
    {
        if (ReferenceEquals(exception, _lastWritten))
        {
            return;
        }
        _lastWritten = exception;
        try
        {
            var file = Path.Combine(Path.GetTempPath(), "planche-contact-app.log");
            File.AppendAllText(file, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {exception}{Environment.NewLine}");
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            // Nothing more can be done.
        }
    }
}
