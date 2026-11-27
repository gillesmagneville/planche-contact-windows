// SPDX-License-Identifier: GPL-3.0-only

using System.Globalization;
using System.Text;

namespace PlancheContact.Core.Localization;

/// <summary>
/// Translates the keys of <c>locales/</c> into the language of the
/// interface, with the same rules as the engine (engine/src/i18n.rs): a key missing
/// in the language falls back to English, then to the key itself; the
/// <c>{name}</c> fields are filled in as Python's <c>str.format</c> would.
/// </summary>
public sealed class Translator
{
    /// <summary>The choices of the language preference, in display order.</summary>
    public static readonly IReadOnlyList<string> Preferences = ["system", "en", "fr", "de", "es"];

    /// <summary>Languages that have a dictionary.</summary>
    public static readonly IReadOnlyList<string> Languages = ["en", "fr", "de", "es"];

    public const string FallbackLanguage = "en";

    private static readonly Lazy<Strings> English = new(() =>
        Strings.Load(FallbackLanguage) ?? throw new InvalidOperationException("the English texts are missing"));

    private readonly Strings _strings;

    private Translator(Strings strings) => _strings = strings;

    /// <summary>Language actually used ("en", "fr", "de" or "es").</summary>
    public string Language => _strings.Language;

    /// <summary>Translator for a supported language code; English otherwise.</summary>
    public static Translator For(string language) =>
        new(Languages.Contains(language) ? Strings.Load(language) ?? English.Value : English.Value);

    /// <summary>
    /// The language to use for a saved preference ("system", "en"…) — the
    /// counterpart of <c>i18n.resolve_language()</c>. When "system" designates
    /// a language without translation, English is used and
    /// <c>UnsupportedDetected</c> holds the detected code ("?" if none), so
    /// that the user can be told.
    /// </summary>
    public static (string Language, string? UnsupportedDetected) Resolve(string? preference, CultureInfo? systemCulture = null)
    {
        if (preference is not null && preference != "system" && Languages.Contains(preference))
        {
            return (preference, null);
        }
        // "system", or a saved value this version does not know.
        var culture = systemCulture ?? CultureInfo.CurrentUICulture;
        var detected = culture.TwoLetterISOLanguageName.ToLowerInvariant();
        if (Languages.Contains(detected))
        {
            return (detected, null);
        }
        return (FallbackLanguage, string.IsNullOrEmpty(detected) || detected == "iv" ? "?" : detected);
    }

    /// <summary>Text of <paramref name="key"/> in the language of the interface.</summary>
    public string T(string key) => _strings.Find(key) ?? English.Value.Find(key) ?? key;

    /// <summary>Text of <paramref name="key"/> with its <c>{name}</c> fields filled in.</summary>
    public string T(string key, params (string Name, object? Value)[] fields) => Format(T(key), fields);

    /// <summary>
    /// Replaces each <c>{name}</c> of <paramref name="template"/> by the value
    /// of the field of that name (<c>{{</c> and <c>}}</c> give a brace). As in
    /// <c>i18n._()</c>, a template naming a field that is not given is
    /// returned as it is rather than failing.
    /// </summary>
    public static string Format(string template, params (string Name, object? Value)[] fields)
    {
        if (fields.Length == 0 || !template.Contains('{'))
        {
            return template;
        }
        var result = new StringBuilder(template.Length + 32);
        for (var i = 0; i < template.Length; i++)
        {
            var c = template[i];
            if (c == '{' && i + 1 < template.Length && template[i + 1] == '{')
            {
                result.Append('{');
                i++;
            }
            else if (c == '}' && i + 1 < template.Length && template[i + 1] == '}')
            {
                result.Append('}');
                i++;
            }
            else if (c == '{')
            {
                var close = template.IndexOf('}', i + 1);
                if (close < 0)
                {
                    return template;
                }
                var name = template.AsSpan(i + 1, close - i - 1);
                var found = false;
                foreach (var (fieldName, value) in fields)
                {
                    if (name.SequenceEqual(fieldName))
                    {
                        result.Append(Convert.ToString(value, CultureInfo.InvariantCulture));
                        found = true;
                        break;
                    }
                }
                if (!found)
                {
                    return template;
                }
                i = close;
            }
            else
            {
                result.Append(c);
            }
        }
        return result.ToString();
    }
}
