// SPDX-License-Identifier: GPL-3.0-only

using System.Diagnostics;
using System.Globalization;
using System.Text.Json;
using System.Text.RegularExpressions;
using PlancheContact.Core.Localization;

namespace PlancheContact.Core.Tests;

internal static partial class LocalizationTests
{
    [Test]
    private static void Every_locale_file_is_read_completely()
    {
        foreach (var language in Translator.Languages)
        {
            foreach (var file in new[] { Repo.File_("locales", $"{language}.py"), Repo.File_("locales", "windows", $"{language}.py") })
            {
                var source = File.ReadAllText(file);
                var strings = Strings.Parse(language, source, out var unread);
                Check.Sequence([], unread, $"lines of {file} the window cannot read");
                var declared = Regex.Matches(source, "^\\s*\"[^\"]+\"\\s*:", RegexOptions.Multiline).Count;
                Check.Equal(declared, strings.Keys.Count, $"entries read from {file}");
            }
        }
    }

    [Test]
    private static void Embedded_texts_are_those_python_reads()
    {
        var python = PythonTexts();
        foreach (var language in Translator.Languages)
        {
            var strings = Strings.Load(language)!;
            var expected = python[language];
            Check.Equal(expected.Count, strings.Keys.Count, $"number of texts in {language}");
            foreach (var (key, value) in expected)
            {
                Check.Equal(value, strings.Find(key), $"{language}: {key}");
            }
        }
    }

    [Test]
    private static void Windows_texts_are_apart_and_complete()
    {
        var englishWindows = Strings.Parse("en", File.ReadAllText(Repo.File_("locales", "windows", "en.py")), out _).Keys
            .Order(StringComparer.Ordinal).ToList();
        Check.True(englishWindows.All(key => key.StartsWith("win.", StringComparison.Ordinal)), "only win. keys in locales/windows/");
        foreach (var language in Translator.Languages)
        {
            var shared = Strings.Parse(language, File.ReadAllText(Repo.File_("locales", $"{language}.py")), out _);
            Check.True(shared.Keys.All(key => !key.StartsWith("win.", StringComparison.Ordinal)),
                $"locales/{language}.py stays that of the Linux version (no win. key)");
            var windows = Strings.Parse(language, File.ReadAllText(Repo.File_("locales", "windows", $"{language}.py")), out _);
            Check.Sequence(englishWindows, windows.Keys.Order(StringComparer.Ordinal), $"keys of locales/windows/{language}.py");
        }
    }

    [Test]
    private static void Every_key_used_by_the_window_exists_in_every_language()
    {
        var keys = UsedKeys();
        Check.True(keys.Count > 100, $"the window should use many texts, found {keys.Count}");
        foreach (var language in Translator.Languages)
        {
            var strings = Strings.Load(language)!;
            var missing = keys.Where(key => strings.Find(key) is null).ToList();
            Check.Sequence([], missing, $"keys missing in {language}");
        }
    }

    [Test]
    private static void Windows_only_texts_keep_their_placeholders_in_every_language()
    {
        var english = Strings.Load("en")!;
        foreach (var key in english.Keys.Where(key => key.StartsWith("win.", StringComparison.Ordinal)))
        {
            var fields = Placeholders(english.Find(key)!);
            foreach (var language in Translator.Languages)
            {
                Check.Sequence(fields, Placeholders(Strings.Load(language)!.Find(key) ?? ""), $"{language}: {key}");
            }
        }
    }

    [Test]
    private static void Missing_texts_fall_back_to_english_then_to_the_key()
    {
        var french = Translator.For("fr");
        Check.Equal("Génération", french.T("tab.generation"));
        Check.Equal("no.such.key", french.T("no.such.key"));
        Check.Equal("Generation", Translator.For("ja").T("tab.generation"), "an unknown language is English");
    }

    [Test]
    private static void Fields_are_filled_in_like_python_format()
    {
        Check.Equal("3 of 5 folder(s)", Translator.For("en").T("gen.subfolders_selected", ("selected", 3), ("total", 5)));
        Check.Equal("a {b} c", Translator.Format("a {{b}} c", ("b", 1)));
        Check.Equal("x {missing} y", Translator.Format("x {missing} y", ("other", 1)), "an unknown field leaves the text as it is");
        Check.Equal("1.5", Translator.Format("{v}", ("v", 1.5)), "numbers are not written in the user's culture");
    }

    [Test]
    private static void System_language_is_resolved_like_i18n()
    {
        Check.Equal(("de", (string?)null), Translator.Resolve("de"));
        Check.Equal(("fr", (string?)null), Translator.Resolve("system", new CultureInfo("fr-CA")));
        Check.Equal(("en", (string?)"ja"), Translator.Resolve("system", new CultureInfo("ja-JP")));
        Check.Equal(("es", (string?)null), Translator.Resolve("klingon", new CultureInfo("es-ES")), "an unknown preference is the system language");
        Check.Equal(("en", (string?)"?"), Translator.Resolve("system", CultureInfo.InvariantCulture));
        Check.Equal(("fr", (string?)null), Translator.Resolve("system", new CultureInfo("fr-FR"), new CultureInfo("en-US")),
            "a French regional format wins over an English display language");
        Check.Equal(("de", (string?)null), Translator.Resolve("system", new CultureInfo("ja-JP"), new CultureInfo("de-DE")),
            "the display language is used when the regional format has no translation");
        Check.Equal(("en", (string?)"ja"), Translator.Resolve("system", new CultureInfo("ja-JP"), new CultureInfo("ko-KR")));
    }

