// SPDX-License-Identifier: GPL-3.0-only

using System.Text.Json.Nodes;

namespace PlancheContact.Core.Tests;

internal static class CoreTests
{
    // ---------------------------------------------------------- arguments

    [Test]
    private static void Arguments_are_kept_for_the_engine()
    {
        var parsed = AppArguments.Parse(["--paths-from", @"C:\Temp\planche-contact-selection-1.txt", ""]);
        Check.Sequence(["--paths-from", @"C:\Temp\planche-contact-selection-1.txt"], parsed.Arguments);
        Check.True(parsed.HasPaths, "a list file is something to open");
        Check.True(!AppArguments.Parse([]).HasPaths, "nothing to open");
        Check.True(!AppArguments.Parse(["--"]).HasPaths, "-- alone is nothing to open");
    }

    // --------------------------------------------------------- generation

    private static GenerationRequest Request() => new()
    {
        InputDir = @"C:\Photos\Été 2026",
        Language = "fr",
    };

    [Test]
    private static void Generation_arguments_match_the_gtk_window()
    {
        using var files = GenerationFiles.Write(Request());
        Check.Sequence(
        [
            "-i", @"C:\Photos\Été 2026",
            "-o", Path.Combine(@"C:\Photos\Été 2026", "Portfolio"),
            "-n", "12", "--format", "A4", "--html-per-page", "48", "--language", "fr",
            "--pdf", "--html", "--csv", "--profile", files.ProfileFile!,
        ], Request().GenerateArguments(files));
    }

    [Test]
    private static void Optional_texts_and_outputs()
    {
        var request = Request() with
        {
            OutputDir = "  D:\\Out  ", Title = " Mariage ", Author = "Gilles", Watermark = "©",
            WatermarkOrientation = "Diagonale horaire", WatermarkOpacity = 55, PerSheet = 20, PageFormat = "A3",
            Pdf = false, Html = true, HtmlOriginals = true, Csv = false, HtmlPerPage = 64,
        };
        using var files = GenerationFiles.Write(request);
        var arguments = request.GenerateArguments(files);
        Check.Sequence(
        [
            "-i", @"C:\Photos\Été 2026", "-o", "D:\\Out", "-n", "20", "--format", "A3", "--html-per-page", "64",
            "--language", "fr", "--title", "Mariage", "--author", "Gilles", "--watermark", "©",
            "--watermark-orientation", "Diagonale horaire", "--watermark-opacity", "55",
            "--html", "--html-originals", "--profile", files.ProfileFile!,
        ], arguments);

        var noHtml = request with { Html = false };
        Check.True(!noHtml.GenerateArguments(files).Contains("--html-originals"), "originals only with the gallery");
    }

    [Test]
    private static void Watermark_options_only_with_a_watermark()
    {
        var request = Request() with { Watermark = "   ", WatermarkOpacity = 10 };
        using var files = GenerationFiles.Write(request);
        Check.True(!request.GenerateArguments(files).Contains("--watermark-opacity"), "no watermark, no opacity");
    }

    [Test]
    private static void Subfolders_are_passed_in_a_file_with_recursive_search()
    {
        var request = Request() with { Recursive = true, IncludeDirs = [".", "Paris", "été/août"] };
        using var files = GenerationFiles.Write(request);
        var arguments = request.GenerateArguments(files);
        Check.True(arguments.Contains("-r"), "recursive");
        var index = arguments.IndexOf("--include-dirs-file");
        Check.True(index > 0, "the list of subfolders is passed");
        Check.Equal(".\nParis\nété/août\n", File.ReadAllText(arguments[index + 1]));
        var bytes = File.ReadAllBytes(arguments[index + 1]);
        Check.True(bytes[0] == (byte)'.', "UTF-8 without byte order mark");

        var notRecursive = request with { Recursive = false };
        using var other = GenerationFiles.Write(notRecursive);
        Check.True(other.IncludeDirsFile is null, "the subfolder choice only applies to a recursive search");
    }

