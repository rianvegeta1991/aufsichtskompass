# Zeichnet die PNG-Icons nach. Auf diesem Rechner gibt es keinen SVG-Renderer, deshalb
# wird das Motiv aus icon.svg hier mit System.Drawing nachgebaut.
# WICHTIG: Bei Aenderungen am Logo beide Stellen nachziehen - die SVG-Dateien und dieses Skript.
#
# Aufruf:  powershell -NoProfile -ExecutionPolicy Bypass -File icons.ps1

Add-Type -AssemblyName System.Drawing

$ordner = Split-Path -Parent $MyInvocation.MyCommand.Path

$karmin = [System.Drawing.Color]::FromArgb(198, 9, 59)
$petrol = [System.Drawing.Color]::FromArgb(47, 72, 88)
$weiss  = [System.Drawing.Color]::White

function Nadel {
    param(
        [System.Drawing.Graphics]$g,
        [double]$mitte,      # Mittelpunkt in Pixel
        [double]$umfang,     # Radius des Aussenrings in Pixel
        [bool]$ring = $true
    )
    $f = $umfang / 196.0     # Skalierung gegenueber dem SVG (r=196 bei 512)

    if ($ring) {
        $stift = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(72, $petrol.R, $petrol.G, $petrol.B)), ([single](14 * $f))
        $g.DrawEllipse($stift, [single]($mitte - $umfang), [single]($mitte - $umfang), [single](2 * $umfang), [single](2 * $umfang))
        $stift.Dispose()
    }

    # Nordspitze (Karmin) und Suedspitze (Petrol, halbtransparent)
    $nord = @(
        (New-Object System.Drawing.PointF ([single]$mitte, [single]($mitte - 188 * $f))),
        (New-Object System.Drawing.PointF ([single]($mitte + 60 * $f), [single]($mitte - 24 * $f))),
        (New-Object System.Drawing.PointF ([single]$mitte, [single]($mitte + 12 * $f))),
        (New-Object System.Drawing.PointF ([single]($mitte - 60 * $f), [single]($mitte - 24 * $f)))
    )
    $sued = @(
        (New-Object System.Drawing.PointF ([single]$mitte, [single]($mitte + 188 * $f))),
        (New-Object System.Drawing.PointF ([single]($mitte - 60 * $f), [single]($mitte + 24 * $f))),
        (New-Object System.Drawing.PointF ([single]$mitte, [single]($mitte - 12 * $f))),
        (New-Object System.Drawing.PointF ([single]($mitte + 60 * $f), [single]($mitte + 24 * $f)))
    )
    $pinselN = New-Object System.Drawing.SolidBrush $karmin
    $pinselS = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(158, $petrol.R, $petrol.G, $petrol.B))
    $g.FillPolygon($pinselN, [System.Drawing.PointF[]]$nord)
    $g.FillPolygon($pinselS, [System.Drawing.PointF[]]$sued)
    $pinselN.Dispose(); $pinselS.Dispose()

    # Nabe
    $r = 26 * $f
    $pinselW = New-Object System.Drawing.SolidBrush $weiss
    $g.FillEllipse($pinselW, [single]($mitte - $r), [single]($mitte - $r), [single](2 * $r), [single](2 * $r))
    $stiftK = New-Object System.Drawing.Pen $karmin, ([single](12 * $f))
    $g.DrawEllipse($stiftK, [single]($mitte - $r), [single]($mitte - $r), [single](2 * $r), [single](2 * $r))
    $pinselW.Dispose(); $stiftK.Dispose()
}

function Zeichne {
    param([int]$groesse, [string]$name, [double]$anteil, [bool]$rundeEcken)

    $bild = New-Object System.Drawing.Bitmap $groesse, $groesse
    $g = [System.Drawing.Graphics]::FromImage($bild)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear($weiss)

    if ($rundeEcken) {
        # Ecken transparent lassen: Radius wie im SVG (96 von 512).
        $r = [int]($groesse * 96 / 512)
        $pfad = New-Object System.Drawing.Drawing2D.GraphicsPath
        $pfad.AddArc(0, 0, 2 * $r, 2 * $r, 180, 90)
        $pfad.AddArc($groesse - 2 * $r, 0, 2 * $r, 2 * $r, 270, 90)
        $pfad.AddArc($groesse - 2 * $r, $groesse - 2 * $r, 2 * $r, 2 * $r, 0, 90)
        $pfad.AddArc(0, $groesse - 2 * $r, 2 * $r, 2 * $r, 90, 90)
        $pfad.CloseFigure()
        $bild2 = New-Object System.Drawing.Bitmap $groesse, $groesse
        $g2 = [System.Drawing.Graphics]::FromImage($bild2)
        $g2.SmoothingMode = 'AntiAlias'
        $g2.SetClip($pfad)
        $g2.Clear($weiss)
        $g.Dispose(); $bild.Dispose()
        $bild = $bild2; $g = $g2
        $pfad.Dispose()
    }

    Nadel -g $g -mitte ($groesse / 2.0) -umfang ($groesse * $anteil / 2.0) -ring $true
    $ziel = Join-Path $ordner $name
    $bild.Save($ziel, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bild.Dispose()
    Write-Output "$name geschrieben ($groesse x $groesse)"
}

# Normale Icons: Motiv auf 76 % - es bleibt Luft zum Rand.
Zeichne -groesse 192 -name 'icon-192.png' -anteil 0.76 -rundeEcken $true
Zeichne -groesse 512 -name 'icon-512.png' -anteil 0.76 -rundeEcken $true
# Maskable: 90 %, der Sicherheitskreis (Radius 40 %) ist damit fast ausgefuellt.
Zeichne -groesse 512 -name 'icon-512-maskable.png' -anteil 0.90 -rundeEcken $false
# Apple legt seine eigene Maske darueber: ohne runde Ecken, Motiv etwas groesser.
Zeichne -groesse 180 -name 'apple-touch-icon.png' -anteil 0.86 -rundeEcken $false
