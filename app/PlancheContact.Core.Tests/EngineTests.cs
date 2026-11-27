// SPDX-License-Identifier: GPL-3.0-only

using System.Text.Json.Nodes;
using PlancheContact.Core.Engine;

namespace PlancheContact.Core.Tests;

internal static class EngineTests
{
    // ------------------------------------------------ reading what it writes

    [Test]
    private static void Events_are_read_from_json_lines()
    {
        Check.Equal(new PhotoFound(@"C:\a b\é.jpg"), EngineEvent.Parse("""{"event": "photo", "path": "C:\\a b\\é.jpg"}"""));
        Check.Equal(new FolderFound("été/Paris", 3), EngineEvent.Parse("""{"event": "folder", "path": "été/Paris", "count": 3}"""));
        Check.Equal(new Summary(4, 0), EngineEvent.Parse("""{"event": "summary", "total": 4}"""));
        var selection = (SelectionResolved)EngineEvent.Parse("""{"event": "selection", "input_dir": "/p", "files": ["a.jpg"]}""")!;
        Check.Equal("/p", selection.InputDir);
        Check.Sequence(["a.jpg"], selection.Files!);
        var whole = (SelectionResolved)EngineEvent.Parse("""{"event": "selection", "input_dir": "/p", "files": null}""")!;
        Check.True(whole.Files is null, "the whole folder");
        var failed = (ThumbnailMade)EngineEvent.Parse("""{"event": "thumbnail", "index": 2, "input": "x", "error": "bad"}""")!;
        Check.True(!failed.Ok, "a failed thumbnail");
        Check.Equal(new FatalError("boom"), EngineEvent.Parse("""{"event": "fatal_error", "message": "boom"}"""));
        Check.Equal(null, EngineEvent.Parse("""{"event": "something new", "x": 1}"""), "unknown events are ignored");
        Check.Equal(null, EngineEvent.Parse("2026-11-27 10:00:00 [INFO] starting"), "a log line is no event");
        Check.Equal(null, EngineEvent.Parse("{not json"), "broken JSON is no event");
    }

    [Test]
    private static void Generation_markers()
    {
        Check.Equal((15, "Génération des planches..."), GenerationLine.Progress("PROGRESS:15/100 Génération des planches..."));
        Check.Equal((40, ""), GenerationLine.Progress("PROGRESS:40/100"));
        Check.Equal(null, GenerationLine.Progress("Planche 1/3 générée"));
        Check.Equal((12, 30), GenerationLine.Preview("PREVIEW:12/30"));
        Check.Equal(null, GenerationLine.Preview("PROGRESS:12/30"));
    }

    [Test]
    private static void Engine_path_and_override()
    {
        var saved = Environment.GetEnvironmentVariable(EngineCommand.EngineVariable);
        try
        {
            Environment.SetEnvironmentVariable(EngineCommand.EngineVariable, null);
            var installed = EngineCommand.FromEnvironment("/app");
            Check.Equal(Path.Combine("/app", "engine", "planche-contact-engine.exe"), installed.FileName);

            Environment.SetEnvironmentVariable(EngineCommand.EngineVariable, "/repo/target/release/planche-contact-engine");
            var built = EngineCommand.FromEnvironment("/app");
            Check.Equal("/repo/target/release/planche-contact-engine", built.FileName, "the variable wins");
        }
        finally
        {
            Environment.SetEnvironmentVariable(EngineCommand.EngineVariable, saved);
        }
    }

    [Test]
    private static void A_missing_engine_cannot_start()
    {
        var runner = new EngineRunner(new EngineCommand(Path.Combine(Path.GetTempPath(), "no-such-engine.exe")));
        try
        {
            runner.Start(["info"], _ => { });
            throw new TestFailure("a missing program must not start");
        }
        catch (EngineStartException)
        {
            // Expected.
        }
    }

    // ------------------------------------------------ the real engine

