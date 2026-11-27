// SPDX-License-Identifier: GPL-3.0-only

using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace PlancheContact.Core;

/// <summary>
/// The layout, captions and theme settings handed to the engine with
/// <c>--profile</c>: the same JSON as the Linux version
/// (<c>Profile.to_dict()</c>, engine/src/profile.rs here). The window only fills it in from its controls
/// and writes it; validating a profile read from a file is the engine's job
/// (<c>planche-contact-engine profile --load</c>), so that the rules stay in one place.
/// </summary>
public sealed class Profile
{
    public string? Name { get; set; }

    public LayoutSettings Layout { get; set; } = new();

    public CaptionSettings Captions { get; set; } = new();

    public ThemeSettings Theme { get; set; } = new();

    /// <summary>
    /// The profile of the command line without <c>--profile</c>
    /// (<c>default_profile()</c>): the window passes its profile only when it
    /// differs from this one, as the Linux window does.
    /// </summary>
    public static Profile Default() => new();

    /// <summary>
    /// What the window shows at startup and after "Reset"
    /// (<c>interface_default_profile()</c>): the file name below each photo.
    /// </summary>
    public static Profile InterfaceDefault() => new()
    {
        Layout = new LayoutSettings { CaptionPosition = "below" },
        Captions = new CaptionSettings { Fields = ["filename"] },
    };

    private static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        WriteIndented = true,
        // Accents of a profile name written as they are, as save_profile() does.
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
    };

    public string ToJson() => JsonSerializer.Serialize(this, Options) + "\n";

    public JsonObject ToJsonObject() => (JsonObject)JsonNode.Parse(ToJson())!;

    /// <summary>
    /// Reads a profile as the engine wrote it (already validated). A value of
    /// the wrong type is left at its default rather than failing.
    /// </summary>
    public static Profile FromJson(JsonObject json)
    {
        try
        {
            return json.Deserialize<Profile>(Options) ?? new Profile();
        }
        catch (JsonException)
        {
            return new Profile();
        }
    }

    /// <summary>Same settings (compared as the JSON the engine would read).</summary>
    public bool SameAs(Profile other) => JsonNode.DeepEquals(ToJsonObject(), other.ToJsonObject());
}

public sealed class LayoutSettings
{
    public string Template { get; set; } = "grid";
    public int? Columns { get; set; }
    public int? Rows { get; set; }
    public int PerPage { get; set; } = 12;
    public double MarginMm { get; set; } = 12.0;
    public double SpacingMm { get; set; } = 5.0;
    public string PageFormat { get; set; } = "A4";
    public string Orientation { get; set; } = "portrait";
    public string FitMode { get; set; } = "contain";
    public string TitlePosition { get; set; } = "top_center";
    public string AuthorPosition { get; set; } = "top_center";
    public bool PageNumbering { get; set; } = true;
    public string CaptionPosition { get; set; } = "none";
    public double CaptionAreaMm { get; set; } = 8.0;
}

public sealed class CaptionSettings
{
    public List<string> Fields { get; set; } = [];
    public Dictionary<string, string> FormatOverrides { get; set; } = [];
    public int Lines { get; set; } = 1;
    public int FontSize { get; set; } = 9;
    public string Alignment { get; set; } = "center";
    public string Separator { get; set; } = " · ";
    public bool ShowGps { get; set; }
}

public sealed class ThemeSettings
{
    public string Mode { get; set; } = "light";
    public ThemeCustomColors Custom { get; set; } = new();
}

public sealed partial class ThemeCustomColors
{
    public string? Background { get; set; }
    public string? Text { get; set; }
    public string? Accent { get; set; }
    public string? Border { get; set; }

    /// <summary>
    /// A colour as typed by the user: "#rrggbb", or null when the field is
    /// empty or invalid (the theme's own colour then applies, as in the GTK
    /// window). <paramref name="invalid"/> tells the window to flag the field.
    /// </summary>
    public static string? Parse(string? text, out bool invalid)
    {
        var value = (text ?? "").Trim();
        invalid = false;
        if (value.Length == 0)
        {
            return null;
        }
        if (HexColor().IsMatch(value))
        {
            return value;
        }
        invalid = true;
        return null;
    }

    [GeneratedRegex("^#[0-9a-fA-F]{6}$")]
    private static partial Regex HexColor();
}

/// <summary>
/// The internal values the engine understands, in the order the menus show
/// them, with the key of their text. Written here because the window needs
/// them before the engine has answered anything; a test compares them with
/// what the engine reports (<c>planche-contact-engine info</c>).
/// </summary>
public static class Choices
{
    public static readonly IReadOnlyList<string> PageFormats = ["A5", "A4", "A3", "A2", "Letter"];
    public const string DefaultPageFormat = "A4";

    /// <summary>Photos per sheet: 8 to 48 by 4, 12 by default.</summary>
    public static readonly IReadOnlyList<int> PerSheet = Enumerable.Range(0, 11).Select(i => 8 + 4 * i).ToList();
    public const int DefaultPerSheet = 12;

    /// <summary>Values of <c>--watermark-orientation</c> (compared as they are by config.py, hence in French).</summary>
    public static readonly IReadOnlyList<(string Value, string Key)> WatermarkOrientations =
    [
        ("Horizontal", "watermark.horizontal"),
        ("Diagonale horaire", "watermark.diagonal_cw"),
        ("Diagonale anti-horaire", "watermark.diagonal_ccw"),
    ];

    public static readonly IReadOnlyList<string> LayoutTemplates = ["grid", "landscape", "portrait", "large_thumbs", "free"];
    public static readonly IReadOnlyList<string> FitModes = ["contain", "cover"];
    public static readonly IReadOnlyList<string> Orientations = ["portrait", "landscape"];
    public static readonly IReadOnlyList<string> HeaderPositions = ["top_left", "top_center", "top_right", "none"];
    public static readonly IReadOnlyList<string> CaptionPositions = ["below", "above", "right", "none"];
    public static readonly IReadOnlyList<string> CaptionAlignments = ["left", "center", "right"];
    public static readonly IReadOnlyList<string> CaptionFields =
    [
        "filename", "datetime", "camera", "lens", "focal_length", "focal_35mm",
        "aperture", "shutter_speed", "iso", "dimensions", "gps",
    ];
    public static readonly IReadOnlyList<string> ThemeModes = ["light", "dark", "neutral", "system"];

    /// <summary>Images per page of the HTML gallery: 12 to 64 by 4, 48 by default.</summary>
    public const int HtmlPerPageMin = 12;
    public const int HtmlPerPageMax = 64;
    public const int HtmlPerPageStep = 4;
    public const int DefaultHtmlPerPage = 48;

    public const int DefaultWatermarkOpacity = 40;

    /// <summary>The nearest allowed value of the gallery slider (the Linux window rounds the same way).</summary>
    public static int SnapHtmlPerPage(double value)
    {
        var snapped = (int)Math.Round((value - HtmlPerPageMin) / HtmlPerPageStep, MidpointRounding.ToEven) * HtmlPerPageStep
            + HtmlPerPageMin;
        return Math.Clamp(snapped, HtmlPerPageMin, HtmlPerPageMax);
    }
}
