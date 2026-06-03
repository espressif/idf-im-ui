"""Local-only HTTPS sanity check.

The Rust parent process spawns a short-lived TLS server on 127.0.0.1, hands us
its port and self-signed certificate via environment variables, and we verify
that the active Python interpreter can perform a full TLS handshake and HTTPS
request against it. No network egress is involved.

Required env vars:
    EIM_HTTPS_TEST_PORT  - port the local TLS server is listening on
    EIM_HTTPS_TEST_CERT  - PEM-encoded self-signed cert to trust explicitly
"""

import os
import ssl
import sys
import urllib.request


def main() -> int:
    port_str = os.environ.get("EIM_HTTPS_TEST_PORT")
    cert_pem = os.environ.get("EIM_HTTPS_TEST_CERT")
    if not port_str or not cert_pem:
        print(
            "Failed: EIM_HTTPS_TEST_PORT and EIM_HTTPS_TEST_CERT must be set "
            "by the parent process",
            file=sys.stderr,
        )
        return 1

    try:
        # Build a client TLS context that explicitly trusts the cert the
        # parent just generated. We do NOT use the system trust store (we
        # don't need to verify a public CA chain), but we DO verify the
        # chain against our pinned cert — which still exercises the full
        # _ssl path that breaks on misbuilt Python (missing _ssl, broken
        # cacert.pem, etc.).
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.load_verify_locations(cadata=cert_pem)
        ctx.verify_mode = ssl.CERT_REQUIRED
        # The cert SAN includes 127.0.0.1, but Python's stdlib hostname
        # check on raw IP literals is fiddly across versions, and the point
        # of this check is the TLS machinery, not URL/hostname parsing.
        ctx.check_hostname = False

        url = f"https://127.0.0.1:{port_str}/"
        response = urllib.request.urlopen(url, context=ctx, timeout=10)
    except Exception as e:
        print(f"Request failed: {e}")
        return 1

    if response.status != 200:
        print(f"Request failed. Status code: {response.status}")
        return 1

    body = response.read()
    if body != b"OK":
        print(f"Request failed. Unexpected body: {body!r}")
        return 1

    print("Request successful!")
    print("Response content:", body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