    [Test]
    private static void Engine_choices_and_defaults_are_those_of_the_window()
    {
        var info = TestEngine.Run("info").Events.OfType<EngineInfo>().Single();
        Check.Equal(File.ReadAllText(Repo.File_("VERSION")).Trim(), info.Version);
        Check.Sequence(Choices.PageFormats, info.Choice("page_formats"));
        Check.Sequence(Choices.LayoutTemplates, info.Choice("layout_templates"));
        Check.Sequence(Choices.FitModes, info.Choice("fit_modes"));
        Check.Sequence(Choices.Orientations, info.Choice("orientations"));
        Check.Sequence(Choices.HeaderPositions, info.Choice("header_positions"));
        Check.Sequence(Choices.CaptionPositions, info.Choice("caption_positions"));
        Check.Sequence(Choices.CaptionAlignments, info.Choice("caption_alignments"));
        Check.Sequence(Choices.CaptionFields, info.Choice("caption_fields"));
        Check.Sequence(Choices.ThemeModes, info.Choice("theme_modes"));
        Check.Sequence(Choices.WatermarkOrientations.Select(o => o.Value), info.Choice("watermark_orientations"));
        Check.Sequence(Localization.Translator.Preferences, info.Json["languages"]!.AsArray().Select(v => v!.GetValue<string>()));

        Check.True(Profile.FromJson(info.Json["default_profile"]!.AsObject()).SameAs(Profile.Default()), "same default profile");
        Check.True(Profile.FromJson(info.Json["interface_default_profile"]!.AsObject()).SameAs(Profile.InterfaceDefault()),
            "same interface default profile");
        Check.True(JsonNode.DeepEquals(info.Json["default_profile"], Profile.Default().ToJsonObject()),
            "the JSON of the default profile is that of to_dict()");
    }

