import json
import os
import shutil
import subprocess


def load_animation_into_filmcraft(
    media_path: str,
    project_out: str,
    frame_png_out: str,
    final_mp4_out: str | None = None,
    extra_assets: list[str] | None = None,
) -> dict:
    os.makedirs(os.path.dirname(os.path.abspath(project_out)), exist_ok=True)
    os.makedirs(os.path.dirname(os.path.abspath(frame_png_out)), exist_ok=True)

    # 1. Import the primary overlay/animation track and any individual cutout layer PNGs into FilmCraft (.fcproj)
    abs_media = os.path.abspath(media_path)
    abs_proj = os.path.abspath(project_out)
    abs_frame = os.path.abspath(frame_png_out)

    import_files = [abs_media]
    if extra_assets:
        for extra in extra_assets:
            abs_extra = os.path.abspath(extra)
            if os.path.isfile(abs_extra) and abs_extra not in import_files:
                import_files.append(abs_extra)

    fc_bin = shutil.which("filmcraft-cli") or "filmcraft-cli"
    cmd_import = [
        fc_bin,
        "--save-as",
        abs_proj,
        "import",
        *import_files,
    ]
    import_out = subprocess.check_output(cmd_import, stderr=subprocess.STDOUT, timeout=30).decode("utf-8")

    # Create active timeline sequence from the imported clip so Program Monitor & timeline have the clip
    subprocess.run(
        [
            fc_bin,
            "--project",
            abs_proj,
            "--save",
            "exec",
            "file.newSequenceFromClip",
            "items=[1]",
        ],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=20,
    )

    # 2. Inspect the project & sequence
    inspect_raw = subprocess.check_output(
        [fc_bin, "--project", abs_proj, "inspect"],
        stderr=subprocess.STDOUT,
        timeout=20,
    ).decode("utf-8")

    # 3. Render a verification frame from FilmCraft's Program Monitor (or probe the imported media)
    probe_raw = subprocess.check_output(
        [fc_bin, "probe", abs_media],
        stderr=subprocess.STDOUT,
        timeout=20,
    ).decode("utf-8")

    try:
        subprocess.run(
            [
                fc_bin,
                "--project",
                abs_proj,
                "render",
                "--seconds",
                "0.25",
                "--out",
                abs_frame,
                "--allow-offline",
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=25,
        )
        if not os.path.exists(abs_frame):
            subprocess.run(
                [
                    fc_bin,
                    "--project",
                    abs_proj,
                    "exec",
                    "file.exportFrame",
                    f"path={abs_frame}",
                    "target=source",
                    "item=1",
                ],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=25,
            )
    except Exception:
        pass

    proj_exists = os.path.exists(abs_proj)
    frame_exists = os.path.exists(abs_frame)
    return {
        "importedToFilmCraft": bool(proj_exists and frame_exists),
        "filmcraftProject": abs_proj,
        "projectExists": proj_exists,
        "programFramePng": abs_frame,
        "frameExists": frame_exists,
        "mediaProbe": json.loads(probe_raw) if probe_raw.strip().startswith("{") else probe_raw,
        "importSummary": import_out.strip(),
        "inspectExcerpt": inspect_raw[:400],
    }


