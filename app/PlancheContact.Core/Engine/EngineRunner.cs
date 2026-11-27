// SPDX-License-Identifier: GPL-3.0-only

using System.Diagnostics;
using System.Text;

namespace PlancheContact.Core.Engine;

/// <summary>The engine program could not be started.</summary>
public sealed class EngineStartException(string message, Exception inner) : Exception(message, inner);

/// <summary>
/// The engine program: <c>engine\planche-contact-engine.exe</c> next to the
/// window once installed. For development and tests, the
/// <c>PLANCHE_CONTACT_ENGINE</c> environment variable gives another path
/// (for example <c>target/release/planche-contact-engine</c> on Linux).
/// </summary>
public sealed record EngineCommand(string FileName)
{
    public const string EngineFolderName = "engine";
    public const string EngineFileName = "planche-contact-engine.exe";
    public const string EngineVariable = "PLANCHE_CONTACT_ENGINE";

    public static EngineCommand Default() => FromEnvironment(AppContext.BaseDirectory);

    public static EngineCommand FromEnvironment(string applicationDirectory)
    {
        var overridden = Environment.GetEnvironmentVariable(EngineVariable);
        return string.IsNullOrWhiteSpace(overridden)
            ? new EngineCommand(Path.Combine(applicationDirectory, EngineFolderName, EngineFileName))
            : new EngineCommand(overridden);
    }
}

/// <summary>
/// Runs the engine as a separate process and reports what it writes, line by
/// line, as it comes. All the work on photos happens in that process: the
/// window never decodes an image, and stopping a generation is stopping that
/// process.
/// </summary>
public sealed class EngineRunner(EngineCommand command)
{
    public EngineCommand Command { get; } = command;

    /// <summary>
    /// Starts the engine with <paramref name="arguments"/> (the command first).
    /// <paramref name="onOutput"/> receives each line of the standard output,
    /// <paramref name="onError"/> each line of the error output, on background
    /// threads: the caller switches to its own thread if it needs to.
    /// </summary>
    /// <exception cref="EngineStartException">The program could not be started.</exception>
    public EngineRun Start(IReadOnlyList<string> arguments, Action<string> onOutput, Action<string>? onError = null)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = Command.FileName,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            // The engine always writes UTF-8, whatever the code page of the
            // system (engine/src/events.rs).
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };
        foreach (var argument in arguments)
        {
            // ArgumentList quotes each argument as needed (spaces, quotes…).
            startInfo.ArgumentList.Add(argument);
        }

        var process = new Process { StartInfo = startInfo };
        try
        {
            process.Start();
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException
            or FileNotFoundException or DirectoryNotFoundException)
        {
            process.Dispose();
            throw new EngineStartException(e.Message, e);
        }
        return new EngineRun(process, onOutput, onError ?? (_ => { }));
    }

    /// <summary>
    /// Runs the engine to its end and returns everything it wrote. Cancelling
    /// stops the process.
    /// </summary>
    /// <exception cref="EngineStartException">The program could not be started.</exception>
    public async Task<EngineResult> RunAsync(IReadOnlyList<string> arguments, CancellationToken cancellation = default)
    {
        var output = new List<string>();
        var errors = new List<string>();
        var run = Start(arguments, line => { lock (output) { output.Add(line); } }, line => { lock (errors) { errors.Add(line); } });
        await using var registration = cancellation.Register(run.Kill);
        var code = await run.Completion.ConfigureAwait(false);
        cancellation.ThrowIfCancellationRequested();
        return new EngineResult(code, output, errors);
    }
}

/// <summary>What a finished engine wrote.</summary>
public sealed record EngineResult(int ExitCode, IReadOnlyList<string> Output, IReadOnlyList<string> Errors)
{
    /// <summary>The JSON events of the output, in order.</summary>
    public IReadOnlyList<EngineEvent> Events => Output.Select(EngineEvent.Parse).OfType<EngineEvent>().ToList();

    /// <summary>The error output as one text (the last lines matter most).</summary>
    public string ErrorText => string.Join(Environment.NewLine, Errors.Where(line => line.Length > 0)).Trim();
}

/// <summary>A running (or finished) engine process.</summary>
public sealed class EngineRun
{
    private readonly Process _process;

    internal EngineRun(Process process, Action<string> onOutput, Action<string> onError)
    {
        _process = process;
        Completion = Task.Run(() => PumpAsync(onOutput, onError));
    }

    /// <summary>
    /// Completes when the engine has exited and all its lines have been
    /// reported; the result is its exit code (0: success).
    /// </summary>
    public Task<int> Completion { get; }

    /// <summary>True once <see cref="Kill"/> has been called.</summary>
    public bool Killed { get; private set; }

    /// <summary>
    /// Stops the engine. The files already written stay on disk.
    /// </summary>
    public void Kill()
    {
        Killed = true;
        try
        {
            if (!_process.HasExited)
            {
                _process.Kill(entireProcessTree: true);
            }
        }
        catch (Exception e) when (e is InvalidOperationException or System.ComponentModel.Win32Exception
            or NotSupportedException)
        {
            // Already gone.
        }
    }

    private async Task<int> PumpAsync(Action<string> onOutput, Action<string> onError)
    {
        try
        {
            // Both streams read at once: an unread pipe would end up blocking
            // the engine.
            var errors = Task.Run(async () =>
            {
                while (await _process.StandardError.ReadLineAsync().ConfigureAwait(false) is { } line)
                {
                    onError(line);
                }
            });
            while (await _process.StandardOutput.ReadLineAsync().ConfigureAwait(false) is { } line)
            {
                onOutput(line);
            }
            await errors.ConfigureAwait(false);
            await _process.WaitForExitAsync().ConfigureAwait(false);
            return _process.ExitCode;
        }
        finally
        {
            _process.Dispose();
        }
    }
}
