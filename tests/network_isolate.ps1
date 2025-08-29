# Setup dummy HTTP server on a non-standard port to catch network attempts
Start-Job -ScriptBlock {
  try {
    $listener = New-Object System.Net.HttpListener
    $listener.Prefixes.Add("http://127.0.0.1:8888/")
    $listener.Start()
    Write-Host "Network monitoring server started on port 8888"
  } catch {
    Write-Error "Failed to start network monitoring server: $_"
    throw "Failed to start network monitoring server."
  }

  while ($listener.IsListening) {
    $context = $listener.GetContext()
    $request = $context.Request
    $response = $context.Response

    Write-Host "NETWORK VIOLATION: $($request.HttpMethod) request to $($request.Url)"

    $response.StatusCode = 503
    $buffer = [System.Text.Encoding]::UTF8.GetBytes("Network access blocked for offline test")
    $response.ContentLength64 = $buffer.Length
    $response.OutputStream.Write($buffer, 0, $buffer.Length)
    $response.OutputStream.Close()
  }
}

# Set proxy environment variables to route traffic through our blocking server
$env:HTTP_PROXY="http://127.0.0.1:8888"
$env:HTTPS_PROXY="http://127.0.0.1:8888"
$env:http_proxy="http://127.0.0.1:8888"
$env:https_proxy="http://127.0.0.1:8888"
$env:FTP_PROXY="http://127.0.0.1:8888"
$env:ftp_proxy="http://127.0.0.1:8888"
$env:NO_PROXY=""
$env:no_proxy=""
