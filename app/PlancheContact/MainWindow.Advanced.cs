// SPDX-License-Identifier: GPL-3.0-only

using System.Runtime.InteropServices;
using System.Text;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using PlancheContact.Core;
using PlancheContact.Core.Engine;
using Windows.Graphics;

namespace PlancheContact;

/// <summary>
/// The "Advanced settings" window of the Linux version: layout, captions,
/// theme colours, reusable profiles and the preview of the first sheet. The
/// controls are built once and keep their values between two openings; they
/// fill in the profile handed to the engine (<see cref="Profile"/>). The
/// number of photos per sheet and the page format stay those of the main
/// form, as in the Linux window.
/// </summary>
public sealed partial class MainWindow
{
    private ComboBox _advTemplate = null!;
    private ComboBox _advOrientation = null!;
    private ComboBox _advFitMode = null!;
    private ComboBox _advTitlePosition = null!;
    private ComboBox _advAuthorPosition = null!;
    private ComboBox _advCaptionPosition = null!;
    private ComboBox _advCaptionAlignment = null!;
    private NumberBox _advColumns = null!;
    private NumberBox _advRows = null!;
    private NumberBox _advMargin = null!;
    private NumberBox _advSpacing = null!;
    private NumberBox _advCaptionArea = null!;
    private NumberBox _advCaptionLines = null!;
    private NumberBox _advCaptionFont = null!;
    private readonly CheckBox _advPageNumbering = new();
    private readonly Dictionary<string, CheckBox> _advFields = [];
    private readonly TextBox _advBackground = new();
    private readonly TextBox _advText = new();
    private readonly TextBox _advAccent = new();
    private readonly TextBox _advBorder = new();
    private readonly TextBlock _advStatus = Ui.Text("", dim: true);
    private ContentDialog? _advDialog;
    private StackPanel _advContent = null!;

    private Window? _previewWindow;
    private EngineRun? _previewRun;

