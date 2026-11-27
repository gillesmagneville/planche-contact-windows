// SPDX-License-Identifier: GPL-3.0-only

using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using PlancheContact.Core;
using PlancheContact.Core.Engine;
using PlancheContact.Core.Localization;
using Windows.Graphics;

namespace PlancheContact;

/// <summary>
/// The main window: the Windows counterpart of the Linux window
/// (<c>planche-contact-gtk.py</c> of planche-contact-linux), with the same
/// tabs (Generation, Help, About), the same settings, the same texts and the
/// same behaviour. It never decodes a photo itself: the engine
/// (<c>engine\planche-contact-engine.exe</c>, engine/ of this repository)
/// does all the work, in its own process.
///
/// Built in code rather than in XAML, split over several files:
///   MainWindow.cs          frame, Generation tab, generation itself
///   MainWindow.Preview.cs  thumbnails of the input folder
///   MainWindow.Advanced.cs advanced settings, profiles, sheet preview
///   MainWindow.Dialogs.cs  subfolders, preferences, messages, Help, About
/// </summary>
public sealed partial class MainWindow : Window
{
    private readonly Settings _settings = Settings.Load();
    private readonly Translator _t;
    private readonly string? _unsupportedLanguage;
    private readonly EngineRunner _engine = new(EngineCommand.Default());
    private readonly AppArguments _arguments;

    /// <summary>
    /// Language passed to the engine (--language). Follows the preference at
    /// once, while the texts of the window change at the next start — as in
    /// the Linux window.
    /// </summary>
    private string _language;

    // State of the form (as in the Linux window).
    private IReadOnlyList<string>? _fileSelection;
    private IReadOnlyList<string>? _subfolderSelection;
    private int _subfolderTotal;
    private bool _suppressInputChanged;
    private Results? _lastResult;

    // The generation in progress.
    private EngineRun? _generation;
    private bool _closeConfirmed;

    // Text of the log, shown in batches so that a long generation does not
    // redraw the window for every line.
    private readonly StringBuilder _log = new();
    private readonly DispatcherQueueTimer _logTimer;
    private readonly DispatcherQueueTimer _inputTimer;
    private readonly DispatcherQueueTimer _outputTimer;

    // ---------------------------------------------------------- controls

    private readonly Grid _generationTab = new();
    private readonly FrameworkElement _helpTab;
    private readonly FrameworkElement _aboutTab;

    private readonly TextBox _title = new();
    private readonly TextBox _author = new();
    private readonly TextBox _input = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    private readonly TextBox _output = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    private readonly StackPanel _selectionRow = new() { Orientation = Orientation.Horizontal, Spacing = 12, Visibility = Visibility.Collapsed };
    private readonly TextBlock _selectionLabel = Ui.Text("", dim: true);
    private readonly CheckBox _recursive = new();
    private readonly Button _subfoldersButton = new();
    private readonly TextBlock _subfoldersLabel = Ui.Text("", dim: true);
    private readonly ComboBox _perSheet;
    private readonly ComboBox _pageFormat;
    private readonly TextBox _watermark = new();
    private readonly ComboBox _watermarkOrientation;
    private readonly Slider _opacity = new() { Minimum = 0, Maximum = 100, StepFrequency = 1, Value = Choices.DefaultWatermarkOpacity, MinWidth = 200 };
    private readonly ComboBox _themeMode;
    private readonly CheckBox _pdf = new() { IsChecked = true };
    private readonly CheckBox _html = new() { IsChecked = true };
    private readonly CheckBox _csv = new() { IsChecked = true };
    private readonly Slider _htmlPerPage = new()
    {
        Minimum = Choices.HtmlPerPageMin, Maximum = Choices.HtmlPerPageMax, StepFrequency = Choices.HtmlPerPageStep,
        SnapsTo = SliderSnapsTo.StepValues, Value = Choices.DefaultHtmlPerPage, MinWidth = 200,
    };
    private readonly CheckBox _htmlOriginals = new();