    [Test]
    private static void Python_escapes_are_resolved()
    {
        Check.Equal("a\nb \"c\" \\ é", Strings.Unescape("a\\nb \\\"c\\\" \\\\ \\u00e9"));
    }

    // ------------------------------------------------------------- helpers

    private static List<string> Placeholders(string text) =>
        Regex.Matches(text, "\\{([a-z_]+)\\}").Select(m => m.Groups[1].Value).Order(StringComparer.Ordinal).ToList();

    /// <summary>
    /// The keys the window's code names: _t.T("key"…) and the constants of
    /// WinStrings, plus the keys built from a choice list ("adv.template." +
    /// value…), which are listed in Window/Keys.cs.
    /// </summary>
    private static HashSet<string> UsedKeys()
    {
        var keys = new HashSet<string>(StringComparer.Ordinal);
        var sources = Directory.EnumerateFiles(Repo.File_("app"), "*.cs", SearchOption.AllDirectories)
            .Where(path => !path.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}", StringComparison.Ordinal)
                && !path.Contains($"{Path.DirectorySeparatorChar}bin{Path.DirectorySeparatorChar}", StringComparison.Ordinal)
                && !path.Contains(".Tests", StringComparison.Ordinal));
        foreach (var path in sources)
        {
            var text = File.ReadAllText(path);
            foreach (Match match in KeyLiteral().Matches(text))
            {
                keys.Add(match.Groups[1].Value);
            }
        }
        foreach (var value in Choices.LayoutTemplates) keys.Add($"adv.template.{value}");
        foreach (var value in Choices.Orientations) keys.Add($"adv.orientation.{value}");
        foreach (var value in Choices.FitModes) keys.Add($"adv.fit_mode.{value}");
        foreach (var value in Choices.HeaderPositions) keys.Add($"adv.header_position.{value}");
        foreach (var value in Choices.CaptionPositions) keys.Add($"adv.caption_position.{value}");
        foreach (var value in Choices.CaptionAlignments) keys.Add($"adv.caption_alignment.{value}");
        foreach (var value in Choices.CaptionFields) keys.Add($"adv.field.{value}");
        foreach (var value in Choices.ThemeModes) keys.Add($"gen.theme_mode.{value}");
        foreach (var (_, key) in Choices.WatermarkOrientations) keys.Add(key);
        foreach (var value in Translator.Preferences) keys.Add($"lang.{value}");
        return keys;
    }

    // A string literal that looks like a translation key: section.name, lower case.
    [GeneratedRegex("\"((?:tab|gen|launch|subfolders|watermark|preview|folder_picker|dialog|help|about|lang|prefs|adv|win)\\.[a-z0-9_.]*[a-z0-9_])\"")]
    private static partial Regex KeyLiteral();

    /// <summary>The dictionaries as Python itself reads them (locales/xx.py, then locales/windows/xx.py).</summary>
    private static Dictionary<string, Dictionary<string, string>> PythonTexts()
    {
        const string script =
            "import json, os, runpy, sys\n" +
            "root = os.path.join(sys.argv[1], 'locales')\n" +
            "texts = {}\n" +
            "for code in ('en', 'fr', 'de', 'es'):\n" +
            "    texts[code] = dict(runpy.run_path(os.path.join(root, code + '.py'))['STRINGS'])\n" +
            "    texts[code].update(runpy.run_path(os.path.join(root, 'windows', code + '.py'))['STRINGS'])\n" +
            "sys.stdout.reconfigure(encoding='utf-8')\n" +
            "print(json.dumps(texts))\n";
        foreach (var python in new[] { Environment.GetEnvironmentVariable("PLANCHE_CONTACT_PYTHON"), "python3", "python" })
        {
            if (string.IsNullOrWhiteSpace(python))
            {
                continue;
            }
            try
            {
                var start = new ProcessStartInfo(python)
                {
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    StandardOutputEncoding = System.Text.Encoding.UTF8,
                    UseShellExecute = false,
                };
                start.ArgumentList.Add("-c");
                start.ArgumentList.Add(script);
                start.ArgumentList.Add(Repo.Root);
                using var process = Process.Start(start)!;
                var output = process.StandardOutput.ReadToEnd();
                process.WaitForExit();
                if (process.ExitCode == 0)
                {
                    return JsonSerializer.Deserialize<Dictionary<string, Dictionary<string, string>>>(output)!;
                }
            }
            catch (System.ComponentModel.Win32Exception)
            {
                // Not this one.
            }
        }
        throw new TestSkipped("no Python on the PATH");
    }
}
