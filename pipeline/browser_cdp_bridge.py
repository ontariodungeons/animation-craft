import os
import subprocess
import time
import urllib.request

WORKDIR = "/home/jesse/Work/omhas-animation"
STUDIO_URL = "http://127.0.0.1:8765/"


def ensure_http_server(port: int = 8765) -> None:
    try:
        urllib.request.urlopen(f"http://127.0.0.1:{port}/api/health", timeout=1.0)
        return
    except Exception:
        pass
    # Replace any static http.server on 8765 with the full-stack Bun Web App server
    subprocess.run(["fuser", "-k", f"{port}/tcp"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    time.sleep(0.3)
    env = os.environ.copy()
    env["PORT"] = str(port)
    subprocess.Popen(
        ["bun", "run", "server.ts"],
        cwd=WORKDIR,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(0.8)


def open_studio_browser(
    headless: bool = False,
    port: int = 9225,
    screenshot_out: str | None = None,
    char_key: str = "hopper",
    angle_deg: int = 0,
) -> dict:
    ensure_http_server(8765)
    target_url = f"{STUDIO_URL}?char={char_key}&angle={angle_deg}&phase=0.35"
    if screenshot_out:
        os.makedirs(os.path.dirname(os.path.abspath(screenshot_out)), exist_ok=True)
        shot_args = [
            "chromium",
            "--no-sandbox",
            "--headless",
            "--use-gl=angle",
            "--use-angle=swiftshader",
            "--enable-webgl",
            "--ignore-gpu-blocklist",
            "--window-size=1680,1020",
            "--virtual-time-budget=4500",
            f"--screenshot={screenshot_out}",
            target_url,
        ]
        try:
            subprocess.run(shot_args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False, timeout=12)
        except subprocess.TimeoutExpired:
            pass

    if headless:
        return {
            "url": target_url,
            "screenshot": screenshot_out,
            "screenshotExists": bool(screenshot_out and os.path.exists(screenshot_out)),
        }

    args = [
        "chromium",
        "--no-sandbox",
        "--enable-webgl",
        "--ignore-gpu-blocklist",
        "--window-size=1680,1020",
        f"--remote-debugging-port={port}",
    ]
    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        args.extend(["--headless", "--use-gl=angle", "--use-angle=swiftshader"])
    args.append(target_url)

    proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.5)
    return {
        "pid": proc.pid,
        "url": target_url,
        "cdpPort": port,
        "screenshot": screenshot_out,
        "screenshotExists": bool(screenshot_out and os.path.exists(screenshot_out)),
        "visibleDesktopWindow": bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")),
    }