    [Test]
    private static void A_selection_is_processed_as_it_is()
    {
        var request = Request() with { Recursive = true, IncludeDirs = ["Paris"], Files = ["a.jpg", "Paris/b.jpg"] };
        using var files = GenerationFiles.Write(request);
        var arguments = request.GenerateArguments(files);
        Check.True(!arguments.Contains("-r"), "no recursive search for a selection");
        Check.True(!arguments.Contains("--include-dirs-file"), "no subfolder choice for a selection");
        Check.Equal("a.jpg\nParis/b.jpg\n", File.ReadAllText(arguments[arguments.IndexOf("--files-from") + 1]));
    }

    [Test]
    private static void The_profile_is_passed_only_when_customised()
    {
        var plain = Request() with { Profile = Profile.Default() };
        using (var files = GenerationFiles.Write(plain))
        {
            Check.True(files.ProfileFile is null, "the command-line default needs no --profile");
            Check.True(!plain.GenerateArguments(files).Contains("--profile"), "no --profile");
        }
        string path;
        using (var files = GenerationFiles.Write(Request()))
        {
            path = files.ProfileFile!;
            var json = JsonNode.Parse(File.ReadAllText(path))!;
            Check.Equal("below", json["layout"]!["caption_position"]!.GetValue<string>());
            Check.Equal("filename", json["captions"]!["fields"]![0]!.GetValue<string>());
        }
        Check.True(!File.Exists(path), "temporary files are deleted");
    }

    [Test]
    private static void Preview_arguments()
    {
        var request = Request() with { Recursive = true, Pdf = true, Csv = true };
        using var files = GenerationFiles.Write(request);
        Check.Sequence(
        [
            "-i", @"C:\Photos\Été 2026", "-n", "12", "--format", "A4", "--language", "fr",
            "--preview-output", "p.png", "-r", "--profile", files.ProfileFile!,
        ], request.PreviewArguments(files, "p.png"));
    }

    // ------------------------------------------------------------ profile

    [Test]
    private static void Profile_json_has_the_shape_of_to_dict()
    {
        var json = Profile.InterfaceDefault().ToJsonObject();
        Check.Sequence(["name", "layout", "captions", "theme"], json.Select(p => p.Key));
        Check.Sequence(
            ["template", "columns", "rows", "per_page", "margin_mm", "spacing_mm", "page_format", "orientation",
             "fit_mode", "title_position", "author_position", "page_numbering", "caption_position", "caption_area_mm"],
            json["layout"]!.AsObject().Select(p => p.Key));
        Check.Sequence(["fields", "format_overrides", "lines", "font_size", "alignment", "separator", "show_gps"],
            json["captions"]!.AsObject().Select(p => p.Key));
        Check.Equal(" · ", json["captions"]!["separator"]!.GetValue<string>());
        Check.True(Profile.InterfaceDefault().ToJson().Contains("\" · \""), "the separator is written as it is");
    }

    [Test]
    private static void Profile_round_trip()
    {
        var profile = Profile.InterfaceDefault();
        profile.Layout.Template = "free";
        profile.Layout.Columns = 4;
        profile.Theme.Custom.Accent = "#a0b0c0";
        var back = Profile.FromJson(profile.ToJsonObject());
        Check.True(back.SameAs(profile), "same profile after a round trip");
        Check.True(!back.SameAs(Profile.Default()), "differs from the default");
        Check.True(Profile.Default().SameAs(Profile.FromJson([])), "an empty object is the default profile");
    }

    [Test]
    private static void Custom_colours_are_checked()
    {
        Check.Equal("#A0b0c0", ThemeCustomColors.Parse(" #A0b0c0 ", out var invalid));
        Check.True(!invalid, "valid");
        Check.Equal(null, ThemeCustomColors.Parse("", out invalid));
        Check.True(!invalid, "empty is not an error");
        Check.Equal(null, ThemeCustomColors.Parse("red", out invalid));
        Check.True(invalid, "flagged");
    }

    [Test]
    private static void Gallery_slider_snaps_like_the_gtk_window()
    {
        Check.Equal(12, Choices.SnapHtmlPerPage(3));
        Check.Equal(48, Choices.SnapHtmlPerPage(49.9));
        Check.Equal(12, Choices.SnapHtmlPerPage(14)); // round half to even, as Python
        Check.Equal(20, Choices.SnapHtmlPerPage(18));
        Check.Equal(20, Choices.SnapHtmlPerPage(22));
        Check.Equal(24, Choices.SnapHtmlPerPage(23));
        Check.Equal(64, Choices.SnapHtmlPerPage(80));
        Check.Sequence([8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48], Choices.PerSheet);
    }

