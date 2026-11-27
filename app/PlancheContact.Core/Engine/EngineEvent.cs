// SPDX-License-Identifier: GPL-3.0-only

using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace PlancheContact.Core.Engine;

/// <summary>
/// One event written by the engine: a line of JSON (engine/src/events.rs
/// and the commands of engine/src/main.rs). An event or a field this window
/// does not know is ignored.
/// </summary>
public abstract record EngineEvent
{
    /// <summary>The event of a line, or null for anything else (a log line, an unknown event).</summary>
    public static EngineEvent? Parse(string line)
    {
        var text = line.Trim();
        if (text.Length == 0 || text[0] != '{')
        {
            return null;
        }
        JsonObject? json;
        try
        {
            json = JsonNode.Parse(text) as JsonObject;
        }
        catch (JsonException)
        {
            return null;
        }
        if (json is null)
        {
            return null;
        }
        return Str(json, "event") switch
        {
            "photo" => new PhotoFound(Str(json, "path") ?? ""),
            "folder" => new FolderFound(Str(json, "path") ?? "", Int(json, "count")),
            "summary" => new Summary(Int(json, "total"), Int(json, "photos")),
            "selection" => new SelectionResolved(
                Str(json, "input_dir"),
                json["files"] is JsonArray files ? files.Select(f => f?.GetValue<string>() ?? "").ToList() : null),
            "thumbnail" => new ThumbnailMade(
                Int(json, "index"), Str(json, "input") ?? "", Str(json, "output"), Str(json, "error"),
                Int(json, "width"), Int(json, "height")),
            "profile" => new ProfileRead(json["profile"] as JsonObject ?? []),
            "info" => new EngineInfo(json),
            "fatal_error" => new FatalError(Str(json, "message") ?? ""),
            _ => null,
        };
    }

    private static string? Str(JsonObject json, string name) =>
        json[name] is JsonValue value && value.TryGetValue<string>(out var text) ? text : null;

    private static int Int(JsonObject json, string name) =>
        json[name] is JsonValue value && value.TryGetValue<int>(out var number) ? number : 0;
}

public sealed record PhotoFound(string Path) : EngineEvent;

public sealed record FolderFound(string RelativePath, int Count) : EngineEvent;

public sealed record Summary(int Total, int Photos) : EngineEvent;

/// <summary>What a context-menu selection designates: no input folder when nothing is usable.</summary>
public sealed record SelectionResolved(string? InputDir, IReadOnlyList<string>? Files) : EngineEvent;

public sealed record ThumbnailMade(int Index, string Input, string? Output, string? Error, int Width, int Height) : EngineEvent
{
    public bool Ok => Output is not null && Error is null;
}

public sealed record ProfileRead(JsonObject Profile) : EngineEvent;

public sealed record EngineInfo(JsonObject Json) : EngineEvent
{
    public string Version => Json["version"]?.GetValue<string>() ?? "";

    public IReadOnlyList<string> Choice(string name) =>
        Json["choices"]?[name] is JsonArray values ? values.Select(v => v!.GetValue<string>()).ToList() : [];
}

public sealed record FatalError(string Message) : EngineEvent;

/// <summary>
/// The lines of a generation (the engine's <c>generate</c> command): <c>PROGRESS:X/100 text</c>
/// gives the progress, <c>PREVIEW:shown/total</c> ends a preview. Both
/// markers are never translated; everything else is a message for the log.
/// </summary>
public static partial class GenerationLine
{
    /// <summary>Progress in percent and the text after it (possibly empty), or null.</summary>
    public static (int Percent, string Message)? Progress(string line)
    {
        var match = ProgressPattern().Match(line);
        if (!match.Success)
        {
            return null;
        }
        var percent = int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture);
        return (Math.Clamp(percent, 0, 100), match.Groups[2].Value.Trim());
    }

    /// <summary>Photos shown on the preview sheet and photos selected, or null.</summary>
    public static (int Shown, int Total)? Preview(string line)
    {
        var match = PreviewPattern().Match(line.Trim());
        if (!match.Success)
        {
            return null;
        }
        return (int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture),
            int.Parse(match.Groups[2].Value, CultureInfo.InvariantCulture));
    }

    // Searched anywhere in the line, as the Linux window does.
    [GeneratedRegex(@"PROGRESS:(\d+)/100(.*)$")]
    private static partial Regex ProgressPattern();

    [GeneratedRegex(@"^PREVIEW:(\d+)/(\d+)$")]
    private static partial Regex PreviewPattern();
}
