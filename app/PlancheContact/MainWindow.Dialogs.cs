// SPDX-License-Identifier: GPL-3.0-only

using System.Reflection;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using PlancheContact.Core;
using PlancheContact.Core.Engine;
using PlancheContact.Core.Localization;

namespace PlancheContact;

/// <summary>
/// Subfolders, preferences, messages, folder dialog, and the Help and About
/// tabs. WinUI shows one dialog at a time: <see cref="_dialogOpen"/> keeps a
/// second one from being opened over it.
/// </summary>
public sealed partial class MainWindow
{
    private const string AuthorName = "Gilles MAGNEVILLE";
    private const string RepositoryUrl = "https://github.com/gillesmagneville/planche-contact-windows";

    private bool _dialogOpen;

    // ---------------------------------------------------------- messages

    /// <summary>A message with an OK button; written to the log if another dialog is already shown.</summary>
    private async Task ShowMessageAsync(string title, string detail)
    {
        if (_dialogOpen)
        {
            AppendLog($"{title} - {detail}");
            return;
        }
        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = title,
            Content = new TextBlock { Text = detail, TextWrapping = TextWrapping.Wrap },
            CloseButtonText = _t.T(WinStrings.Ok),
            DefaultButton = ContentDialogButton.Close,
        };
        _dialogOpen = true;
        try
        {
            await dialog.ShowAsync();
        }
        finally
        {
            _dialogOpen = false;
        }
    }

    /// <summary>A question; true when the user confirms with <paramref name="confirm"/>.</summary>
    private async Task<bool> ConfirmAsync(string title, string detail, string confirm)
    {
        if (_dialogOpen)
        {
            return false;
        }
        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = title,
            Content = new TextBlock { Text = detail, TextWrapping = TextWrapping.Wrap },
            PrimaryButtonText = confirm,
            CloseButtonText = _t.T(WinStrings.Cancel),
            DefaultButton = ContentDialogButton.Close,
        };
        _dialogOpen = true;
        try
        {
            return await dialog.ShowAsync() == ContentDialogResult.Primary;
        }
        finally
        {
            _dialogOpen = false;
        }
    }

    /// <summary>The system folder dialog; null when the user cancels.</summary>
    private async Task<string?> PickFolderAsync()
    {
        try
        {
            var picker = new Windows.Storage.Pickers.FolderPicker
            {
                SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.PicturesLibrary,
            };
            // A desktop application must tell the dialog which window owns it.
            WinRT.Interop.InitializeWithWindow.Initialize(picker, Handle);
            picker.FileTypeFilter.Add("*");
            var folder = await picker.PickSingleFolderAsync();
            return folder?.Path;
        }
        catch (COMException)
        {
            return null;
        }
    }

    // -------------------------------------------------------- subfolders

    /// <summary>
    /// The folders of the input folder that hold photos (listed by the
    /// engine's <c>folders</c> command), to tick those to include. All ticked
    /// means no restriction.
    /// </summary>
    private async Task ChooseSubfoldersAsync()
    {
        var input = _input.Text.Trim();
        if (input.Length == 0 || !Directory.Exists(input))
        {
            AppendLog(_t.T("gen.select_input_first"));
            return;
        }
        if (_dialogOpen)
        {
            return;
        }
        var output = _output.Text.Trim().Length > 0 ? _output.Text.Trim() : Path.Combine(input, "Portfolio");

        var status = Ui.Text(_t.T("subfolders.scanning"), dim: true);
        var list = new StackPanel { Spacing = 2 };
        var summary = Ui.Text("", dim: true);
        var all = new Button { Content = _t.T("subfolders.select_all") };
        var none = new Button { Content = _t.T("subfolders.select_none") };
        var ticks = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        ticks.Children.Add(all);
        ticks.Children.Add(none);
        ticks.Children.Add(summary);

        var panel = new StackPanel { Spacing = 10, MinWidth = 480 };
        panel.Children.Add(Ui.Text(_t.T("subfolders.intro")));
        panel.Children.Add(status);
        panel.Children.Add(new ScrollViewer { Content = list, MaxHeight = 340, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
        panel.Children.Add(ticks);

        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = _t.T("subfolders.title"),
            Content = panel,
            PrimaryButtonText = _t.T("subfolders.ok"),
            CloseButtonText = _t.T("subfolders.cancel"),
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };
        dialog.Resources["ContentDialogMaxWidth"] = 720.0;

        var entries = new List<(string Path, int Count, CheckBox Check)>();
        void Refresh()
        {
            var ticked = entries.Where(entry => entry.Check.IsChecked == true).ToList();
            summary.Text = _t.T("subfolders.summary", ("selected", ticked.Count), ("total", entries.Count),
                ("photos", ticked.Sum(entry => entry.Count)));
            dialog.IsPrimaryButtonEnabled = ticked.Count > 0;
        }
        void SetAll(bool value)
        {
            foreach (var entry in entries)
            {
                entry.Check.IsChecked = value;
            }
        }
        all.Click += (_, _) => SetAll(true);
        none.Click += (_, _) => SetAll(false);

        async Task PopulateAsync()
        {
            List<FolderFound> folders;
            try
            {
                var result = await _engine.RunAsync(["folders", "--input", input, "--output", output]);
                folders = result.Events.OfType<FolderFound>().ToList();
            }
            catch (EngineStartException e)
            {
                status.Text = _t.T(WinStrings.EngineNotStarted, ("error", e.Message));
                return;
            }
            status.Text = folders.Count == 0 ? _t.T("subfolders.none_found") : "";
            status.Visibility = Ui.ShownIf(folders.Count == 0);
            var current = _subfolderSelection;
            foreach (var folder in folders)
            {
                var name = folder.RelativePath == "." ? _t.T("subfolders.root_label") : folder.RelativePath;
                var check = new CheckBox
                {
                    Content = $"{name}  ({_t.T("subfolders.photo_count", ("count", folder.Count))})",
                    IsChecked = current is null || current.Contains(folder.RelativePath),
                };
                check.Checked += (_, _) => Refresh();
                check.Unchecked += (_, _) => Refresh();
                list.Children.Add(check);
                entries.Add((folder.RelativePath, folder.Count, check));
            }
            Refresh();
        }

        _dialogOpen = true;
        try
        {
            var showing = dialog.ShowAsync();
            await PopulateAsync();
            if (await showing != ContentDialogResult.Primary)
            {
                return;
            }
        }
        finally
        {
            _dialogOpen = false;
        }

        var chosen = entries.Where(entry => entry.Check.IsChecked == true).Select(entry => entry.Path).ToList();
        _subfolderSelection = chosen.Count == entries.Count ? null : chosen;
        _subfolderTotal = entries.Count;
        UpdateSubfolderControls();
    }

    // ------------------------------------------------------- preferences

    /// <summary>
    /// The language. The engine follows the choice at once; the texts of the
    /// window change at the next start, as in the Linux window.
    /// </summary>
    private async Task OpenPreferencesAsync()
    {
        if (_dialogOpen)
        {
            return;
        }
        var combo = Ui.Combo(_t.Header("about.language_label"), Translator.Preferences.Select(code => _t.T($"lang.{code}")),
            Math.Max(0, Translator.Preferences.ToList().IndexOf(_settings.Language)));
        var notice = Ui.Text(_t.T("about.language_restart_notice"), dim: true);
        notice.Visibility = Visibility.Collapsed;
        combo.SelectionChanged += (_, _) =>
        {
            var preference = Translator.Preferences[Math.Max(0, combo.SelectedIndex)];
            if (preference == _settings.Language)
            {
                return;
            }
            _settings.Language = preference;
            _settings.Save();
            _language = Translator.Resolve(preference).Language;
            notice.Visibility = Visibility.Visible;
        };
        var panel = new StackPanel { Spacing = 14, MinWidth = 320 };
        panel.Children.Add(combo);
        panel.Children.Add(notice);

        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = _t.T("prefs.title"),
            Content = panel,
            CloseButtonText = _t.T("adv.dialog_close"),
            DefaultButton = ContentDialogButton.Close,
        };
        _dialogOpen = true;
        try
        {
            await dialog.ShowAsync();
        }
        finally
        {
            _dialogOpen = false;
        }
    }

    // ---------------------------------------------------------- Help tab

    private FrameworkElement BuildHelpTab()
    {
        var panel = new StackPanel { Spacing = 16, Padding = new Thickness(32, 24, 32, 24), MaxWidth = 720, HorizontalAlignment = HorizontalAlignment.Left };
        panel.Children.Add(Ui.Title(_t.T("help.title")));
        panel.Children.Add(Ui.Text(_t.T("help.description")));
        var open = Ui.AccentButton(_t.T("help.open_manual"));
        open.Click += async (_, _) => await OpenManualAsync();
        panel.Children.Add(open);
        return new ScrollViewer { Content = panel };
    }

    /// <summary>
    /// The manual installed with the window (docs\), in the language of the
    /// interface; a page saying where it should be when it is missing.
    /// </summary>
    private async Task OpenManualAsync()
    {
        var manual = Installed.Manual(Path.Combine(AppContext.BaseDirectory, "docs"), _t.Language);
        if (manual is null)
        {
            var text = _t.T("help.manual_fallback", ("path", "<code>docs\\planche-contact-manual.html</code>"));
            manual = TempFile.Write("planche-contact-manual-", ".html",
                $"<!DOCTYPE html>\n<html lang=\"{_t.Language}\"><head><meta charset=\"utf-8\"></head><body><h1>Planche-Contact</h1>\n<p>{text}</p>\n</body></html>\n");
        }
        if (!OpenWithDefaultApp(manual))
        {
            await ShowMessageAsync(_t.T("dialog.no_browser_title"), _t.T("dialog.no_browser_detail"));
        }
    }

    // --------------------------------------------------------- About tab

    private FrameworkElement BuildAboutTab()
    {
        var panel = new StackPanel { Spacing = 8, Padding = new Thickness(32, 40, 32, 24), HorizontalAlignment = HorizontalAlignment.Center };
        TextBlock Centered(TextBlock block, double top = 0)
        {
            block.TextAlignment = TextAlignment.Center;
            block.HorizontalAlignment = HorizontalAlignment.Center;
            block.Margin = new Thickness(0, top, 0, 0);
            return block;
        }

        panel.Children.Add(Centered(Ui.Title("Planche-Contact", "TitleTextBlockStyle")));
        panel.Children.Add(Centered(Ui.Text(VersionText()), top: 6));
        panel.Children.Add(Centered(Ui.Text(_t.T("about.tagline"), dim: true), top: 16));
        panel.Children.Add(Centered(Ui.Text(_t.T("about.developed_by", ("name", AuthorName))), top: 20));

        // "Designed with the help of {link} (Anthropic)": the link in the middle of the text.
        var credit = Centered(new TextBlock { TextWrapping = TextWrapping.Wrap });
        var template = _t.T("about.ai_credit");
        var at = template.IndexOf("{link}", StringComparison.Ordinal);
        if (at < 0)
        {
            credit.Text = template;
        }
        else
        {
            credit.Inlines.Add(new Run { Text = template[..at] });
            var link = new Hyperlink { NavigateUri = new Uri("https://claude.ai") };
            link.Inlines.Add(new Run { Text = "Claude.ai" });
            credit.Inlines.Add(link);
            credit.Inlines.Add(new Run { Text = template[(at + "{link}".Length)..] });
        }
        panel.Children.Add(credit);

        panel.Children.Add(Centered(Ui.Text(_t.T("about.license")), top: 8));
        panel.Children.Add(new HyperlinkButton
        {
            Content = "github.com/gillesmagneville/planche-contact-windows",
            NavigateUri = new Uri(RepositoryUrl),
            HorizontalAlignment = HorizontalAlignment.Center,
        });
        return new ScrollViewer { Content = panel };
    }

    /// <summary>"Version 5.3.0 — Build of 27/11/2026": the version of the program and the date of its file.</summary>
    private string VersionText()
    {
        var version = typeof(MainWindow).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
            ?? typeof(MainWindow).Assembly.GetName().Version?.ToString() ?? "?";
        // Without the "+commit" the build may append.
        var plus = version.IndexOf('+');
        if (plus > 0)
        {
            version = version[..plus];
        }
        var program = Environment.ProcessPath;
        if (program is not null && File.Exists(program))
        {
            return _t.T("about.version_build", ("version", version), ("date", Installed.FormatDate(File.GetLastWriteTime(program))));
        }
        return _t.T("about.version_only", ("version", version));
    }
}
