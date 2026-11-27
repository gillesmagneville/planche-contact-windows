// SPDX-License-Identifier: GPL-3.0-only

using System.Globalization;

namespace PlancheContact.Core;

/// <summary>What the window was started with.</summary>
/// <param name="Arguments">
/// The arguments as given. The Explorer context menu passes
/// <c>--paths-from FILE</c> (see shell-verb/);
/// paths may also be given directly. They are handed as they are to the
/// engine's <c>selection</c> command, which reads them with the rules of
/// engine/src/launch.rs (and deletes the list file of the menu).
/// </param>
public sealed record AppArguments(IReadOnlyList<string> Arguments)
{
    public static AppArguments Parse(IEnumerable<string> arguments) =>
        new(arguments.Where(argument => argument.Length > 0).ToList());

    /// <summary>True when the window was asked to open a folder or photos.</summary>
    public bool HasPaths => Arguments.Any(argument => argument != "--");
}

/// <summary>Page buttons of the photo preview.</summary>
public static class Pagination
{
    /// <summary>Marker of a gap in the sequence ("…").</summary>
    public const int Gap = 0;

    /// <summary>
    /// The pages to show as buttons, <see cref="Gap"/> marking an ellipsis:
    /// for page 5 of 19, <c>1 … 3 4 5 6 7 … 19</c>. Pages count from 1.
    /// The same as <c>_pagination_sequence()</c> of the Linux window.
    /// </summary>
    public static List<int> Sequence(int current, int total, int window = 2)
    {
        var sequence = new List<int>();
        if (total <= 0)
        {
            return sequence;
        }
        var pages = new SortedSet<int> { 1, total };
        for (var page = current - window; page <= current + window; page++)
        {
            if (page >= 1 && page <= total)
            {
                pages.Add(page);
            }
        }
        int? previous = null;
        foreach (var page in pages)
        {
            if (previous is { } p && page - p > 1)
            {
                sequence.Add(Gap);
            }
            sequence.Add(page);
            previous = page;
        }
        return sequence;
    }

    public static int PageCount(int items, int pageSize) => items <= 0 ? 0 : (items - 1) / pageSize + 1;
}

/// <summary>What a generation left in an output folder: the "View results" menu offers what exists.</summary>
public sealed record Results(string OutputDir, bool Sheets, bool Pdf, bool Gallery, bool Csv)
{
    public string SheetsDir => Path.Combine(OutputDir, "planches");
    public string PdfFile => Path.Combine(OutputDir, "portfolio.pdf");
    public string GalleryIndex => Path.Combine(OutputDir, "gallery", "index.html");
    public string CsvFile => Path.Combine(OutputDir, "index.csv");

    public bool Any => Sheets || Pdf || Gallery || Csv;

    /// <summary>The results found in <paramref name="outputDir"/>; nothing for an empty or missing folder.</summary>
    public static Results Check(string? outputDir)
    {
        var dir = (outputDir ?? "").Trim();
        if (dir.Length == 0 || !Directory.Exists(dir))
        {
            return new Results(dir, false, false, false, false);
        }
        var found = new Results(dir, false, false, false, false);
        bool sheets;
        try
        {
            sheets = Directory.Exists(found.SheetsDir) && Directory.EnumerateFileSystemEntries(found.SheetsDir).Any();
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            sheets = false;
        }
        return found with
        {
            Sheets = sheets,
            Pdf = File.Exists(found.PdfFile),
            Gallery = File.Exists(found.GalleryIndex),
            Csv = File.Exists(found.CsvFile),
        };
    }
}

/// <summary>Files installed with the window: the manual and the version.</summary>
public static class Installed
{
    /// <summary>
    /// The manual in <paramref name="language"/>: French is
    /// <c>planche-contact-manual.html</c>, the others
    /// <c>planche-contact-manual.xx.html</c>; French when the language has
    /// none. Null when no manual is installed.
    /// </summary>
    public static string? Manual(string docsDir, string language)
    {
        var french = Path.Combine(docsDir, "planche-contact-manual.html");
        var candidate = language == "fr" ? french : Path.Combine(docsDir, $"planche-contact-manual.{language}.html");
        if (File.Exists(candidate))
        {
            return candidate;
        }
        return File.Exists(french) ? french : null;
    }

    /// <summary>The version of the VERSION file and its date, or null.</summary>
    public static (string Version, DateTime Date)? Version(string versionFile)
    {
        try
        {
            var version = File.ReadAllText(versionFile).Trim();
            return version.Length == 0 ? null : (version, File.GetLastWriteTime(versionFile));
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>The date as the About tab of the Linux window writes it.</summary>
    public static string FormatDate(DateTime date) => date.ToString("dd/MM/yyyy", CultureInfo.InvariantCulture);
}
