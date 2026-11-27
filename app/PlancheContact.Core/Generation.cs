// SPDX-License-Identifier: GPL-3.0-only

using System.Globalization;
using System.Text;

namespace PlancheContact.Core;

/// <summary>
/// Everything the form asks for: what the Linux window reads from its widgets
/// in <c>_on_generate()</c> and <c>_on_preview_clicked()</c>. Turned into
/// the arguments of the engine's <c>generate</c> command by
/// <see cref="GenerateArguments"/> and <see cref="PreviewArguments"/>, with
/// exactly the same rules as the Linux window.
/// </summary>
public sealed record GenerationRequest
{
    public required string InputDir { get; init; }

    /// <summary>Empty: <c>Portfolio</c> in the input folder.</summary>
    public string OutputDir { get; init; } = "";

    public int PerSheet { get; init; } = Choices.DefaultPerSheet;
    public string PageFormat { get; init; } = Choices.DefaultPageFormat;
    public string Title { get; init; } = "";
    public string Author { get; init; } = "";
    public string Watermark { get; init; } = "";
    public string WatermarkOrientation { get; init; } = "Horizontal";
    public int WatermarkOpacity { get; init; } = Choices.DefaultWatermarkOpacity;
    public bool Recursive { get; init; }

    /// <summary>Subfolders to include (relative, "." = the input folder), or null for all.</summary>
    public IReadOnlyList<string>? IncludeDirs { get; init; }

    /// <summary>Photos received from a context menu (relative to the input folder), or null for the whole folder.</summary>
    public IReadOnlyList<string>? Files { get; init; }

    public bool Pdf { get; init; } = true;
    public bool Html { get; init; } = true;
    public bool HtmlOriginals { get; init; }
    public bool Csv { get; init; } = true;
    public int HtmlPerPage { get; init; } = Choices.DefaultHtmlPerPage;

    /// <summary>Language already resolved ("fr"…), never "system".</summary>
    public string Language { get; init; } = "en";

    public Profile Profile { get; init; } = Profile.InterfaceDefault();

    /// <summary>The output folder actually used.</summary>
    public string EffectiveOutputDir =>
        OutputDir.Trim().Length > 0 ? OutputDir.Trim() : Path.Combine(InputDir.Trim(), "Portfolio");

    /// <summary>A selection of photos is processed as it is: no recursive search, no subfolder choice.</summary>
    public bool EffectiveRecursive => Recursive && Files is null;

    public IReadOnlyList<string>? EffectiveIncludeDirs => EffectiveRecursive ? IncludeDirs : null;

    /// <summary>Arguments of a generation (after the engine's <c>generate</c> command).</summary>
    public List<string> GenerateArguments(GenerationFiles files)
    {
        var arguments = new List<string>
        {
            "-i", InputDir.Trim(),
            "-o", EffectiveOutputDir,
            "-n", PerSheet.ToString(CultureInfo.InvariantCulture),
            "--format", PageFormat,
            "--html-per-page", HtmlPerPage.ToString(CultureInfo.InvariantCulture),
            "--language", Language,
        };
        AddCommon(arguments, files);
        if (Pdf)
        {
            arguments.Add("--pdf");
        }
        if (Html)
        {
            arguments.Add("--html");
            if (HtmlOriginals)
            {
                arguments.Add("--html-originals");
            }
        }
        if (Csv)
        {
            arguments.Add("--csv");
        }
        if (files.ProfileFile is { } profile)
        {
            arguments.AddRange(["--profile", profile]);
        }
        return arguments;
    }

    /// <summary>Arguments of the preview of the first sheet, written to <paramref name="previewPng"/>.</summary>
    public List<string> PreviewArguments(GenerationFiles files, string previewPng)
    {
        var arguments = new List<string>
        {
            "-i", InputDir.Trim(),
            "-n", PerSheet.ToString(CultureInfo.InvariantCulture),
            "--format", PageFormat,
            "--language", Language,
            "--preview-output", previewPng,
        };
        AddCommon(arguments, files);
        if (files.ProfileFile is { } profile)
        {
            arguments.AddRange(["--profile", profile]);
        }
        return arguments;
    }