    private readonly Button _runButton;
    private readonly Button _cancelButton = new() { Visibility = Visibility.Collapsed };
    private readonly DropDownButton _viewResults = new() { IsEnabled = false };
    private readonly MenuFlyoutItem _viewSheets = new();
    private readonly MenuFlyoutItem _viewPdf = new();
    private readonly MenuFlyoutItem _viewGallery = new();
    private readonly MenuFlyoutItem _viewCsv = new();
    private readonly TextBlock _status = Ui.Text("");
    private readonly ProgressBar _progress = new() { Minimum = 0, Maximum = 100, Value = 0 };
    private readonly TextBlock _logText = new()
    {
        TextWrapping = TextWrapping.Wrap,
        IsTextSelectionEnabled = true,
        FontFamily = new FontFamily("Cascadia Mono, Consolas"),
        FontSize = 12,
    };
    private readonly ScrollViewer _logScroll = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        MinHeight = 120,
    };

    public MainWindow(AppArguments arguments)
    {
        _arguments = arguments;
        (_language, _unsupportedLanguage) = Translator.Resolve(_settings.Language);
        _t = Translator.For(_language);

        Title = "Planche-Contact";
        SystemBackdrop = new MicaBackdrop();
        SetUpWindowFrame();

        _perSheet = Ui.Combo(_t.Header("gen.images_per_sheet"),
            Choices.PerSheet.Select(n => n.ToString(System.Globalization.CultureInfo.InvariantCulture)),
            IndexOf(Choices.PerSheet, Choices.DefaultPerSheet));
        _pageFormat = Ui.Combo(_t.Header("gen.sheet_format"), Choices.PageFormats,
            IndexOf(Choices.PageFormats, Choices.DefaultPageFormat));
        _watermarkOrientation = Ui.Combo(_t.Header("gen.watermark_orientation"),
            Choices.WatermarkOrientations.Select(o => _t.T(o.Key)));
        _themeMode = Ui.Combo(_t.Header("gen.theme_label"), Choices.ThemeModes.Select(m => _t.T($"gen.theme_mode.{m}")));
        _runButton = Ui.AccentButton(_t.T("gen.run"));

        _logTimer = NewTimer(150, ShowLog);
        _inputTimer = NewTimer(400, () => RefreshPreview(_input.Text.Trim()));
        _outputTimer = NewTimer(400, () => CheckExistingResults(_output.Text.Trim()));

        BuildGenerationTab();
        BuildAdvancedSettings();
        _helpTab = BuildHelpTab();
        _aboutTab = BuildAboutTab();
        Content = BuildFrame();

        // At startup: the file name under each photo (interface_default_profile()).
        ApplyProfileToControls(Profile.InterfaceDefault());

        AppWindow.Closing += OnWindowClosing;
        Closed += (_, _) => CleanUp();
        if (Content is FrameworkElement root)
        {
            root.Loaded += async (_, _) => await OnFirstShownAsync();
        }
    }

    private static int IndexOf<T>(IReadOnlyList<T> list, T value) => Math.Max(0, list.ToList().IndexOf(value));

    private DispatcherQueueTimer NewTimer(int milliseconds, Action tick)
    {
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(milliseconds);
        timer.IsRepeating = false;
        timer.Tick += (_, _) => tick();
        return timer;
    }

    /// <summary>Runs <paramref name="action"/> on the thread of the window (engine lines arrive on other threads).</summary>
    private void OnWindowThread(Action action) => DispatcherQueue.TryEnqueue(() => action());

    // ------------------------------------------------------------- frame

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    private IntPtr Handle => WinRT.Interop.WindowNative.GetWindowHandle(this);

    private double Scale
    {
        get
        {
            var dpi = GetDpiForWindow(Handle);
            return dpi == 0 ? 1.0 : dpi / 96.0;
        }
    }

    /// <summary>Initial size (in the scale of the screen), minimum size and icon.</summary>
    private void SetUpWindowFrame()
    {
        var scale = Scale;
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        // As large as the form needs, never larger than the screen.
        var width = Math.Min((int)(1080 * scale), area.Width);
        var height = Math.Min((int)(900 * scale), area.Height);
        AppWindow.Resize(new SizeInt32(width, height));
        AppWindow.Move(new PointInt32(area.X + (area.Width - width) / 2, area.Y + (area.Height - height) / 2));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = (int)(640 * scale);
            presenter.PreferredMinimumHeight = (int)(520 * scale);
        }
        var icon = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(icon))
        {
            AppWindow.SetIcon(icon);
        }
    }

    private Grid BuildFrame()
    {
        var root = new Grid();
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = Ui.Star() });

        // The tabs, and the Preferences button at the end of their row (the
        // menu button of the Linux title bar).
        var bar = new SelectorBar { Margin = new Thickness(16, 4, 0, 0) };
        var tabs = new (string Key, FrameworkElement Content)[]
        {
            ("tab.generation", _generationTab), ("tab.help", _helpTab), ("tab.about", _aboutTab),
        };
        foreach (var (key, _) in tabs)
        {
            bar.Items.Add(new SelectorBarItem { Text = _t.T(key) });
        }
        var content = new Grid();
        foreach (var (_, panel) in tabs)
        {
            panel.Visibility = Visibility.Collapsed;
            content.Children.Add(panel);
        }
        bar.SelectionChanged += (sender, _) =>
        {
            var index = sender.Items.IndexOf(sender.SelectedItem);
            for (var i = 0; i < tabs.Length; i++)
            {
                tabs[i].Content.Visibility = Ui.ShownIf(i == index);
            }
        };
        bar.SelectedItem = bar.Items[0];
        tabs[0].Content.Visibility = Visibility.Visible;

        var preferences = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 16 },
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 4, 16, 0),
        };
        ToolTipService.SetToolTip(preferences, _t.T("prefs.title"));
        AutomationProperties.SetName(preferences, _t.T("prefs.title"));
        preferences.Click += async (_, _) => await OpenPreferencesAsync();

        var header = new Grid();
        header.Children.Add(bar);
        header.Children.Add(preferences);
        Ui.Place(root, header, row: 0);
        Ui.Place(root, content, row: 1);
        return root;
    }

    // -------------------------------------------------- Generation tab

    private void BuildGenerationTab()
    {
        // Two parts, as in the Linux window: the form, which scrolls on a small
        // screen, and the bottom (buttons, status, progress, log), always
        // visible.
        _generationTab.Padding = new Thickness(24, 12, 24, 16);
        _generationTab.RowSpacing = 10;
        foreach (var height in new[] { Ui.Star(3), GridLength.Auto, GridLength.Auto, GridLength.Auto, Ui.Star(1) })
        {
            _generationTab.RowDefinitions.Add(new RowDefinition { Height = height });
        }

        var form = new StackPanel { Spacing = 14, Padding = new Thickness(0, 0, 16, 8) };
        var formScroll = new ScrollViewer
        {
            Content = form,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        Ui.Place(_generationTab, formScroll, row: 0);

        // Project title + author.
        var names = Ui.Columns(Ui.Star(5), Ui.Star(4));
        _title.Header = _t.Header("gen.project_title");
        _author.Header = _t.Header("gen.author");
        Ui.Place(names, _title, row: 0);
        Ui.Place(names, _author, row: 0, column: 1);
        form.Children.Add(names);

        // Input folder.
        _input.Header = _t.Header("gen.input_dir");
        _input.Text = _settings.LastInputDir;
        _input.TextChanged += (_, _) => OnInputChanged();
        form.Children.Add(FolderRow(_input, "input"));

        // Photos received from a context menu, and the way back to the whole folder.
        var clear = new Button { Content = _t.T("gen.selection_clear") };
        ToolTipService.SetToolTip(clear, _t.T("gen.selection_clear_tooltip"));
        clear.Click += (_, _) =>
        {
            SetFileSelection(null);
            RefreshPreview(_input.Text.Trim());
        };
        _selectionRow.Children.Add(_selectionLabel);
        _selectionRow.Children.Add(clear);
        form.Children.Add(_selectionRow);

        // Thumbnails of the input folder.
        form.Children.Add(BuildPreviewStrip());

        // Output folder.
        _output.Header = _t.Header("gen.output_dir");
        _output.Text = _settings.LastOutputDir;
        _output.TextChanged += (_, _) => Restart(_outputTimer);
        form.Children.Add(FolderRow(_output, "output"));

        form.Children.Add(BuildOptions());

        // Buttons.
        var buttons = new Grid();
        var left = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        _runButton.Click += (_, _) => StartGeneration();
        var advanced = new Button { Content = _t.T("adv.open_button") };
        ToolTipService.SetToolTip(advanced, _t.T("adv.open_button_tooltip"));
        advanced.Click += async (_, _) => await OpenAdvancedSettingsAsync();
        var reset = new Button { Content = _t.T("gen.reset") };
        reset.Click += (_, _) => ResetForm();
        _viewResults.Content = _t.T("gen.view_results");
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.Bottom };
        (_viewSheets.Text, _viewPdf.Text, _viewGallery.Text, _viewCsv.Text) =
            (_t.T("gen.view_planches"), _t.T("gen.view_pdf"), _t.T("gen.view_gallery"), _t.T("gen.view_csv"));
        _viewSheets.Click += async (_, _) => await OpenResultAsync(r => r.SheetsDir, "dialog.no_file_manager_title", "dialog.no_file_manager_detail");
        _viewPdf.Click += async (_, _) => await OpenResultAsync(r => r.PdfFile, "dialog.no_pdf_viewer_title", "dialog.no_pdf_viewer_detail");
        _viewGallery.Click += async (_, _) => await OpenResultAsync(r => r.GalleryIndex, "dialog.no_browser_title", "dialog.no_browser_detail");
        _viewCsv.Click += async (_, _) => await OpenResultAsync(r => r.CsvFile, "dialog.no_csv_app_title", "dialog.no_csv_app_detail");
        foreach (var item in new[] { _viewSheets, _viewPdf, _viewGallery, _viewCsv })
        {
            menu.Items.Add(item);
        }
        _viewResults.Flyout = menu;
        foreach (var element in new FrameworkElement[] { _runButton, advanced, reset, _viewResults })
        {
            left.Children.Add(element);
        }

        var right = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Right };
        _cancelButton.Content = _t.T(WinStrings.CancelGeneration);
        _cancelButton.Click += (_, _) => CancelGeneration();
        var quit = new Button { Content = _t.T("gen.quit") };
        quit.Click += async (_, _) => await RequestCloseAsync();
        right.Children.Add(_cancelButton);
        right.Children.Add(quit);
        buttons.Children.Add(left);
        buttons.Children.Add(right);
        Ui.Place(_generationTab, buttons, row: 1);

        Ui.Place(_generationTab, _status, row: 2);
        Ui.Place(_generationTab, _progress, row: 3);

        _logScroll.Content = _logText;
        var logBorder = new Border
        {
            Child = _logScroll,
            Padding = new Thickness(8),
            CornerRadius = new CornerRadius(4),
            BorderThickness = new Thickness(1),
        };
        if (Ui.TryGetResource("ControlStrokeColorDefaultBrush") is Brush stroke)
        {
            logBorder.BorderBrush = stroke;
        }
        Ui.Place(_generationTab, logBorder, row: 4);
    }

    /// <summary>A folder field with its "Choose..." button.</summary>
    private Grid FolderRow(TextBox field, string kind)
    {
        var row = Ui.Columns(Ui.Star(), GridLength.Auto);
        var choose = new Button { Content = _t.T("gen.choose"), VerticalAlignment = VerticalAlignment.Bottom };
        choose.Click += async (_, _) =>
        {
            if (await PickFolderAsync() is { } folder)
            {
                field.Text = folder;
            }
        };
        AutomationProperties.SetName(choose, $"{_t.T("gen.choose")} {(kind == "input" ? _t.Header("gen.input_dir") : _t.Header("gen.output_dir"))}");
        Ui.Place(row, field, row: 0);
        Ui.Place(row, choose, row: 0, column: 1);
        return row;
    }

    private Grid BuildOptions()
    {
        var grid = Ui.Columns(Ui.Star(), Ui.Star(), Ui.Star(), Ui.Star());
        Ui.Rows(grid, 5);

        // Recursive search and the choice of subfolders.
        var recursiveRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        _recursive.Content = _t.T("gen.recursive");
        _recursive.Click += (_, _) => OnRecursiveToggled();
        _subfoldersButton.Content = _t.T("gen.subfolders_button");
        ToolTipService.SetToolTip(_subfoldersButton, _t.T("gen.subfolders_tooltip"));
        _subfoldersButton.Click += async (_, _) => await ChooseSubfoldersAsync();
        recursiveRow.Children.Add(_recursive);
        recursiveRow.Children.Add(_subfoldersButton);
        recursiveRow.Children.Add(_subfoldersLabel);
        Ui.Place(grid, recursiveRow, row: 0, columnSpan: 4);
        UpdateSubfolderControls();

        Ui.Place(grid, _perSheet, row: 1);
        Ui.Place(grid, _pageFormat, row: 1, column: 1);
        ToolTipService.SetToolTip(_themeMode, _t.T("gen.theme_tooltip"));
        Ui.Place(grid, _themeMode, row: 1, column: 2);

        _watermark.Header = _t.Header("gen.watermark_text");
        Ui.Place(grid, _watermark, row: 2);
        Ui.Place(grid, _watermarkOrientation, row: 2, column: 1);
        Ui.Place(grid, SliderWithValue(_opacity, _t.Header("gen.watermark_opacity"), " %"), row: 2, column: 2, columnSpan: 2);

        // What to produce.
        _pdf.Content = _t.T("gen.generate_pdf");
        _csv.Content = _t.T("gen.generate_csv");
        _html.Content = _t.T("gen.generate_html");
        _htmlOriginals.Content = _t.T("gen.html_originals");
        ToolTipService.SetToolTip(_htmlOriginals, _t.T("gen.html_originals_tooltip"));
        _html.Click += (_, _) => _htmlOriginals.IsEnabled = _html.IsChecked == true;
        var outputs = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 24 };
        outputs.Children.Add(_pdf);
        outputs.Children.Add(_csv);
        Ui.Place(grid, outputs, row: 3, columnSpan: 4);

        Ui.Place(grid, _html, row: 4);
        Ui.Place(grid, SliderWithValue(_htmlPerPage, _t.Header("gen.images_per_page"), ""), row: 4, column: 1);
        _htmlOriginals.VerticalAlignment = VerticalAlignment.Bottom;
        Ui.Place(grid, _htmlOriginals, row: 4, column: 2, columnSpan: 2);
        _html.VerticalAlignment = VerticalAlignment.Bottom;
        return grid;
    }

    /// <summary>A slider with its value written next to it, as the Linux window shows it.</summary>
    private static Grid SliderWithValue(Slider slider, string header, string unit)
    {
        var grid = Ui.Columns(Ui.Star(), GridLength.Auto);
        slider.Header = header;
        AutomationProperties.SetName(slider, header);
        var value = Ui.Text($"{slider.Value:0}{unit}");
        value.VerticalAlignment = VerticalAlignment.Bottom;
        value.Margin = new Thickness(0, 0, 0, 8);
        value.MinWidth = 40;
        slider.ValueChanged += (_, e) => value.Text = $"{e.NewValue:0}{unit}";
        Ui.Place(grid, slider, row: 0);
        Ui.Place(grid, value, row: 0, column: 1);
        return grid;
    }

    private static void Restart(DispatcherQueueTimer timer)
    {
        timer.Stop();
        timer.Start();
    }

    // ------------------------------------------- input folder and selection

    private void OnInputChanged()
    {
        if (_suppressInputChanged)
        {
            return;
        }
        // Another input folder: a selection of photos no longer applies, nor
        // the choice of subfolders.
        if (_fileSelection is not null)
        {
            SetFileSelection(null);
        }
        ResetSubfolderSelection();
        Restart(_inputTimer);
    }

    /// <summary>
    /// Photos received from a context menu (relative paths), or null for the
    /// whole input folder. The recursive search makes no sense for a
    /// selection: it is switched off and locked.
    /// </summary>
    private void SetFileSelection(IReadOnlyList<string>? files)
    {
        _fileSelection = files;
        var active = files is not null;
        _selectionRow.Visibility = Ui.ShownIf(active);
        if (active)
        {
            _selectionLabel.Text = _t.T("gen.selection_note", ("count", files!.Count));
            _recursive.IsChecked = false;
        }
        _recursive.IsEnabled = !active;
        UpdateSubfolderControls();
    }

    private void OnRecursiveToggled()
    {
        if (_recursive.IsChecked != true)
        {
            _subfolderSelection = null;
            _subfolderTotal = 0;
        }
        UpdateSubfolderControls();
    }

    private void ResetSubfolderSelection()
    {
        _subfolderSelection = null;
        _subfolderTotal = 0;
        UpdateSubfolderControls();
    }

    /// <summary>The button is active only with a recursive search on a valid folder; the label says what will be processed.</summary>
    private void UpdateSubfolderControls()
    {
        var recursive = _recursive.IsChecked == true;
        var input = _input.Text.Trim();
        _subfoldersButton.IsEnabled = recursive && input.Length > 0 && Directory.Exists(input) && _fileSelection is null;
        _subfoldersLabel.Text = !recursive ? ""
            : _subfolderSelection is null ? _t.T("gen.subfolders_all")
            : _t.T("gen.subfolders_selected", ("selected", _subfolderSelection.Count), ("total", _subfolderTotal));
    }

    /// <summary>Started from a context menu, or from the Start menu with folders remembered.</summary>
    private async Task OnFirstShownAsync()
    {
        if (_arguments.HasPaths)
        {
            await ApplyLaunchPathsAsync();
        }
        else
        {
            RefreshPreview(_input.Text.Trim());
            CheckExistingResults(_output.Text.Trim());
        }

        if (_unsupportedLanguage is not null)
        {
            await ShowMessageAsync(_t.T("lang.fallback_warning_title"),
                _t.T("lang.fallback_warning_detail", ("detected", _unsupportedLanguage)));
        }
    }

    /// <summary>
    /// Puts the folder or the photos received from the context menu in the
    /// form. What they designate is decided by the engine
    /// (engine/src/launch.rs): one folder alone is the whole folder;
    /// photos, several folders or a mix give their common folder and the exact
    /// list of photos.
    /// </summary>
    private async Task ApplyLaunchPathsAsync()
    {
        SelectionResolved? selection;
        try
        {
            var result = await _engine.RunAsync(["selection", .. _arguments.Arguments]);
            selection = result.Events.OfType<SelectionResolved>().FirstOrDefault();
            if (selection is null)
            {
                AppendLog(_t.T(WinStrings.EngineError, ("error", LastLine(result))));
            }
        }
        catch (EngineStartException e)
        {
            AppendLog(_t.T(WinStrings.EngineNotStarted, ("error", e.Message)));
            return;
        }
        if (selection?.InputDir is not { } inputDir)
        {
            AppendLog(_t.T("launch.nothing_usable"));
            return;
        }

        _suppressInputChanged = true;
        try
        {
            _input.Text = inputDir;
        }
        finally
        {
            _suppressInputChanged = false;
        }
        ResetSubfolderSelection();
        SetFileSelection(selection.Files);

        // The results go in the folder that was selected: the output folder
        // remembered from an earlier session would otherwise get the sheets
        // of an unrelated folder (as in the Linux window).
        _output.Text = inputDir;

        AppendLog(selection.Files is null
            ? _t.T("launch.log_folder", ("dir", inputDir))
            : _t.T("launch.log_selection", ("count", selection.Files.Count), ("dir", inputDir)));

        _inputTimer.Stop();
        RefreshPreview(inputDir, selection.Files);
        CheckExistingResults(inputDir);
    }

    // ----------------------------------------------------------- results

    /// <summary>Enables "View results" for what exists in the output folder (from this generation or an earlier one).</summary>
    private void CheckExistingResults(string outputDir)
    {
        var found = Results.Check(outputDir);
        _lastResult = found.Any ? found : null;
        _viewSheets.IsEnabled = found.Sheets;
        _viewPdf.IsEnabled = found.Pdf;
        _viewGallery.IsEnabled = found.Gallery;
        _viewCsv.IsEnabled = found.Csv;
        _viewResults.IsEnabled = found.Any;
    }

    /// <summary>Opens a result with the default application of Windows; says so when there is none.</summary>
    private async Task OpenResultAsync(Func<Results, string> path, string titleKey, string detailKey)
    {
        if (_lastResult is null)
        {
            return;
        }
        var target = path(_lastResult);
        if (!File.Exists(target) && !Directory.Exists(target))
        {
            return;
        }
        if (!OpenWithDefaultApp(target))
        {
            await ShowMessageAsync(_t.T(titleKey), _t.T(detailKey));
        }
    }

    private static bool OpenWithDefaultApp(string path)
    {
        try
        {
            Process.Start(new ProcessStartInfo { FileName = path, UseShellExecute = true })?.Dispose();
            return true;
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException
            or FileNotFoundException)
        {
            return false;
        }
    }

    // -------------------------------------------------------- generation

    /// <summary>What the form asks for, as the engine will receive it.</summary>
    private GenerationRequest ReadForm() => new()
    {
        InputDir = _input.Text.Trim(),
        OutputDir = _output.Text.Trim(),
        PerSheet = Choices.PerSheet[Math.Max(0, _perSheet.SelectedIndex)],
        PageFormat = Choices.PageFormats[Math.Max(0, _pageFormat.SelectedIndex)],
        Title = _title.Text,
        Author = _author.Text,
        Watermark = _watermark.Text,
        WatermarkOrientation = Choices.WatermarkOrientations[Math.Max(0, _watermarkOrientation.SelectedIndex)].Value,
        WatermarkOpacity = (int)Math.Round(_opacity.Value),
        Recursive = _recursive.IsChecked == true,
        IncludeDirs = _subfolderSelection,
        Files = _fileSelection,
        Pdf = _pdf.IsChecked == true,
        Html = _html.IsChecked == true,
        HtmlOriginals = _htmlOriginals.IsChecked == true,
        Csv = _csv.IsChecked == true,
        HtmlPerPage = Choices.SnapHtmlPerPage(_htmlPerPage.Value),
        Language = _language,
        Profile = ProfileFromControls(),
    };

    private void StartGeneration()
    {
        if (_generation is not null)
        {
            return;
        }
        var request = ReadForm();
        if (request.InputDir.Length == 0)
        {
            AppendLog(_t.T("gen.select_input_first"));
            return;
        }
        _settings.LastInputDir = request.InputDir;
        _settings.LastOutputDir = request.EffectiveOutputDir;
        _settings.Save();

        GenerationFiles files;
        try
        {
            files = GenerationFiles.Write(request);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            AppendLog(_t.T("gen.error_log_exception", ("error", e.Message)));
            return;
        }

        AppendLog(_t.T("gen.starting"));
        _runButton.IsEnabled = false;
        _viewResults.IsEnabled = false;
        _cancelButton.Visibility = Visibility.Visible;
        _progress.Value = 0;
        _status.Text = _t.T("gen.running");

        EngineRun run;
        try
        {
            run = _engine.Start(["generate", .. request.GenerateArguments(files)],
                line => OnWindowThread(() => OnGenerationLine(line)),
                line => OnWindowThread(() => OnGenerationLine(line)));
        }
        catch (EngineStartException e)
        {
            files.Dispose();
            AppendLog(_t.T(WinStrings.EngineNotStarted, ("error", e.Message)));
            _status.Text = _t.T("gen.error_status_generic");
            GenerationEnded();
            return;
        }
        _generation = run;
        var outputDir = request.EffectiveOutputDir;
        run.Completion.ContinueWith(task => OnWindowThread(() =>
        {
            files.Dispose();
            OnGenerationDone(run, task.IsCompletedSuccessfully ? task.Result : -1, outputDir);
        }), TaskScheduler.Default);
    }

    /// <summary>Each line of the engine goes to the log and the status; PROGRESS:X/100 moves the bar.</summary>
    private void OnGenerationLine(string raw)
    {
        var line = raw.Trim();
        if (line.Length == 0)
        {
            return;
        }
        AppendLog(line);
        _status.Text = line;
        if (GenerationLine.Progress(line) is { } progress)
        {
            _progress.Value = progress.Percent;
        }
    }

    private void OnGenerationDone(EngineRun run, int exitCode, string outputDir)
    {
        if (run.Killed)
        {
            AppendLog(_t.T(WinStrings.Cancelled));
            _status.Text = _t.T(WinStrings.Cancelled);
        }
        else if (exitCode == 0)
        {
            AppendLog(_t.T("gen.success_log"));
            _status.Text = _t.T("gen.success_status");
            _progress.Value = 100;
        }
        else
        {
            AppendLog(_t.T("gen.error_log_code", ("code", exitCode)));
            _status.Text = _t.T("gen.error_status");
        }
        GenerationEnded();
        CheckExistingResults(outputDir);
    }

    private void GenerationEnded()
    {
        _generation = null;
        _runButton.IsEnabled = true;
        _cancelButton.Visibility = Visibility.Collapsed;
    }

    private void CancelGeneration() => _generation?.Kill();

    private void ResetForm()
    {
        _input.Text = "";
        _output.Text = "";
        _title.Text = "";
        _author.Text = "";
        _watermark.Text = "";
        _recursive.IsChecked = false;
        _perSheet.SelectedIndex = IndexOf(Choices.PerSheet, Choices.DefaultPerSheet);
        _pageFormat.SelectedIndex = IndexOf(Choices.PageFormats, Choices.DefaultPageFormat);
        _watermarkOrientation.SelectedIndex = 0;
        _opacity.Value = Choices.DefaultWatermarkOpacity;
        _htmlPerPage.Value = Choices.DefaultHtmlPerPage;
        _pdf.IsChecked = true;
        _html.IsChecked = true;
        _htmlOriginals.IsChecked = false;
        _htmlOriginals.IsEnabled = true;
        _csv.IsChecked = true;
        ApplyProfileToControls(Profile.InterfaceDefault());
        ResetSubfolderSelection();
        SetFileSelection(null);
        _log.Clear();
        _logText.Text = "";
        _status.Text = "";
        _progress.Value = 0;
        _lastResult = null;
        _viewResults.IsEnabled = false;
    }

    private static string LastLine(EngineResult result) =>
        result.Errors.LastOrDefault(line => line.Trim().Length > 0)?.Trim()
        ?? result.Events.OfType<FatalError>().LastOrDefault()?.Message
        ?? $"exit code {result.ExitCode}";

    // --------------------------------------------------------------- log

    private void AppendLog(string text)
    {
        _log.Append(text).Append('\n');
        if (!_logTimer.IsRunning)
        {
            _logTimer.Start();
        }
    }

    /// <summary>Shows the text accumulated so far and scrolls to its end.</summary>
    private void ShowLog()
    {
        _logText.Text = _log.ToString();
        _logScroll.UpdateLayout();
        _logScroll.ChangeView(null, _logScroll.ScrollableHeight, null, true);
    }

    // ----------------------------------------------------------- closing

    private async void OnWindowClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_generation is not null && !_closeConfirmed)
        {
            // Not now: ask first, and close for good if confirmed.
            args.Cancel = true;
            await RequestCloseAsync();
        }
    }

    /// <summary>The "Quit" button, and the close button during a generation.</summary>
    private async Task RequestCloseAsync()
    {
        if (_generation is not null && !await ConfirmAsync(
                _t.T(WinStrings.QuitRunningTitle), _t.T(WinStrings.QuitRunningDetail), _t.T(WinStrings.QuitAnyway)))
        {
            return;
        }
        _closeConfirmed = true;
        Close();
    }

    /// <summary>Stops whatever the engine is still doing and deletes the temporary files.</summary>
    private void CleanUp()
    {
        _generation?.Kill();
        _thumbnailRun?.Kill();
        _previewRun?.Kill();
        _previewWindow?.Close();
    }
}
