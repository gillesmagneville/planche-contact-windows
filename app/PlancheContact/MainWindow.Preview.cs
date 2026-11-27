// SPDX-License-Identifier: GPL-3.0-only

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using PlancheContact.Core;
using PlancheContact.Core.Engine;

namespace PlancheContact;

/// <summary>
/// Thumbnails of the photos of the input folder, ten per page — the
/// <c>FolderPreviewController</c> of the Linux window. The list of photos and
/// the thumbnails come from the engine (<c>photos</c> and
/// <c>thumbnails</c> commands), so that RAW files are shown exactly as the
/// sheets will use them.
/// </summary>
public sealed partial class MainWindow
{
    private const int PreviewPageSize = 10;
    private const int PreviewThumbSize = 90;

    private readonly TextBlock _previewStatus = Ui.Text("", dim: true);
    private readonly StackPanel _previewStrip = new() { Orientation = Orientation.Horizontal, Spacing = 8, Height = PreviewThumbSize };
    private readonly Button _previewPrevious = new() { IsEnabled = false };
    private readonly Button _previewNext = new() { IsEnabled = false };
    private readonly StackPanel _previewPages = new() { Orientation = Orientation.Horizontal, Spacing = 4, MinHeight = 32 };

    /// <summary>Raised at each refresh: an answer for an older folder or page is ignored.</summary>
    private int _previewToken;
    private List<string> _previewFiles = [];
    private int _previewPage;
    private readonly Dictionary<int, List<byte[]>> _previewCache = [];
    private EngineRun? _thumbnailRun;

