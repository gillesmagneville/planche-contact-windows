// SPDX-License-Identifier: GPL-3.0-only

using System.Diagnostics;
using System.Reflection;
using PlancheContact.Core.Engine;

namespace PlancheContact.Core.Tests;

/// <summary>Marks a static method without parameters as a test.</summary>
[AttributeUsage(AttributeTargets.Method)]
internal sealed class TestAttribute : Attribute;

/// <summary>Thrown by a failed check.</summary>
internal sealed class TestFailure(string message) : Exception(message);

/// <summary>Thrown by a test that cannot run here (reported, not a failure).</summary>
internal sealed class TestSkipped(string reason) : Exception(reason);

internal static class Check
{
    public static void True(bool condition, string what)
    {
        if (!condition)
        {
            throw new TestFailure(what);
        }
    }

    public static void Equal<T>(T expected, T actual, string? what = null)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
        {
            throw new TestFailure($"{what ?? "values differ"}\n      expected: {Show(expected)}\n      actual:   {Show(actual)}");
        }
    }

    public static void Sequence<T>(IEnumerable<T> expected, IEnumerable<T> actual, string? what = null)
    {
        var (e, a) = (expected.ToList(), actual.ToList());
        if (!e.SequenceEqual(a))
        {
            throw new TestFailure(
                $"{what ?? "sequences differ"}\n      expected: [{string.Join(", ", e.Select(Show))}]\n      actual:   [{string.Join(", ", a.Select(Show))}]");
        }
    }

    private static string Show<T>(T value) => value switch
    {
        null => "null",
        string s => "\"" + s.Replace("\n", "\\n").Replace("\r", "\\r") + "\"",
        _ => value.ToString() ?? "",
    };
}

/// <summary>Where things are in the repository.</summary>
internal static class Repo
{
    private static readonly Lazy<string> LazyRoot = new(() =>
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "VERSION")) && Directory.Exists(Path.Combine(dir.FullName, "locales")))
            {
                return dir.FullName;
            }
        }
        throw new TestFailure("repository root not found (no VERSION file and locales/ folder above the test program)");
    });

    public static string Root => LazyRoot.Value;

    public static string File_(params string[] parts) => Path.Combine([Root, .. parts]);
}

/// <summary>
/// The engine the integration tests run: the one given by
/// PLANCHE_CONTACT_ENGINE, otherwise the one built in target/release (or
/// target/debug) of the repository by <c>cargo build</c>. Without one, those
/// tests are skipped, unless PLANCHE_CONTACT_REQUIRE_ENGINE=1, which makes
/// them fail (continuous integration).
/// </summary>
internal static class TestEngine
{
    private static readonly Lazy<EngineCommand?> LazyCommand = new(Find);

    public static EngineRunner Runner()
    {
        if (LazyCommand.Value is { } command)
        {
            return new EngineRunner(command);
        }
        const string reason = "no engine: set PLANCHE_CONTACT_ENGINE, or build it with cargo build --release";
        if (Environment.GetEnvironmentVariable("PLANCHE_CONTACT_REQUIRE_ENGINE") == "1")
        {
            throw new TestFailure(reason);
        }
        throw new TestSkipped(reason);
    }

    private static EngineCommand? Find()
    {
        if (!string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable(EngineCommand.EngineVariable)))
        {
            return EngineCommand.FromEnvironment(AppContext.BaseDirectory);
        }
        var name = OperatingSystem.IsWindows() ? "planche-contact-engine.exe" : "planche-contact-engine";
        return new[] { "release", "debug" }
            .Select(profile => Repo.File_("target", profile, name))
            .Where(File.Exists)
            .Select(path => new EngineCommand(path))
            .FirstOrDefault();
    }

    public static EngineResult Run(params string[] arguments) =>
        Runner().RunAsync(arguments).GetAwaiter().GetResult();
}

internal static class Program
{
    private static int Main(string[] args)
    {
        var tests = typeof(Program).Assembly.GetTypes()
            .SelectMany(type => type.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic))
            .Where(method => method.GetCustomAttribute<TestAttribute>() is not null)
            .Where(method => args.Length == 0 || args.Any(filter => $"{method.DeclaringType!.Name}.{method.Name}".Contains(filter, StringComparison.Ordinal)))
            .OrderBy(method => method.DeclaringType!.Name, StringComparer.Ordinal)
            .ThenBy(method => method.Name, StringComparer.Ordinal)
            .ToList();

        var (passed, failed, skipped) = (0, 0, 0);
        foreach (var test in tests)
        {
            var name = $"{test.DeclaringType!.Name}.{test.Name}";
            var watch = Stopwatch.StartNew();
            try
            {
                test.Invoke(null, null);
                passed++;
                Console.WriteLine($"ok    {name} ({watch.ElapsedMilliseconds} ms)");
            }
            catch (TargetInvocationException e) when (e.InnerException is TestSkipped skip)
            {
                skipped++;
                Console.WriteLine($"skip  {name}: {skip.Message}");
            }
            catch (TargetInvocationException e)
            {
                failed++;
                var cause = e.InnerException!;
                Console.WriteLine($"FAIL  {name}\n      {(cause is TestFailure ? cause.Message : cause.ToString())}");
            }
        }

        Console.WriteLine();
        Console.WriteLine($"{passed} passed, {failed} failed, {skipped} skipped");
        return failed == 0 ? 0 : 1;
    }
}