    private void BuildAdvancedSettings()
    {
        var box = new StackPanel { Spacing = 10, Padding = new Thickness(0, 0, 16, 0) };

        // Layout.
        box.Children.Add(Ui.Title(_t.T("adv.layout_subtitle"), "BodyStrongTextBlockStyle"));
        var layout = Ui.Columns(Ui.Star(), Ui.Star(), Ui.Star());
        Ui.Rows(layout, 4);
        _advTemplate = ChoiceCombo("adv.template_label", Choices.LayoutTemplates, "adv.template.");
        _advTemplate.SelectionChanged += (_, _) => UpdateAdvancedSensitivity();
        _advOrientation = ChoiceCombo("adv.orientation_label", Choices.Orientations, "adv.orientation.");
        _advFitMode = ChoiceCombo("adv.fit_mode_label", Choices.FitModes, "adv.fit_mode.");
        ToolTipService.SetToolTip(_advFitMode, _t.T("adv.fit_mode_tooltip"));
        Ui.Place(layout, _advTemplate, row: 0);
        Ui.Place(layout, _advOrientation, row: 0, column: 1);
        Ui.Place(layout, _advFitMode, row: 0, column: 2);

        _advColumns = Ui.Number(_t.T("adv.free_columns_label"), 0, 12);
        _advRows = Ui.Number(_t.T("adv.free_rows_label"), 0, 12);
        Ui.Place(layout, _advColumns, row: 1);
        Ui.Place(layout, _advRows, row: 1, column: 1);

        _advMargin = Ui.Number(_t.T("adv.margin_label"), 0, 60);
        _advSpacing = Ui.Number(_t.T("adv.spacing_label"), 0, 40);
        Ui.Place(layout, _advMargin, row: 2);
        Ui.Place(layout, _advSpacing, row: 2, column: 1);

        _advTitlePosition = ChoiceCombo("adv.title_position_label", Choices.HeaderPositions, "adv.header_position.");
        _advAuthorPosition = ChoiceCombo("adv.author_position_label", Choices.HeaderPositions, "adv.header_position.");
        _advPageNumbering.Content = _t.T("adv.page_numbering_label");
        _advPageNumbering.VerticalAlignment = VerticalAlignment.Bottom;
        ToolTipService.SetToolTip(_advPageNumbering, _t.T("adv.page_numbering_tooltip"));
        Ui.Place(layout, _advTitlePosition, row: 3);
        Ui.Place(layout, _advAuthorPosition, row: 3, column: 1);
        Ui.Place(layout, _advPageNumbering, row: 3, column: 2);
        box.Children.Add(layout);

        // Captions per photo.
        box.Children.Add(Ui.Title(_t.T("adv.captions_subtitle"), "BodyStrongTextBlockStyle"));
        var captions = Ui.Columns(Ui.Star(), Ui.Star(), Ui.Star());
        Ui.Rows(captions, 2);
        _advCaptionPosition = ChoiceCombo("adv.caption_position_label", Choices.CaptionPositions, "adv.caption_position.");
        _advCaptionPosition.SelectionChanged += (_, _) => UpdateAdvancedSensitivity();
        _advCaptionArea = Ui.Number(_t.T("adv.caption_area_label"), 4, 40);
        _advCaptionLines = Ui.Number(_t.T("adv.caption_lines_label"), 1, 4);
        ToolTipService.SetToolTip(_advCaptionLines, _t.T("adv.caption_lines_tooltip"));
        Ui.Place(captions, _advCaptionPosition, row: 0);
        Ui.Place(captions, _advCaptionArea, row: 0, column: 1);
        Ui.Place(captions, _advCaptionLines, row: 0, column: 2);
        _advCaptionFont = Ui.Number(_t.T("adv.caption_font_size_label"), 6, 24);
        _advCaptionAlignment = ChoiceCombo("adv.caption_alignment_label", Choices.CaptionAlignments, "adv.caption_alignment.");
        Ui.Place(captions, _advCaptionFont, row: 1);
        Ui.Place(captions, _advCaptionAlignment, row: 1, column: 1);
        box.Children.Add(captions);

        box.Children.Add(Ui.Text(_t.T("adv.caption_fields_label")));
        var fields = Ui.Columns(Ui.Star(), Ui.Star(), Ui.Star(), Ui.Star());
        Ui.Rows(fields, (Choices.CaptionFields.Count + 3) / 4);
        fields.RowSpacing = 0;
        for (var i = 0; i < Choices.CaptionFields.Count; i++)
        {
            var name = Choices.CaptionFields[i];
            var check = new CheckBox { Content = _t.T($"adv.field.{name}") };
            if (name == "gps")
            {
                ToolTipService.SetToolTip(check, _t.T("adv.field_gps_warning"));
            }
            _advFields[name] = check;
            Ui.Place(fields, check, row: i / 4, column: i % 4);
        }
        box.Children.Add(fields);

        // Theme colours (the mode itself is chosen in the main form).
        box.Children.Add(Ui.Title(_t.T("adv.theme_subtitle"), "BodyStrongTextBlockStyle"));
        box.Children.Add(Ui.Text(_t.T("adv.custom_colors_label"), dim: true));
        var colors = Ui.Columns(Ui.Star(), Ui.Star(), Ui.Star(), Ui.Star());
        var colorFields = new (TextBox Box, string Key)[]
        {
            (_advBackground, "adv.custom_background_label"), (_advText, "adv.custom_text_label"),
            (_advAccent, "adv.custom_accent_label"), (_advBorder, "adv.custom_border_label"),
        };
        for (var i = 0; i < colorFields.Length; i++)
        {
            var (field, key) = colorFields[i];
            field.Header = _t.T(key);
            field.PlaceholderText = _t.T("adv.custom_color_placeholder");
            field.TextChanged += (_, _) => FlagColor(field);
            Ui.Place(colors, field, row: 0, column: i);
        }
        box.Children.Add(colors);

        // Preview and profiles.
        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        var preview = Ui.AccentButton(_t.T("adv.preview_button"));
        preview.Click += async (_, _) => await ShowSheetPreviewAsync();
        var save = new Button { Content = _t.T("adv.save_profile_button") };
        save.Click += async (_, _) => await SaveProfileAsync();
        var load = new Button { Content = _t.T("adv.load_profile_button") };
        load.Click += async (_, _) => await LoadProfileAsync();
        actions.Children.Add(preview);
        actions.Children.Add(save);
        actions.Children.Add(load);
        box.Children.Add(actions);

        _advContent = box;
    }

    /// <summary>A drop-down showing translated texts for internal values (kept in <c>Tag</c>).</summary>
    private ComboBox ChoiceCombo(string headerKey, IReadOnlyList<string> values, string keyPrefix)
    {
        var combo = Ui.Combo(_t.T(headerKey), values.Select(value => _t.T(keyPrefix + value)));
        combo.Tag = values;
        return combo;
    }