    private StackPanel BuildPreviewStrip()
    {
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(_previewStatus);
        panel.Children.Add(new ScrollViewer
        {
            Content = _previewStrip,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            HorizontalScrollMode = ScrollMode.Auto,
            VerticalScrollMode = ScrollMode.Disabled,
            Height = PreviewThumbSize + 14,
        });

        _previewPrevious.Content = _t.T("preview.prev");
        _previewNext.Content = _t.T("preview.next");
        _previewPrevious.Click += (_, _) =>
        {
            if (_previewPage > 0)
            {
                LoadPreviewPage(_previewPage - 1);
            }
        };
        _previewNext.Click += (_, _) =>
        {
            if (_previewPage < Pagination.PageCount(_previewFiles.Count, PreviewPageSize) - 1)
            {
                LoadPreviewPage(_previewPage + 1);
            }
        };
        var navigation = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center };
        navigation.Children.Add(_previewPrevious);
        navigation.Children.Add(_previewPages);
        navigation.Children.Add(_previewNext);
        panel.Children.Add(navigation);
        return panel;
    }

    /// <summary>
    /// Shows the photos of <paramref name="folder"/>. <paramref name="files"/>
    /// (relative to it) replaces the listing of the folder: the selection of a
    /// context menu, shown as it is.
    /// </summary>
    private async void RefreshPreview(string folder, IReadOnlyList<string>? files = null)
    {
        var token = ++_previewToken;
        _thumbnailRun?.Kill();
        _previewFiles = [];
        _previewPage = 0;
        _previewCache.Clear();
        _previewStrip.Children.Clear();
        UpdatePreviewNavigation();
        UpdateSubfolderControls();

        if (folder.Length == 0 || !Directory.Exists(folder))
        {
            _previewStatus.Text = "";
            return;
        }

        if (files is not null)
        {
            var comparer = OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal;
            ApplyPreviewFiles(token, files
                .Select(relative => Path.Combine(folder, relative.Replace('/', Path.DirectorySeparatorChar)))
                .Where(File.Exists)
                .Order(comparer)
                .ToList());
            return;
        }

        _previewStatus.Text = _t.T("preview.analyzing");
        List<string> photos;
        try
        {
            var result = await _engine.RunAsync(["photos", "--input", folder]);
            photos = result.Events.OfType<PhotoFound>().Select(photo => photo.Path).ToList();
        }
        catch (EngineStartException e)
        {
            if (token == _previewToken)
            {
                _previewStatus.Text = _t.T(Core.Localization.WinStrings.EngineNotStarted, ("error", e.Message));
            }
            return;
        }
        ApplyPreviewFiles(token, photos);
    }

    private void ApplyPreviewFiles(int token, List<string> files)
    {
        if (token != _previewToken)
        {
            return; // A more recent folder was chosen in the meantime.
        }
        _previewFiles = files;
        if (files.Count == 0)
        {
            _previewStatus.Text = _t.T("preview.no_photos");
            UpdatePreviewNavigation();
            return;
        }
        LoadPreviewPage(0);
    }

    private void UpdatePreviewNavigation()
    {
        _previewPages.Children.Clear();
        var total = Pagination.PageCount(_previewFiles.Count, PreviewPageSize);
        if (total <= 1)
        {
            _previewPrevious.IsEnabled = false;
            _previewNext.IsEnabled = false;
            return;
        }
        var current = _previewPage + 1;
        foreach (var page in Pagination.Sequence(current, total))
        {
            if (page == Pagination.Gap)
            {
                _previewPages.Children.Add(Ui.Text("…", dim: true));
                continue;
            }
            var button = new Button { Content = page.ToString(System.Globalization.CultureInfo.InvariantCulture), MinWidth = 36 };
            if (page == current)
            {
                button = Ui.AccentButton(button.Content.ToString()!);
                button.MinWidth = 36;
                button.IsHitTestVisible = false;
            }
            else
            {
                var target = page - 1;
                button.Click += (_, _) => LoadPreviewPage(target);
            }
            _previewPages.Children.Add(button);
        }
        _previewPrevious.IsEnabled = _previewPage > 0;
        _previewNext.IsEnabled = _previewPage < total - 1;
    }

    private async void LoadPreviewPage(int page)
    {
        _previewPage = page;
        var token = _previewToken;
        var total = _previewFiles.Count;
        var start = page * PreviewPageSize;
        var end = Math.Min(start + PreviewPageSize, total);
        UpdatePreviewNavigation();

        // A page already made (going back notably) is shown at once.
        if (_previewCache.TryGetValue(page, out var cached))
        {
            await ShowThumbnailsAsync(cached, start, end, total);
            return;
        }

        _previewStrip.Children.Clear();
        _previewStatus.Text = _t.T("preview.loading", ("start", start + 1), ("end", end), ("total", total));

        var thumbnails = await MakeThumbnailsAsync(_previewFiles.GetRange(start, end - start));
        if (token != _previewToken || page != _previewPage || thumbnails is null)
        {
            return; // Another folder or another page in the meantime.
        }
        _previewCache[page] = thumbnails;
        await ShowThumbnailsAsync(thumbnails, start, end, total);
    }

    /// <summary>PNG thumbnails of <paramref name="photos"/> made by the engine; the unreadable ones are left out.</summary>
    private async Task<List<byte[]>?> MakeThumbnailsAsync(List<string> photos)
    {
        _thumbnailRun?.Kill();
        var folder = TempFile.NewPath("planche-contact-thumbs-", "");
        var list = TempFile.WriteLines("planche-contact-thumbs-", photos);
        try
        {
            var lines = new List<string>();
            var run = _engine.Start(
                ["thumbnails", "--size", PreviewThumbSize.ToString(System.Globalization.CultureInfo.InvariantCulture),
                 "--output-dir", folder, "--input-list", list],
                line => { lock (lines) { lines.Add(line); } });
            _thumbnailRun = run;
            await run.Completion;
            if (run.Killed)
            {
                return null;
            }
            var thumbnails = new List<byte[]>();
            foreach (var made in lines.Select(EngineEvent.Parse).OfType<ThumbnailMade>().Where(t => t.Ok))
            {
                thumbnails.Add(await File.ReadAllBytesAsync(made.Output!));
            }
            return thumbnails;
        }
        catch (Exception e) when (e is EngineStartException or IOException or UnauthorizedAccessException)
        {
            _previewStatus.Text = e.Message;
            return null;
        }
        finally
        {
            TempFile.Delete(list, folder);
        }
    }

    private async Task ShowThumbnailsAsync(List<byte[]> thumbnails, int start, int end, int total)
    {
        _previewStrip.Children.Clear();
        _previewStatus.Text = thumbnails.Count < end - start
            ? _t.T("preview.showing_partial", ("start", start + 1), ("end", end), ("total", total), ("shown", thumbnails.Count))
            : _t.T("preview.showing", ("start", start + 1), ("end", end), ("total", total));
        foreach (var bytes in thumbnails)
        {
            try
            {
                _previewStrip.Children.Add(new Image
                {
                    Source = await Ui.BitmapAsync(bytes),
                    Width = PreviewThumbSize,
                    Height = PreviewThumbSize,
                    Stretch = Stretch.Uniform,
                });
            }
            catch (Exception)
            {
                // One image that cannot be shown does not stop the others.
            }
        }
    }
}
