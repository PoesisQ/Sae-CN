# Exercise Windows' real file-transfer protocol on the hosted runner's desktop.
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Specialized;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

public static class ShellFileGesture {
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int w, int h, uint flags);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point p);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();

    public static void Drop(IntPtr reader, string[] paths) {
        // Reserve two distinct areas even on the smallest runner display.
        if (!SetWindowPos(reader, new IntPtr(-1), 70, 0, 750, 500, 0x0040))
            throw new Exception("Cannot place reader");
        var dest = new Point(400, 230);
        if (WindowFromPoint(dest) != reader) throw new Exception("Reader does not own drop point");
        var files = new StringCollection();
        files.AddRange(paths);
        var data = new DataObject();
        data.SetFileDropList(files);

        using (var source = new Form())
        using (var accepted = new ManualResetEvent(false)) {
            source.FormBorderStyle = FormBorderStyle.None;
            source.StartPosition = FormStartPosition.Manual;
            source.Bounds = new Rectangle(4, 10, 46, 46);
            source.TopMost = true;
            source.Show();
            Application.DoEvents();
            Cursor.Position = new Point(24, 28);
            if (WindowFromPoint(Cursor.Position) != source.Handle)
                throw new Exception("Source does not own drag origin");
            source.GiveFeedback += (s, e) => {
                if (e.Effect == DragDropEffects.Copy && WindowFromPoint(Cursor.Position) == reader)
                    accepted.Set();
            };
            mouse_event(2, 0, 0, 0, UIntPtr.Zero);
            Application.DoEvents();
            var pointer = new Thread(() => {
                // OLE owns the modal loop. Move until it reports that the
                // reader accepted this actual CF_HDROP, then release the mouse.
                Thread.Sleep(120);
                for (int i = 0; i < 60 && !accepted.WaitOne(40); i++)
                    Cursor.Position = new Point(dest.X + i % 2, dest.Y);
                mouse_event(4, 0, 0, 0, UIntPtr.Zero);
            });
            pointer.IsBackground = true;
            pointer.Start();
            DragDropEffects result;
            try { result = source.DoDragDrop(data, DragDropEffects.Copy); }
            finally {
                pointer.Join();
                mouse_event(4, 0, 0, 0, UIntPtr.Zero);
            }
            if (!accepted.WaitOne(0) || result != DragDropEffects.Copy)
                throw new Exception("OLE file drop did not complete: " + result);
        }
    }
}
'@
[void][ShellFileGesture]::SetProcessDPIAware()
$root = Split-Path -Parent $PSScriptRoot
$output = [IO.Directory]::CreateDirectory($OutputDirectory).FullName
$env:SAE_READER_DATA_DIR = Join-Path $output 'profile'
$app = Start-Process -FilePath (Resolve-Path -LiteralPath $Exe).Path -ArgumentList '--cover=blue' -PassThru
function Wait-ReaderTitle([string]$prefix) {
    $deadline = (Get-Date).AddSeconds(12)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Reader exited before receiving the drop' }
        if ($app.MainWindowHandle -ne 0 -and $app.MainWindowTitle.StartsWith($prefix)) { return }
        Start-Sleep -Milliseconds 40
    } while ((Get-Date) -lt $deadline)
    throw "Reader did not open $prefix (window title: $($app.MainWindowTitle))"
}
try {
    Wait-ReaderTitle 'Sae Reader'
    $fixtures = Join-Path $root 'tests/fixtures'
    $first = Join-Path $fixtures 'core.pdf'
    $second = Join-Path $fixtures 'slides.pdf'
    [ShellFileGesture]::Drop($app.MainWindowHandle, @((Join-Path $fixtures 'manifest.json'), $first, $second))
    Wait-ReaderTitle 'core.pdf'
    Start-Sleep -Milliseconds 500
    $app.Refresh()
    if (-not $app.MainWindowTitle.StartsWith('core.pdf')) { throw 'More than one file opened from one drop' }
    [ShellFileGesture]::Drop($app.MainWindowHandle, @($second))
    Wait-ReaderTitle 'slides.pdf'
    @{ shell_drop_from_home = $true; shell_drop_while_reading = $true; first_supported_in_batch = $true } |
        ConvertTo-Json | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $output 'shell-drop.json')
} finally {
    if (!$app.HasExited) {
        [void]$app.CloseMainWindow()
        if (!$app.WaitForExit(3000)) { $app.Kill() }
    }
    $app.Dispose()
}