    private static string ValueOf(ComboBox combo)
    {
        var values = (IReadOnlyList<string>)combo.Tag;
        return values[Math.Clamp(combo.SelectedIndex, 0, values.Count - 1)];
    }

    private static void Select(ComboBox combo, string value)
    {
        var values = (IReadOnlyList<string>)combo.Tag;
        combo.SelectedIndex = Math.Max(0, values.ToList().IndexOf(value));
    }

    /// <summary>A colour that is neither empty nor "#rrggbb" is flagged, without blocking anything.</summary>
    private static void FlagColor(TextBox field)
    {
        ThemeCustomColors.Parse(field.Text, out var invalid);
        if (invalid && Ui.TryGetResource("SystemFillColorCriticalBrush") is Brush critical)
        {
            field.BorderBrush = critical;
        }
        else
        {
            field.ClearValue(Control.BorderBrushProperty);
        }
    }

    private void UpdateAdvancedSensitivity()
    {
        var free = ValueOf(_advTemplate) == "free";
        _advColumns.IsEnabled = free;
        _advRows.IsEnabled = free;
        var caption = ValueOf(_advCaptionPosition) != "none";
        _advCaptionArea.IsEnabled = caption;
        _advCaptionLines.IsEnabled = caption;
        _advCaptionFont.IsEnabled = caption;
        _advCaptionAlignment.IsEnabled = caption;
        foreach (var check in _advFields.Values)
        {
            check.IsEnabled = caption;
        }
    }

    /// <summary>The profile the controls describe, with the theme mode of the main form.</summary>
    private Profile ProfileFromControls()
    {
        static int? AutoOrValue(NumberBox box) => (int)Ui.ValueOf(box) is var n && n > 0 ? n : null;

        var gps = _advFields["gps"].IsChecked == true;
        var fields = Choices.CaptionFields.Where(name => name != "gps" && _advFields[name].IsChecked == true).ToList();
        if (gps)
        {
            fields.Add("gps");
        }
        return new Profile
        {
            Layout = new LayoutSettings
            {
                Template = ValueOf(_advTemplate),
                Columns = AutoOrValue(_advColumns),
                Rows = AutoOrValue(_advRows),
                MarginMm = Ui.ValueOf(_advMargin),
                SpacingMm = Ui.ValueOf(_advSpacing),
                Orientation = ValueOf(_advOrientation),
                FitMode = ValueOf(_advFitMode),
                CaptionPosition = ValueOf(_advCaptionPosition),
                CaptionAreaMm = Ui.ValueOf(_advCaptionArea),
                TitlePosition = ValueOf(_advTitlePosition),
                AuthorPosition = ValueOf(_advAuthorPosition),
                PageNumbering = _advPageNumbering.IsChecked == true,
            },
            Captions = new CaptionSettings
            {
                Fields = fields,
                Lines = (int)Ui.ValueOf(_advCaptionLines),
                FontSize = (int)Ui.ValueOf(_advCaptionFont),
                Alignment = ValueOf(_advCaptionAlignment),
                ShowGps = gps,
            },
            Theme = new ThemeSettings
            {
                Mode = Choices.ThemeModes[Math.Max(0, _themeMode.SelectedIndex)],
                Custom = new ThemeCustomColors
                {
                    Background = ThemeCustomColors.Parse(_advBackground.Text, out _),
                    Text = ThemeCustomColors.Parse(_advText.Text, out _),
                    Accent = ThemeCustomColors.Parse(_advAccent.Text, out _),
                    Border = ThemeCustomColors.Parse(_advBorder.Text, out _),
                },
            },
        };
    }

    /// <summary>Shows a profile (loaded, or the default one after "Reset") in the controls.</summary>
    private void ApplyProfileToControls(Profile profile)
    {
        Select(_advTemplate, profile.Layout.Template);
        Select(_advOrientation, profile.Layout.Orientation);
        Select(_advFitMode, profile.Layout.FitMode);
        _advMargin.Value = profile.Layout.MarginMm;
        _advSpacing.Value = profile.Layout.SpacingMm;
        _advColumns.Value = profile.Layout.Columns ?? 0;
        _advRows.Value = profile.Layout.Rows ?? 0;
        Select(_advTitlePosition, profile.Layout.TitlePosition);
        Select(_advAuthorPosition, profile.Layout.AuthorPosition);
        _advPageNumbering.IsChecked = profile.Layout.PageNumbering;

        Select(_advCaptionPosition, profile.Layout.CaptionPosition);
        _advCaptionArea.Value = profile.Layout.CaptionAreaMm;
        _advCaptionLines.Value = profile.Captions.Lines;
        _advCaptionFont.Value = profile.Captions.FontSize;
        Select(_advCaptionAlignment, profile.Captions.Alignment);
        foreach (var (name, check) in _advFields)
        {
            check.IsChecked = profile.Captions.Fields.Contains(name) || (name == "gps" && profile.Captions.ShowGps);
        }

        _themeMode.SelectedIndex = Math.Max(0, Choices.ThemeModes.ToList().IndexOf(profile.Theme.Mode));
        _advBackground.Text = profile.Theme.Custom.Background ?? "";
        _advText.Text = profile.Theme.Custom.Text ?? "";
        _advAccent.Text = profile.Theme.Custom.Accent ?? "";
        _advBorder.Text = profile.Theme.Custom.Border ?? "";
        UpdateAdvancedSensitivity();
    }

