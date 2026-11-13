# Window screenshot helper for visual checks on Windows (see AGENTS.md, "Подводные камни").
#
# Captures the main window of a running process to a PNG; before that it can resize the window,
# send keys and click or drag with the mouse.
# Comments are ASCII on purpose: Windows PowerShell 5 reads a BOM-less file in the ANSI code page.
#
# Usage (from the repository root, the example already running):
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/shot.ps1 -Name widgets -Out target\shots\1.png
#   ... -Keys "{TAB}{END}"            send keys, then capture
#   ... -Size "740x300"               resize the window (outer size, physical pixels)
#   ... -Click "250,120"              left click at a point
#   ... -Drag "250,90,250,200"        press at the first point, move to the second, release
#   ... -Hold "250,230,1500"          press at a point, keep the button down for N ms, release
#   ... -Wheel "100,150,-3"           rotate the wheel over a point: notches, negative = down
#
# -Name   process name without ".exe" (widgets, widget_edit, demo, ...)
# -Out    path of the PNG to write; its directory is created if missing
# -Keys   System.Windows.Forms.SendKeys string ("{TAB}", "{ENTER}", "abc", "^s" for Ctrl+S)
# -Size   "WIDTHxHEIGHT" of the whole window
# -Click  "X,Y"            points are pixels from the top-left corner of the window,
# -Drag   "X1,Y1,X2,Y2"    i.e. the same coordinates as in the captured PNG
# -Hold   "X,Y,MILLISECONDS"
# -Wheel  "X,Y,NOTCHES"
#
# Order: resize, keys, click, drag, hold, wheel, capture. Keys and mouse are sent only when the target window
# really is in the foreground; the mouse cursor is put back afterwards.
#
# Prints "saved <path> <W>x<H> foreground=<bool> ..." and exits 0;
# exits 2 with "no window for <name>" while the process has no visible window yet - poll until it does.
param(
	[Parameter(Mandatory = $true)][string]$Name,
	[Parameter(Mandatory = $true)][string]$Out,
	[string]$Keys = "",
	[string]$Size = "",
	[string]$Click = "",
	[string]$Drag = "",
	[string]$Hold = "",
	[string]$Wheel = ""
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class GtermShotWin {
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
	[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
	[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
	[DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr processId);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint from, uint to, bool attach);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
	[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
	[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
	[DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
	[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
	[DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
	[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
	[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
	[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
"@
$MouseLeftDown = 0x0002
$MouseLeftUp = 0x0004
$MouseWheel = 0x0800

# Physical pixels: without this the rectangle is DPI-virtualized and the picture is blurred.
[GtermShotWin]::SetProcessDPIAware() | Out-Null

$process = Get-Process -Name $Name -ErrorAction SilentlyContinue |
	Where-Object { $_.MainWindowHandle -ne 0 } |
	Select-Object -First 1
if (-not $process) {
	Write-Output "no window for $Name"
	exit 2
}
$handle = $process.MainWindowHandle

# The handle appears before the window is shown and sized: a hidden or tiny window is "not yet".
$rect = New-Object GtermShotWin+RECT
[GtermShotWin]::GetWindowRect($handle, [ref]$rect) | Out-Null
if (-not [GtermShotWin]::IsWindowVisible($handle) -or ($rect.R - $rect.L) -lt 64 -or ($rect.B - $rect.T) -lt 64) {
	Write-Output "no window for $Name"
	exit 2
}

if ($Size) {
	$parts = $Size -split "x"
	[GtermShotWin]::MoveWindow($handle, $rect.L, $rect.T, [int]$parts[0], [int]$parts[1], $true) | Out-Null
	Start-Sleep -Milliseconds 500
	[GtermShotWin]::GetWindowRect($handle, [ref]$rect) | Out-Null
}

# CopyFromScreen grabs what is on screen, so the window has to be on top.
[GtermShotWin]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 500
$isForeground = ([GtermShotWin]::GetForegroundWindow() -eq $handle)
if (-not $isForeground) {
	# Windows refuses the foreground while another application is active. Joining the input
	# queue of the current foreground thread lifts that restriction without sending anything
	# to that application.
	$foreground = [GtermShotWin]::GetForegroundWindow()
	$foregroundThread = [GtermShotWin]::GetWindowThreadProcessId($foreground, [IntPtr]::Zero)
	$ownThread = [GtermShotWin]::GetCurrentThreadId()
	[GtermShotWin]::AttachThreadInput($ownThread, $foregroundThread, $true) | Out-Null
	[GtermShotWin]::ShowWindow($handle, 9) | Out-Null
	[GtermShotWin]::BringWindowToTop($handle) | Out-Null
	[GtermShotWin]::SetForegroundWindow($handle) | Out-Null
	[GtermShotWin]::AttachThreadInput($ownThread, $foregroundThread, $false) | Out-Null
	Start-Sleep -Milliseconds 500
	$isForeground = ([GtermShotWin]::GetForegroundWindow() -eq $handle)
}

# Never type or click into somebody else's window.
if ($Keys -and $isForeground) {
	[System.Windows.Forms.SendKeys]::SendWait($Keys)
	Start-Sleep -Milliseconds 600
}

# Is the target window the one under a point given relative to its top-left corner?
# A window of another application lying over that point would swallow the mouse input.
function Test-Hit([int]$x, [int]$y) {
	$point = New-Object GtermShotWin+POINT
	$point.X = $rect.L + $x
	$point.Y = $rect.T + $y
	$under = [GtermShotWin]::GetAncestor([GtermShotWin]::WindowFromPoint($point), 2)
	return ($under -eq $handle)
}
$hits = ""

if (($Click -or $Drag -or $Hold -or $Wheel) -and $isForeground) {
	$saved = New-Object GtermShotWin+POINT
	[GtermShotWin]::GetCursorPos([ref]$saved) | Out-Null
	foreach ($spec in @($Click, $Drag, $Hold, $Wheel)) {
		if ($spec) {
			$p = $spec -split ","
			$hits += "$(Test-Hit ([int]$p[0]) ([int]$p[1])) "
		}
	}
	if ($Click) {
		$p = $Click -split ","
		[GtermShotWin]::SetCursorPos($rect.L + [int]$p[0], $rect.T + [int]$p[1]) | Out-Null
		Start-Sleep -Milliseconds 150
		[GtermShotWin]::mouse_event($MouseLeftDown, 0, 0, 0, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 80
		[GtermShotWin]::mouse_event($MouseLeftUp, 0, 0, 0, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 400
	}
	if ($Drag) {
		$p = $Drag -split ","
		$x1 = $rect.L + [int]$p[0]; $y1 = $rect.T + [int]$p[1]
		$x2 = $rect.L + [int]$p[2]; $y2 = $rect.T + [int]$p[3]
		[GtermShotWin]::SetCursorPos($x1, $y1) | Out-Null
		Start-Sleep -Milliseconds 150
		[GtermShotWin]::mouse_event($MouseLeftDown, 0, 0, 0, [UIntPtr]::Zero)
		# Several steps: the application has to see the pointer moving while the button is held.
		for ($step = 1; $step -le 8; $step++) {
			$x = $x1 + [int](($x2 - $x1) * $step / 8)
			$y = $y1 + [int](($y2 - $y1) * $step / 8)
			[GtermShotWin]::SetCursorPos($x, $y) | Out-Null
			Start-Sleep -Milliseconds 40
		}
		[GtermShotWin]::mouse_event($MouseLeftUp, 0, 0, 0, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 400
	}
	if ($Hold) {
		$p = $Hold -split ","
		[GtermShotWin]::SetCursorPos($rect.L + [int]$p[0], $rect.T + [int]$p[1]) | Out-Null
		Start-Sleep -Milliseconds 150
		[GtermShotWin]::mouse_event($MouseLeftDown, 0, 0, 0, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds ([int]$p[2])
		[GtermShotWin]::mouse_event($MouseLeftUp, 0, 0, 0, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 400
	}
	if ($Wheel) {
		$p = $Wheel -split ","
		[GtermShotWin]::SetCursorPos($rect.L + [int]$p[0], $rect.T + [int]$p[1]) | Out-Null
		Start-Sleep -Milliseconds 150
		# One notch is 120 units; a negative amount travels as its two's complement.
		$amount = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$p[2] * 120), 0)
		[GtermShotWin]::mouse_event($MouseWheel, 0, 0, $amount, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 400
	}
	# Park the cursor off the window so hover does not change the picture, then capture below.
	[GtermShotWin]::SetCursorPos($saved.X, $saved.Y) | Out-Null
	Start-Sleep -Milliseconds 200
}

[GtermShotWin]::GetWindowRect($handle, [ref]$rect) | Out-Null
$width = $rect.R - $rect.L
$height = $rect.B - $rect.T

$directory = Split-Path -Parent $Out
if ($directory -and -not (Test-Path $directory)) {
	New-Item -ItemType Directory -Path $directory | Out-Null
}

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.L, $rect.T, 0, 0, $bitmap.Size)
$bitmap.Save($Out)
$graphics.Dispose()
$bitmap.Dispose()
Write-Output "saved $Out ${width}x${height} foreground=$isForeground keys='$Keys' size='$Size' click='$Click' drag='$Drag' hold='$Hold' wheel='$Wheel' hit='$($hits.Trim())'"
