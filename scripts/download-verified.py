"""Resumable HTTPS download with an independently supplied SHA256, no TLS bypass.

Used only for development bootstrap when the host's Schannel connection truncates.
"""
import hashlib
from pathlib import Path
import sys
import time
import requests

url, destination, expected = sys.argv[1:]
path = Path(destination)
for attempt in range(8):
    try:
        offset = path.stat().st_size if path.exists() else 0
        if offset and hashlib.sha256(path.read_bytes()).hexdigest() == expected.lower():
            print("SHA256 verified:", path.name, offset, flush=True)
            break
        headers = {"User-Agent": "SaeReader-build/0.1"}
        if offset:
            headers["Range"] = f"bytes={offset}-"
        with requests.get(url, stream=True, headers=headers, timeout=(20, 35)) as response:
            response.raise_for_status()
            if response.status_code != 206:
                offset = 0
            print("Download", path.name, "attempt", attempt + 1, "offset", offset, "length", response.headers.get("Content-Length"), flush=True)
            with path.open("ab" if offset else "wb") as output:
                for chunk in response.iter_content(64 * 1024):
                    output.write(chunk)
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected.lower():
            raise ValueError(f"SHA256 mismatch: {actual}; expected {expected}")
        print("SHA256 verified:", path.name, path.stat().st_size, flush=True)
        break
    except Exception as error:
        print(type(error).__name__, str(error)[:400], flush=True)
        if attempt == 7:
            raise
        time.sleep(1)