    private async Task OpenAdvancedSettingsAsync()
    {
        if (_dialogOpen)
        {
            return;
        }
        if (_advDialog is null)
        {
            var root = new Grid { RowSpacing = 8 };
            root.RowDefinitions.Add(new RowDefinition { Height = Ui.Star() });
            root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            Ui.Place(root, new ScrollViewer
            {
                Content = _advContent,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            }, row: 0);
            Ui.Place(root, _advStatus, row: 1);
            _advDialog = new ContentDialog
            {
                Title = _t.T("adv.dialog_title"),
                Content = root,
                CloseButtonText = _t.T("adv.dialog_close"),
                DefaultButton = ContentDialogButton.Close,
            };
            // A ContentDialog is narrow by default; these settings need room.
            _advDialog.Resources["ContentDialogMaxWidth"] = 960.0;
            _advDialog.Resources["ContentDialogMaxHeight"] = 900.0;
        }
        _advStatus.Text = "";
        _advDialog.XamlRoot = Content.XamlRoot;
        _dialogOpen = true;
        try
        {
            await _advDialog.ShowAsync();
        }
        finally
        {
            _dialogOpen = false;
        }
    }

    /// <summary>
    /// A message of the advanced settings (profile saved, preview…): in the
    /// log, and in the status line of the dialog, the only one visible while
    /// it is open.
    /// </summary>
    private void AdvancedNotify(string message)
    {
        AppendLog(message);
        _advStatus.Text = message;
    }

    private async Task SaveProfileAsync()
    {
        var path = await PickFileAsync(save: true, _t.T("adv.save_profile_dialog_title"));
        if (path is null)
        {
            return;
        }
        if (!path.EndsWith(".json", StringComparison.OrdinalIgnoreCase))
        {
            path += ".json";
        }
        try
        {
            await File.WriteAllTextAsync(path, ProfileFromControls().ToJson(), new UTF8Encoding(false));
            AdvancedNotify(_t.T("adv.profile_saved", ("path", path)));
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            AdvancedNotify(_t.T("adv.profile_save_error", ("error", e.Message)));
        }
    }

    /// <summary>The engine reads and checks the file (an invalid value falls back to its default, as on Linux).</summary>
    private async Task LoadProfileAsync()
    {
        var path = await PickFileAsync(save: false, _t.T("adv.load_profile_dialog_title"));
        if (path is null)
        {
            return;
        }
        try
        {
            var result = await _engine.RunAsync(["profile", "--load", path]);
            if (result.Events.OfType<ProfileRead>().FirstOrDefault() is { } read)
            {
                ApplyProfileToControls(Profile.FromJson(read.Profile));
                AdvancedNotify(_t.T("adv.profile_loaded", ("path", path)));
            }
            else
            {
                AdvancedNotify(_t.T(Core.Localization.WinStrings.EngineError, ("error", LastLine(result))));
            }
        }
        catch (EngineStartException e)
        {
            AdvancedNotify(_t.T(Core.Localization.WinStrings.EngineNotStarted, ("error", e.Message)));
        }
    }

    /// <summary>The system dialog to save or open a profile (.json); null when cancelled.</summary>
    private async Task<string?> PickFileAsync(bool save, string title)
    {
        try
        {
            if (save)
            {
                var picker = new Windows.Storage.Pickers.FileSavePicker { SuggestedFileName = "profil", CommitButtonText = title };
                WinRT.Interop.InitializeWithWindow.Initialize(picker, Handle);
                picker.FileTypeChoices.Add("JSON", [".json"]);
                return (await picker.PickSaveFileAsync())?.Path;
            }
            var open = new Windows.Storage.Pickers.FileOpenPicker { CommitButtonText = title };
            WinRT.Interop.InitializeWithWindow.Initialize(open, Handle);
            open.FileTypeFilter.Add(".json");
            return (await open.PickSingleFileAsync())?.Path;
        }
        catch (COMException)
        {
            return null;
        }
    }

