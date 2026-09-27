param(
    [int]$Port = 8798
)

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://localhost:$Port/")
$listener.Start()
Write-Output "Serving $root on http://localhost:$Port/"

$mimeMap = @{
    ".html" = "text/html"
    ".htm"  = "text/html"
    ".css"  = "text/css"
    ".js"   = "application/javascript"
    ".json" = "application/json"
    ".webmanifest" = "application/manifest+json"
    ".svg"  = "image/svg+xml"
    ".png"  = "image/png"
    ".jpg"  = "image/jpeg"
    ".ico"  = "image/x-icon"
}

try {
    while ($listener.IsListening) {
        $context = $listener.GetContext()
        $request = $context.Request
        $response = $context.Response
        try {
            $path = $request.Url.AbsolutePath
            if ($path -eq "/") { $path = "/index.html" }
            $filePath = Join-Path $root ($path.TrimStart("/"))
            $fullRoot = (Resolve-Path $root).Path
            if ((Test-Path $filePath) -and ((Resolve-Path $filePath).Path.StartsWith($fullRoot))) {
                $ext = [System.IO.Path]::GetExtension($filePath)
                $contentType = $mimeMap[$ext]
                if (-not $contentType) { $contentType = "application/octet-stream" }
                $bytes = [System.IO.File]::ReadAllBytes($filePath)
                $response.ContentType = $contentType
                # Kein Zwischenspeichern beim Entwickeln: sonst haelt der Browser alte
                # Skripte und Datendateien fest und man prueft eine Fassung von vorhin.
                $response.Headers.Add("Cache-Control", "no-store, must-revalidate")
                $response.ContentLength64 = $bytes.Length
                $response.OutputStream.Write($bytes, 0, $bytes.Length)
            } else {
                $response.StatusCode = 404
                $notFound = [System.Text.Encoding]::UTF8.GetBytes("404 Not Found")
                $response.OutputStream.Write($notFound, 0, $notFound.Length)
            }
        } catch {
            $response.StatusCode = 500
        } finally {
            $response.OutputStream.Close()
        }
    }
} finally {
    $listener.Stop()
}