    private void AddCommon(List<string> arguments, GenerationFiles files)
    {
        if (Title.Trim().Length > 0)
        {
            arguments.AddRange(["--title", Title.Trim()]);
        }
        if (Author.Trim().Length > 0)
        {
            arguments.AddRange(["--author", Author.Trim()]);
        }
        if (Watermark.Trim().Length > 0)
        {
            arguments.AddRange(["--watermark", Watermark.Trim()]);
            arguments.AddRange(["--watermark-orientation", WatermarkOrientation]);
            arguments.AddRange(["--watermark-opacity", WatermarkOpacity.ToString(CultureInfo.InvariantCulture)]);
        }
        if (EffectiveRecursive)
        {
            arguments.Add("-r");
        }
        if (files.IncludeDirsFile is { } include)
        {
            arguments.AddRange(["--include-dirs-file", include]);
        }
        if (files.FilesFromFile is { } list)
        {
            arguments.AddRange(["--files-from", list]);
        }
    }
}

/// <summary>
/// The temporary files a generation hands to the engine: the subfolder list,
/// the photo list and the profile — each only when needed, as in the GTK
/// window. Deleted by <see cref="Dispose"/>, once the engine has finished.
/// </summary>
public sealed class GenerationFiles : IDisposable
{
    private GenerationFiles(string? includeDirsFile, string? filesFromFile, string? profileFile)
    {
        IncludeDirsFile = includeDirsFile;
        FilesFromFile = filesFromFile;
        ProfileFile = profileFile;
    }

    public string? IncludeDirsFile { get; }
    public string? FilesFromFile { get; }
    public string? ProfileFile { get; }

    public static GenerationFiles Write(GenerationRequest request)
    {
        string? include = null, list = null, profile = null;
        try
        {
            if (request.EffectiveIncludeDirs is { } dirs)
            {
                include = TempFile.WriteLines("planche-contact-dirs-", dirs);
            }
            if (request.Files is { } photos)
            {
                list = TempFile.WriteLines("planche-contact-files-", photos);
            }
            // Only when something was customised: never change what someone
            // who never touches these settings gets (see Profile.Default).
            if (!request.Profile.SameAs(Profile.Default()))
            {
                profile = TempFile.Write("planche-contact-profile-", ".json", request.Profile.ToJson());
            }
            return new GenerationFiles(include, list, profile);
        }
        catch
        {
            TempFile.Delete(include, list, profile);
            throw;
        }
    }

    public void Dispose() => TempFile.Delete(IncludeDirsFile, FilesFromFile, ProfileFile);
}

/// <summary>Temporary files, in the temporary folder of the user, UTF-8 without byte order mark.</summary>
public static class TempFile
{
    private static readonly UTF8Encoding Utf8 = new(encoderShouldEmitUTF8Identifier: false);

    public static string Write(string prefix, string suffix, string content)
    {
        var path = Path.Combine(Path.GetTempPath(), $"{prefix}{Environment.ProcessId}-{Guid.NewGuid():N}{suffix}");
        File.WriteAllText(path, content, Utf8);
        return path;
    }

    /// <summary>One line per item, each ended by "\n" (what the engine reads with splitlines()).</summary>
    public static string WriteLines(string prefix, IEnumerable<string> lines) =>
        Write(prefix, ".txt", string.Concat(lines.Select(line => line + "\n")));

    public static string NewPath(string prefix, string suffix) =>
        Path.Combine(Path.GetTempPath(), $"{prefix}{Environment.ProcessId}-{Guid.NewGuid():N}{suffix}");

    public static void Delete(params string?[] paths)
    {
        foreach (var path in paths)
        {
            if (path is null)
            {
                continue;
            }
            try
            {
                if (Directory.Exists(path))
                {
                    Directory.Delete(path, recursive: true);
                }
                else
                {
                    File.Delete(path);
                }
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                // Left in the temporary folder: harmless.
            }
        }
    }
}