    [Test]
    private static void Engine_lists_photos_folders_and_selections()
    {
        var root = PhotoTree();
        try
        {
            var photos = TestEngine.Run("photos", "--input", root).Events;
            Check.Sequence(["a.jpg", "b.png"], photos.OfType<PhotoFound>().Select(p => Path.GetFileName(p.Path)));

            var folders = TestEngine.Run("folders", "--input", root).Events.OfType<FolderFound>().ToList();
            Check.Sequence([(".", 2), ("été 2026", 1)], folders.Select(f => (f.RelativePath, f.Count)));

            var list = TempFile.WriteLines("planche-contact-selection-", [Path.Combine(root, "a.jpg"), Path.Combine(root, "été 2026", "c.jpg")]);
            var selection = TestEngine.Run("selection", "--paths-from", list).Events.OfType<SelectionResolved>().Single();
            Check.Equal(root, selection.InputDir);
            Check.Sequence(["a.jpg", "été 2026/c.jpg"], selection.Files!);
            Check.True(!File.Exists(list), "the list of the context menu is deleted once read");

            var none = TestEngine.Run("selection", Path.Combine(root, "notes.txt")).Events.OfType<SelectionResolved>().Single();
            Check.Equal(null, none.InputDir);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Test]
    private static void Engine_makes_thumbnails_and_previews()
    {
        var root = PhotoTree();
        var thumbs = Directory.CreateTempSubdirectory("pc-thumbs-").FullName;
        try
        {
            var list = TempFile.WriteLines("pc-thumbs-", [Path.Combine(root, "a.jpg"), Path.Combine(root, "notes.txt")]);
            var made = TestEngine.Run("thumbnails", "--size", "90", "--output-dir", thumbs, "--input-list", list)
                .Events.OfType<ThumbnailMade>().ToList();
            File.Delete(list);
            Check.Equal(2, made.Count);
            Check.True(made[0].Ok && File.Exists(made[0].Output), "a thumbnail of the photo");
            Check.Equal((90, 60), (made[0].Width, made[0].Height));
            Check.True(!made[1].Ok, "the text file is reported, not fatal");

            var request = new GenerationRequest { InputDir = root, Language = "fr", Recursive = true };
            using var files = GenerationFiles.Write(request);
            var png = Path.Combine(root, "preview.png");
            var result = TestEngine.Run(["generate", .. request.PreviewArguments(files, png)]);
            Check.Equal(0, result.ExitCode, result.ErrorText);
            Check.True(File.Exists(png), "the preview is written");
            Check.Equal((3, 3), result.Output.Select(GenerationLine.Preview).OfType<(int, int)>().Single());
        }
        finally
        {
            Directory.Delete(root, recursive: true);
            Directory.Delete(thumbs, recursive: true);
        }
    }

    [Test]
    private static void Engine_generates_with_the_window_arguments()
    {
        var root = PhotoTree();
        try
        {
            var request = new GenerationRequest
            {
                InputDir = root, OutputDir = Path.Combine(root, "sortie é"), Language = "fr",
                Recursive = true, IncludeDirs = ["été 2026"], Html = false, Pdf = true, Csv = true,
                Watermark = "Démo", Title = "Été",
            };
            using var files = GenerationFiles.Write(request);
            var progress = new List<int>();
            var result = TestEngine.Run(["generate", .. request.GenerateArguments(files)]);
            Check.Equal(0, result.ExitCode, result.ErrorText);
            progress.AddRange(result.Output.Select(GenerationLine.Progress).OfType<(int, string)>().Select(p => p.Item1));
            Check.Equal(100, progress.Last());
            Check.True(result.Output.Any(line => line.Contains("Génération", StringComparison.Ordinal)
                || line.Contains("planche", StringComparison.OrdinalIgnoreCase)), "French messages read as UTF-8");
            var found = Results.Check(request.EffectiveOutputDir);
            Check.Equal((true, true, false, true), (found.Sheets, found.Pdf, found.Gallery, found.Csv));
            var csv = File.ReadAllText(found.CsvFile);
            Check.True(csv.Contains("c.jpg") && !csv.Contains("a.jpg"), "only the chosen subfolder");
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Test]
    private static void Engine_normalises_a_profile()
    {
        var file = TempFile.Write("pc-profile-", ".json", """{"layout": {"template": "free", "rows": 2, "margin_mm": "x"}, "theme": {"mode": "dark"}}""");
        try
        {
            var read = TestEngine.Run("profile", "--load", file).Events.OfType<ProfileRead>().Single();
            var profile = Profile.FromJson(read.Profile);
            Check.Equal(("free", (int?)2, 12.0, "dark"), (profile.Layout.Template, profile.Layout.Rows, profile.Layout.MarginMm, profile.Theme.Mode));
        }
        finally
        {
            File.Delete(file);
        }
    }

    /// <summary>
    /// root/ a.jpg b.png notes.txt, root/été 2026/c.jpg — three small photos
    /// made without any image library: a minimal valid PNG, written as the
    /// three files (the engine reads the content, not the extension).
    /// </summary>
    private static string PhotoTree()
    {
        var root = Directory.CreateTempSubdirectory("pc-engine-").FullName;
        var png = Convert.FromBase64String(TinyPng);
        File.WriteAllBytes(Path.Combine(root, "a.jpg"), png);
        File.WriteAllBytes(Path.Combine(root, "b.png"), png);
        File.WriteAllText(Path.Combine(root, "notes.txt"), "not a photo");
        Directory.CreateDirectory(Path.Combine(root, "été 2026"));
        File.WriteAllBytes(Path.Combine(root, "été 2026", "c.jpg"), png);
        return root;
    }

    // A 120x80 grey PNG.
    private const string TinyPng =
        "iVBORw0KGgoAAAANSUhEUgAAAHgAAABQCAIAAABd+SbeAAAAf0lEQVR42u3QQQ0AAAgEIDX1xTeFmw+IQCcp7o0C0aIRLVo0okUjWrRoRItGtGjRiBaNaNGiES0a0aJFI1o0okWLRrRoRIsWjWjRiBYtGtGiES1aNKJFI1q0aESLRrRo0YgWjWjRohEtGtGiRSNaNKJFi0a0aESLFo1o0Yj+YwFe3gIIXmsXbQAAAABJRU5ErkJggg==";
}
