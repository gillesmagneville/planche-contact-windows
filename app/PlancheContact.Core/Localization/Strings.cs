// SPDX-License-Identifier: GPL-3.0-only

using System.Globalization;
using System.Reflection;
using System.Text;
using System.Text.RegularExpressions;

namespace PlancheContact.Core.Localization;

/// <summary>
/// The texts of one language, read from the Python dictionary the engine and
/// the Linux version use (<c>locales/xx.py</c>, embedded in this library),
/// completed by the texts only this window uses
/// (<c>locales/windows/xx.py</c>, same shape). Linux and Windows thus share
/// one set of translations.
///
/// Only the shape of those files is understood: one <c>"key": "value",</c>
/// entry per line, double-quoted strings with the usual escapes, comments and
/// blank lines. A line of another shape inside the dictionary is reported by
/// <see cref="Parse"/>, so that a test fails rather than a text going missing.
/// </summary>
public sealed partial class Strings
{
    private readonly Dictionary<string, string> _texts;

    private Strings(string language, Dictionary<string, string> texts)
    {
        Language = language;
        _texts = texts;
    }

    /// <summary>Language code ("en", "fr"…).</summary>
    public string Language { get; }

    /// <summary>All the keys of the language.</summary>
    public IReadOnlyCollection<string> Keys => _texts.Keys;

    /// <summary>The text of <paramref name="key"/>, or null.</summary>
    public string? Find(string key) => _texts.TryGetValue(key, out var text) ? text : null;

    /// <summary>Reads the dictionary of <paramref name="language"/> embedded in this library.</summary>
    public static Strings? Load(string language)
    {
        var assembly = typeof(Strings).GetTypeInfo().Assembly;
        var shared = ReadResource(assembly, $"locales/{language}.py");
        if (shared is null)
        {
            return null;
        }
        var strings = Parse(language, shared, out _);
        if (ReadResource(assembly, $"locales/windows/{language}.py") is { } windows)
        {
            foreach (var (key, text) in Parse(language, windows, out _)._texts)
            {
                strings._texts[key] = text;
            }
        }
        return strings;
    }

    private static string? ReadResource(Assembly assembly, string name)
    {
        using var stream = assembly.GetManifestResourceStream(name);
        if (stream is null)
        {
            return null;
        }
        using var reader = new StreamReader(stream, Encoding.UTF8);
        return reader.ReadToEnd();
    }

    /// <summary>
    /// Parses the text of a locale file. <paramref name="unreadLines"/> lists
    /// the lines inside the dictionary that are neither an entry, a comment
    /// nor blank (none in a well-formed file).
    /// </summary>
    public static Strings Parse(string language, string source, out List<string> unreadLines)
    {
        var texts = new Dictionary<string, string>(StringComparer.Ordinal);
        unreadLines = [];
        var inside = false;
        foreach (var rawLine in source.Split('\n'))
        {
            var line = rawLine.Trim();
            if (!inside)
            {
                inside = line.StartsWith("STRINGS = {", StringComparison.Ordinal);
                continue;
            }
            if (line == "}")
            {
                break;
            }
            if (line.Length == 0 || line[0] == '#')
            {
                continue;
            }
            var match = EntryPattern().Match(line);
            if (match.Success)
            {
                texts[Unescape(match.Groups["key"].Value)] = Unescape(match.Groups["value"].Value);
            }
            else
            {
                unreadLines.Add(line);
            }
        }
        return new Strings(language, texts);
    }

    [GeneratedRegex("""^"(?<key>(?:[^"\\]|\\.)*)"\s*:\s*"(?<value>(?:[^"\\]|\\.)*)"\s*,?\s*(?:#.*)?$""")]
    private static partial Regex EntryPattern();

    /// <summary>Content of a Python string literal, escapes resolved.</summary>
    public static string Unescape(string text)
    {
        if (!text.Contains('\\'))
        {
            return text;
        }
        var result = new StringBuilder(text.Length);
        for (var i = 0; i < text.Length; i++)
        {
            var c = text[i];
            if (c != '\\' || i + 1 >= text.Length)
            {
                result.Append(c);
                continue;
            }
            var next = text[++i];
            switch (next)
            {
                case 'n':
                    result.Append('\n');
                    break;
                case 't':
                    result.Append('\t');
                    break;
                case 'r':
                    result.Append('\r');
                    break;
                case 'u' when i + 4 < text.Length
                    && int.TryParse(text.AsSpan(i + 1, 4), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var code):
                    result.Append((char)code);
                    i += 4;
                    break;
                default:
                    // \\, \", \' and anything else: the character itself.
                    result.Append(next);
                    break;
            }
        }
        return result.ToString();
    }
}
