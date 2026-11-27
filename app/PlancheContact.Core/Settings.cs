// SPDX-License-Identifier: GPL-3.0-only

using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace PlancheContact.Core;

/// <summary>
/// What the window remembers between two sessions: the last folders and the
/// language. Same file and same keys as the former GTK version for Windows
/// (<c>%USERPROFILE%\.config\planche-contact\settings.json</c>), so that
/// someone coming from the former GTK version for Windows keeps their folders and language.
/// Keys this window does not know are kept as they are.
/// </summary>
public sealed class Settings
{
    private readonly JsonObject _json;

    private Settings(string path, JsonObject json)
    {
        FilePath = path;
        _json = json;
    }

    public string FilePath { get; }

    public static string DefaultPath() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".config", "planche-contact", "settings.json");

    public string LastInputDir
    {
        get => Get("last_input_dir");
        set => _json["last_input_dir"] = value;
    }

    public string LastOutputDir
    {
        get => Get("last_output_dir");
        set => _json["last_output_dir"] = value;
    }

    /// <summary>"system", "en", "fr", "de" or "es".</summary>
    public string Language
    {
        get
        {
            var value = Get("language");
            return Localization.Translator.Preferences.Contains(value) ? value : "system";
        }
        set => _json["language"] = value;
    }

    private string Get(string key) =>
        _json[key] is JsonValue value && value.TryGetValue<string>(out var text) ? text : "";

    /// <summary>The settings of <paramref name="path"/>; an unreadable file gives empty settings.</summary>
    public static Settings Load(string? path = null)
    {
        path ??= DefaultPath();
        try
        {
            if (JsonNode.Parse(File.ReadAllText(path, Encoding.UTF8)) is JsonObject json)
            {
                return new Settings(path, json);
            }
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException)
        {
            // Nothing remembered.
        }
        return new Settings(path, []);
    }

    /// <summary>Writes the settings; false when the file cannot be written (nothing else to do).</summary>
    public bool Save()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            File.WriteAllText(FilePath, _json.ToJsonString(new JsonSerializerOptions { WriteIndented = true }),
                new UTF8Encoding(false));
            return true;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }
}
