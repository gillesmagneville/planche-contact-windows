// SPDX-License-Identifier: GPL-3.0-only

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using PlancheContact.Core.Localization;
using Windows.Storage.Streams;

namespace PlancheContact;

/// <summary>Small helpers to build the windows in code.</summary>
internal static class Ui
{
    /// <summary>
    /// A label of the Linux window used as the header of a WinUI control: the
    /// texts end with a colon ("Input folder:", "Dossier d'entrée :"), which a
    /// header above its field does not need.
    /// </summary>
    public static string Header(this Translator t, string key) => t.T(key).TrimEnd().TrimEnd(':').TrimEnd();

    public static void Place(Grid grid, FrameworkElement element, int row, int column = 0, int columnSpan = 1)
    {
        Grid.SetRow(element, row);
        Grid.SetColumn(element, column);
        Grid.SetColumnSpan(element, columnSpan);
        grid.Children.Add(element);
    }

    public static Grid Columns(params GridLength[] widths)
    {
        var grid = new Grid { ColumnSpacing = 12, RowSpacing = 12 };
        foreach (var width in widths)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = width });
        }
        return grid;
    }

    public static void Rows(Grid grid, int count)
    {
        for (var i = 0; i < count; i++)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        }
    }

    /// <summary>A row of controls side by side, vertically centred (a GTK horizontal box).</summary>
    public static StackPanel Line(double spacing = 10) =>
        new() { Orientation = Orientation.Horizontal, Spacing = spacing };

    /// <summary>
    /// The label of a field, on its left as in the Linux window; it names the
    /// field for screen readers.
    /// </summary>
    public static TextBlock Label(string text, FrameworkElement? target = null, double marginStart = 0)
    {
        var label = Text(text, wrap: false);
        label.Margin = new Thickness(marginStart, 0, 0, 0);
        if (target is not null)
        {
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(target, text.TrimEnd().TrimEnd(':').TrimEnd());
        }
        return label;
    }

    public static GridLength Star(double value = 1) => new(value, GridUnitType.Star);

    public static Visibility ShownIf(bool shown) => shown ? Visibility.Visible : Visibility.Collapsed;

    public static TextBlock Text(string text, bool dim = false, bool wrap = true)
    {
        var block = new TextBlock
        {
            Text = text,
            TextWrapping = wrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
        };
        if (dim && TryGetResource("TextFillColorSecondaryBrush") is Microsoft.UI.Xaml.Media.Brush brush)
        {
            block.Foreground = brush;
        }
        return block;
    }

    public static TextBlock Title(string text, string style = "SubtitleTextBlockStyle")
    {
        var block = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap };
        if (TryGetResource(style) is Style found)
        {
            block.Style = found;
        }
        else
        {
            block.FontWeight = Microsoft.UI.Text.FontWeights.SemiBold;
        }
        return block;
    }

    /// <summary>A standard resource of the WinUI controls, or null.</summary>
    public static object? TryGetResource(string key)
    {
        try
        {
            return Application.Current.Resources.TryGetValue(key, out var value) ? value : null;
        }
        catch (Exception)
        {
            // Purely cosmetic: the default look will do.
            return null;
        }
    }

    public static Button AccentButton(string text)
    {
        var button = new Button { Content = text };
        if (TryGetResource("AccentButtonStyle") is Style accent)
        {
            button.Style = accent;
        }
        return button;
    }

    public static ComboBox Combo(string header, IEnumerable<string> items, int selected = 0)
    {
        var combo = new ComboBox { Header = header, HorizontalAlignment = HorizontalAlignment.Stretch, MinWidth = 140 };
        foreach (var item in items)
        {
            combo.Items.Add(item);
        }
        combo.SelectedIndex = selected;
        return combo;
    }

    public static NumberBox Number(string header, double minimum, double maximum)
    {
        return new NumberBox
        {
            Header = header,
            Minimum = minimum,
            Maximum = maximum,
            SmallChange = 1,
            LargeChange = 5,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
            ValidationMode = NumberBoxValidationMode.InvalidInputOverwritten,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
    }

    /// <summary>The value of a number field, its minimum when it is empty.</summary>
    public static double ValueOf(NumberBox box) => double.IsNaN(box.Value) ? box.Minimum : box.Value;

    /// <summary>An image from the bytes of a PNG file.</summary>
    public static async Task<BitmapImage> BitmapAsync(byte[] bytes)
    {
        var bitmap = new BitmapImage();
        using var stream = new InMemoryRandomAccessStream();
        using (var writer = new DataWriter(stream.GetOutputStreamAt(0)))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        stream.Seek(0);
        await bitmap.SetSourceAsync(stream);
        return bitmap;
    }
}