    // ------------------------------------------------------------ helpers

    [Test]
    private static void Pagination_sequence()
    {
        const int gap = Pagination.Gap;
        Check.Sequence([1, gap, 3, 4, 5, 6, 7, gap, 19], Pagination.Sequence(5, 19));
        Check.Sequence([1, 2, 3, gap, 10], Pagination.Sequence(1, 10));
        Check.Sequence([1], Pagination.Sequence(1, 1));
        Check.Sequence([], Pagination.Sequence(1, 0));
        Check.Sequence([1, 2, 3, 4, 5], Pagination.Sequence(3, 5));
        Check.Equal(3, Pagination.PageCount(21, 10));
        Check.Equal(0, Pagination.PageCount(0, 10));
    }

    [Test]
    private static void Results_found_in_an_output_folder()
    {
        var dir = Directory.CreateTempSubdirectory("pc-results-").FullName;
        try
        {
            Check.True(!Results.Check(dir).Any, "nothing yet");
            Directory.CreateDirectory(Path.Combine(dir, "planches"));
            Check.True(!Results.Check(dir).Sheets, "an empty sheets folder is nothing");
            File.WriteAllText(Path.Combine(dir, "planches", "planche_001.jpg"), "x");
            File.WriteAllText(Path.Combine(dir, "index.csv"), "x");
            var found = Results.Check(dir);
            Check.Equal((true, false, false, true), (found.Sheets, found.Pdf, found.Gallery, found.Csv));
            Check.True(!Results.Check("").Any && !Results.Check(Path.Combine(dir, "missing")).Any, "no folder");
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Test]
    private static void Manual_in_the_language_or_french()
    {
        var docs = Directory.CreateTempSubdirectory("pc-docs-").FullName;
        try
        {
            File.WriteAllText(Path.Combine(docs, "planche-contact-manual.html"), "fr");
            File.WriteAllText(Path.Combine(docs, "planche-contact-manual.de.html"), "de");
            Check.Equal(Path.Combine(docs, "planche-contact-manual.de.html"), Installed.Manual(docs, "de"));
            Check.Equal(Path.Combine(docs, "planche-contact-manual.html"), Installed.Manual(docs, "fr"));
            Check.Equal(Path.Combine(docs, "planche-contact-manual.html"), Installed.Manual(docs, "es"), "French when the language has none");
            Check.Equal(null, Installed.Manual(Path.Combine(docs, "missing"), "en"));
        }
        finally
        {
            Directory.Delete(docs, recursive: true);
        }
    }

    [Test]
    private static void Settings_are_shared_with_the_gtk_window()
    {
        var dir = Directory.CreateTempSubdirectory("pc-settings-").FullName;
        try
        {
            var file = Path.Combine(dir, "sub", "settings.json");
            var empty = Settings.Load(file);
            Check.Equal(("", "", "system"), (empty.LastInputDir, empty.LastOutputDir, empty.Language));

            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            File.WriteAllText(file, """{"last_input_dir": "C:\\Photos", "language": "de", "future": 1}""");
            var settings = Settings.Load(file);
            Check.Equal(("C:\\Photos", "de"), (settings.LastInputDir, settings.Language));
            settings.LastOutputDir = "D:\\Été";
            settings.Language = "fr";
            Check.True(settings.Save(), "saved");
            var json = JsonNode.Parse(File.ReadAllText(file))!;
            Check.Equal("D:\\Été", json["last_output_dir"]!.GetValue<string>());
            Check.Equal("fr", json["language"]!.GetValue<string>());
            Check.Equal(1, json["future"]!.GetValue<int>(), "unknown keys are kept");

            File.WriteAllText(file, "{broken");
            Check.Equal("system", Settings.Load(file).Language, "a broken file is empty settings");
            File.WriteAllText(file, """{"language": "klingon"}""");
            Check.Equal("system", Settings.Load(file).Language, "an unknown language is the system's");
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }
}