    // ------------------------------------------------------ sheet preview

    /// <summary>
    /// The first sheet as the generation will draw it, made by the engine
    /// (<c>generate --preview-output</c>) and shown in a window of its own.
    /// </summary>
    private async Task ShowSheetPreviewAsync()
    {
        var request = ReadForm();
        if (request.InputDir.Length == 0 || !Directory.Exists(request.InputDir))
        {
            AdvancedNotify(_t.T("gen.select_input_first"));
            return;
        }
        _previewRun?.Kill();
        var (window, image, status) = OpenPreviewWindow();
        status.Text = _t.T("adv.preview_generating");

        GenerationFiles files;
        try
        {
            files = GenerationFiles.Write(request);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            status.Text = _t.T("adv.preview_error", ("error", e.Message));
            return;
        }
        var png = TempFile.NewPath("planche-contact-preview-", ".png");
        try
        {
            var lines = new List<string>();
            var errors = new List<string>();
            EngineRun run;
            try
            {
                run = _engine.Start(["generate", .. request.PreviewArguments(files, png)],
                    line => { lock (lines) { lines.Add(line); } },
                    line => { lock (errors) { errors.Add(line); } });
            }
            catch (EngineStartException e)
            {
                status.Text = _t.T(Core.Localization.WinStrings.EngineNotStarted, ("error", e.Message));
                return;
            }
            _previewRun = run;
            var code = await run.Completion;
            if (run.Killed || !ReferenceEquals(window, _previewWindow))
            {
                return; // Closed, or replaced by a newer preview.
            }
            var shown = lines.Select(GenerationLine.Preview).OfType<(int Shown, int Total)>().LastOrDefault();
            if (code == 0 && File.Exists(png))
            {
                image.Source = await Ui.BitmapAsync(await File.ReadAllBytesAsync(png));
                status.Text = _t.T("adv.preview_note", ("count", shown.Shown));
            }
            else if (code == 0)
            {
                status.Text = _t.T("adv.preview_no_photos");
            }
            else
            {
                var error = errors.LastOrDefault(line => line.Trim().Length > 0)?.Trim() ?? _t.T("adv.preview_empty_result");
                status.Text = _t.T("adv.preview_error", ("error", error));
            }
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or COMException)
        {
            status.Text = _t.T("adv.preview_error", ("error", e.Message));
        }
        finally
        {
            files.Dispose();
            TempFile.Delete(png);
        }
    }

    /// <summary>A window for the preview (replacing an earlier one): its image and its status line.</summary>
    private (Window Window, Image Image, TextBlock Status) OpenPreviewWindow()
    {
        _previewWindow?.Close();
        var window = new Window { Title = _t.T("adv.preview_title"), SystemBackdrop = new MicaBackdrop() };
        var status = Ui.Text("", dim: true);
        var image = new Image { Stretch = Stretch.Uniform };
        var close = Ui.AccentButton(_t.T("adv.preview_close"));
        close.HorizontalAlignment = HorizontalAlignment.Right;
        close.Click += (_, _) => window.Close();

        var root = new Grid { Padding = new Thickness(16), RowSpacing = 10 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = Ui.Star() });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Ui.Place(root, status, row: 0);
        Ui.Place(root, new ScrollViewer
        {
            Content = image,
            ZoomMode = ZoomMode.Enabled,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        }, row: 1);
        Ui.Place(root, close, row: 2);
        window.Content = root;

        var scale = Scale;
        var area = DisplayArea.GetFromWindowId(window.AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var height = Math.Min((int)(940 * scale), area.Height);
        var width = Math.Min((int)(760 * scale), area.Width);
        window.AppWindow.Resize(new SizeInt32(width, height));
        window.AppWindow.Move(new PointInt32(area.X + (area.Width - width) / 2, area.Y + (area.Height - height) / 2));
        var icon = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(icon))
        {
            window.AppWindow.SetIcon(icon);
        }
        window.Closed += (_, _) =>
        {
            if (ReferenceEquals(_previewWindow, window))
            {
                _previewWindow = null;
                _previewRun?.Kill();
            }
        };
        _previewWindow = window;
        window.Activate();
        return (window, image, status);
    }
}
