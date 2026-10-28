"""Main CLI entry point for Magic Studio (`magic` / `magic`).

Built with the CLI-Anything methodology (Click + ReplSkin + stateful Session + MCP).
"""

from __future__ import annotations

import functools
import json
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

import click

from cli_anything.magic import __version__
from cli_anything.magic.core import export as export_mod
from cli_anything.magic.core import mcp_server
from cli_anything.magic.core import project as proj_mod
from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend
from cli_anything.magic.utils.repl_skin import ReplSkin


_SESSION: Session | None = None
_REPL_MODE: bool = False
_USE_JSON: bool = False
_DRY_RUN: bool = False
_BRIDGE: str | None = None


def get_session() -> Session:
    """Return the active global Session singleton."""
    global _SESSION
    if _SESSION is None:
        _SESSION = Session()
    return _SESSION


def parse_kv_args(kv_args: tuple[str, ...]) -> dict[str, Any]:
    """Parse `key=value` tokens into a typed JSON dictionary."""
    out: dict[str, Any] = {}
    for item in kv_args:
        if "=" not in item:
            continue
        k, v = item.split("=", 1)
        if v == "true":
            out[k] = True
        elif v == "false":
            out[k] = False
        else:
            try:
                out[k] = int(v)
            except ValueError:
                try:
                    out[k] = float(v)
                except ValueError:
                    try:
                        out[k] = json.loads(v)
                    except Exception:
                        out[k] = v
    return out


def emit(data: Any) -> None:
    """Print command output as formatted JSON."""
    click.echo(json.dumps(data, indent=2))


def handle_error(fn: Callable[..., Any]) -> Callable[..., Any]:
    """Consistent error handling decorator for Click commands."""

    @functools.wraps(fn)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        try:
            return fn(*args, **kwargs)
        except Exception as exc:
            if _USE_JSON:
                click.echo(json.dumps({"ok": False, "error": str(exc)}, indent=2))
            else:
                click.echo(f"magic: {exc}", err=True)
            if not _REPL_MODE:
                sys.exit(1)
            return None

    return wrapper


@click.group(invoke_without_command=True)
@click.option("--json", "use_json", is_flag=True, help="Output as machine-readable JSON")
@click.option("--project", "project_path", type=str, default=None, help="Path to .magic project file")
@click.option("--bridge", "bridge_addr", type=str, default=None, help="Explicit Studio bridge port or unix:<socket>")
@click.option("--dry-run", "dry_run", is_flag=True, default=False, help="Run command without saving changes to disk")
@click.version_option(version=__version__, prog_name="magic")
@click.pass_context
def cli(
    ctx: click.Context,
    use_json: bool,
    project_path: str | None,
    bridge_addr: str | None,
    dry_run: bool,
) -> None:
    """Magic Studio CLI (`magic`): drive headless projects and live Studio windows with one command hierarchy."""
    global _USE_JSON, _DRY_RUN, _BRIDGE
    _USE_JSON = use_json
    _DRY_RUN = dry_run
    _BRIDGE = bridge_addr

    sess = get_session()
    if project_path:
        resolved = proj_mod.resolve_project_path(project_path)
        if resolved.is_file():
            opened = proj_mod.open(resolved)
            sess.set_project(
                opened["document"],
                resolved,
                studio_endpoint=sess.studio_endpoint,
                studio_instance=sess.studio_instance,
            )
        else:
            sess.project_path = str(resolved)

    if ctx.invoked_subcommand is None:
        ctx.invoke(repl, project_path=project_path)


@cli.result_callback()
def auto_save_on_exit(
    result: Any,
    use_json: bool,
    project_path: str | None,
    bridge_addr: str | None,
    dry_run: bool,
    **kwargs: Any,
) -> None:
    """Auto-save session and project after one-shot mutations unless --dry-run is set."""
    if _REPL_MODE or dry_run:
        return
    sess = get_session()
    if sess.has_project() and sess._modified and sess.project_path:
        try:
            sess.save_session()
        except Exception as exc:
            click.echo(f"Warning: Auto-save failed: {exc}", err=True)


# ── REPL ─────────────────────────────────────────────────────────────────────


@cli.command("repl")
@click.option("--project", "project_path", type=str, default=None, help="Optional project to open")
def repl(project_path: str | None = None) -> None:
    """Start the interactive Magic Studio REPL with unified ReplSkin."""
    global _REPL_MODE
    _REPL_MODE = True
    sess = get_session()
    if project_path:
        resolved = proj_mod.resolve_project_path(project_path)
        if resolved.is_file():
            opened = proj_mod.open(resolved)
            sess.set_project(opened["document"], resolved)

    skin = ReplSkin("magic", version=__version__)
    skin.print_banner()
    pt_session = skin.create_prompt_session()

    commands_help = {
        "studio new | <project>": "Launch or focus Magic Studio GUI window and attach session",
        "studio list | status": "List running Studio windows and connection state",
        "project new | open | list | save | close | reveal | info": "Manage .magic projects",
        "storyboard create | read | update | delete | edit | open | render": "Storyboard CRUD & DeckCraft",
        "character list | import | configure | cut-sheet | build-rig": "2D/3D character rig pipeline",
        "motion tracks | clips | sample | keyframe | undo | path | undo-path": "Animation & Theatre motion",
        "video init | status | snapshot | render | place-track": "HyperFrames / Remotion video studio",
        "ui state | look | feedback | elements | click | set | key | screenshot": "Live Studio window controls",
        "session status | undo | redo | clear": "Inspect or clear bound session state",
        "exec <id> [key=value ...]": "Run any engine or UI command directly",
    }

    while True:
        proj_label = Path(sess.project_path).stem if sess.project_path else ""
        try:
            line = skin.get_input(
                pt_session,
                project_name=proj_label,
                modified=sess._modified,
            ).strip()
        except (EOFError, KeyboardInterrupt):
            break
        if not line:
            continue
        if line in {"quit", "exit", "q"}:
            break
        if line == "help":
            skin.help(commands_help)
            continue
        try:
            tokens = shlex.split(line)
            cli.main(args=tokens, prog_name="magic", standalone_mode=False)
        except SystemExit:
            pass
        except Exception as exc:
            skin.error(str(exc))

    skin.print_goodbye()


# ── STUDIO GROUP ─────────────────────────────────────────────────────────────


@cli.group()
def studio() -> None:
    """Launch, focus, or inspect live Magic Studio GUI windows."""


@studio.command("new")
@click.option("--animate", is_flag=True, help="Open focused Animation surface only")
@handle_error
def studio_new(animate: bool) -> None:
    """Launch a new Magic Studio window (`magic studio new`) and bind this session."""
    sess = get_session()
    live = backend.open_studio_window(None, animate_only=animate)
    sess.bind_studio(live)
    emit(
        {
            "ok": True,
            "active_project": live.get("active_project"),
            "endpoint": live.get("endpoint"),
            "instance": live.get("instance"),
            "identity": live.get("identity"),
            "ui_ready": live.get("ui_ready"),
        }
    )


@studio.command("open")
@click.argument("project_target", required=False)
@click.option("--animate", is_flag=True, help="Open focused Animation surface only")
@click.option("--character", type=str, default=None, help="Character ID to select")
@handle_error
def studio_open(
    project_target: str | None, animate: bool, character: str | None
) -> None:
    """Open or focus a project in Magic Studio (`magic studio open <project>`) and bind this session."""
    sess = get_session()
    target = project_target or sess.project_path
    resolved = str(proj_mod.resolve_project_path(target)) if target else None
    live = backend.open_studio_window(resolved, animate_only=animate, character=character)
    sess.bind_studio(live)
    emit(
        {
            "ok": True,
            "active_project": live.get("active_project"),
            "endpoint": live.get("endpoint"),
            "instance": live.get("instance"),
            "identity": live.get("identity"),
            "ui_ready": live.get("ui_ready"),
        }
    )


@studio.command("list")
@handle_error
def studio_list() -> None:
    """List all responsive Magic Studio windows and the currently bound session."""
    sess = get_session()
    discovery = backend.discover_studios()
    emit(
        {
            "ok": True,
            "studios": discovery["sessions"],
            "errors": discovery["errors"],
            "pinned_endpoint": sess.studio_endpoint,
            "pinned_project": sess.project_path,
        }
    )


@studio.command("status")
@handle_error
def studio_status() -> None:
    """Report live feedback from the bound Studio window."""
    sess = get_session()
    emit(sess.execute("ui.feedback", {}, bridge=_BRIDGE))


@studio.command("unpin")
@handle_error
def studio_unpin() -> None:
    """Clear the bound Studio window and project pin."""
    sess = get_session()
    sess.clear()
    emit({"ok": True, "unpinned": True})


@studio.command("quit")
@handle_error
def studio_quit() -> None:
    """Request guarded close of the bound Magic Studio window."""
    sess = get_session()
    res = sess.execute("app.quit", {"_origin": "CLI"}, bridge=_BRIDGE)
    sess.studio_endpoint = None
    sess.studio_instance = None
    sess.save_session()
    emit(res)


# ── PROJECT GROUP ────────────────────────────────────────────────────────────


@cli.group()
def project() -> None:
    """Create, open, list, save, reveal, and manage `.magic` projects."""


@project.command("new")
@click.option("-o", "--output", "output_path", type=str, default=None, help="Output .magic file path")
@click.option("--name", type=str, default=None, help="Project name")
@click.option("--profile", type=str, default="hyperframes", help="Project profile (hyperframes, remotion, character)")
@click.option("--audio", type=str, default=None, help="Optional song audio file path")
@click.option("--overwrite", is_flag=True, help="Overwrite existing project file")
@click.argument("extra", nargs=-1)
@handle_error
def project_new(
    output_path: str | None,
    name: str | None,
    profile: str,
    audio: str | None,
    overwrite: bool,
    extra: tuple[str, ...],
) -> None:
    """Create a new `.magic` project and bind it as the active project."""
    sess = get_session()
    kv = parse_kv_args(extra)
    pos_path = next((x for x in extra if "=" not in x), None)
    target_path = output_path or kv.get("path") or pos_path
    proj_name = name or kv.get("name")
    audio_path = audio or kv.get("audio")
    ow = overwrite or bool(kv.get("overwrite", False))

    if not target_path and proj_name:
        res = sess.execute(
            "project.intake",
            {"name": proj_name, "audio": audio_path, **{k: v for k, v in kv.items() if k not in {"name", "audio"}}},
            bridge=_BRIDGE,
            dry_run=_DRY_RUN,
        )
        if res.get("project") and not _DRY_RUN:
            opened = proj_mod.open(res["project"])
            sess.set_project(opened["document"], res["project"])
        emit(res)
        return

    if not target_path:
        raise ValueError("Provide a project path (-o <path> or positional <path>) or --name <song>")

    if _DRY_RUN:
        p = Path(target_path).expanduser()
        if p.suffix.lower() != ".magic":
            p = p.with_suffix(".magic")
        emit({"ok": True, "dry_run": True, "project": str(p.absolute()), "profile": profile})
        return

    created = proj_mod.create(
        target_path,
        name=proj_name,
        profile=profile,
        audio=audio_path,
        overwrite=ow,
    )
    sess.set_project(created["document"], created["project"])
    emit(created)


@project.command("open")
@click.argument("target", required=False)
@click.argument("extra", nargs=-1)
@handle_error
def project_open(target: str | None, extra: tuple[str, ...]) -> None:
    """Open an existing `.magic` project (attaches to its Studio window if open, else headless)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    raw = target or kv.get("path") or kv.get("project")
    if not raw:
        raise ValueError("Usage: magic project open <project.magic | folder | name>")
    resolved = proj_mod.resolve_project_path(raw)
    live = backend.studio_for_project(resolved)
    if live or _BRIDGE:
        res = sess.execute("project.open", {"path": str(resolved), **kv}, bridge=_BRIDGE)
        if live:
            sess.bind_studio(live)
        emit(res)
        return

    opened = proj_mod.open(resolved)
    res = backend.execute_headless("project.open", {"path": str(resolved), **kv}, dry_run=_DRY_RUN)
    if not _DRY_RUN:
        sess.set_project(opened["document"], resolved)
    emit(res)


@project.command("list")
@click.argument("extra", nargs=-1)
@handle_error
def project_list(extra: tuple[str, ...]) -> None:
    """List recent `.magic` projects and their readiness."""
    sess = get_session()
    kv = parse_kv_args(extra)
    emit(sess.execute("project.catalog", kv, bridge=_BRIDGE))


@project.command("save")
@click.option("-o", "--output", "output_path", type=str, default=None, help="Optional save-as path")
@click.argument("extra", nargs=-1)
@handle_error
def project_save(output_path: str | None, extra: tuple[str, ...]) -> None:
    """Save the active project (flushes pending UI storyboard drafts when connected to Studio)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    if output_path:
        kv["path"] = output_path
    res = sess.execute("project.save", kv, bridge=_BRIDGE, dry_run=_DRY_RUN)
    if not _DRY_RUN and sess.project_path:
        sess._modified = False
        sess.save_session()
    emit(res)


@project.command("close")
@handle_error
def project_close() -> None:
    """Close the active project and clear session context."""
    sess = get_session()
    if sess.studio_endpoint or _BRIDGE:
        res = sess.execute("project.close", {}, bridge=_BRIDGE)
    else:
        res = {"ok": True}
    sess.clear()
    emit(res)


@project.command("reveal")
@handle_error
def project_reveal() -> None:
    """Reveal the active project folder in the system file manager."""
    sess = get_session()
    status = sess.execute("workshop.status", {}, bridge=_BRIDGE)
    folder = status.get("project_dir") or (
        str(Path(sess.project_path).parent) if sess.project_path else None
    )
    if not folder:
        raise RuntimeError("No active project to reveal")
    try:
        subprocess.Popen(
            ["xdg-open", folder],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except OSError:
        pass
    emit({"ok": True, "folder": folder})


@project.command("info")
@click.argument("target", required=False)
@handle_error
def project_info(target: str | None) -> None:
    """Inspect `.magic` project details, assets, storyboard, and HyperFrames website status."""
    sess = get_session()
    path = target or sess.project_path
    if not path:
        raise ValueError("No project specified or bound in session")
    emit(proj_mod.info(path))


@project.command("profiles")
@handle_error
def project_profiles() -> None:
    """List available project profiles."""
    emit({"ok": True, "profiles": proj_mod.list_profiles()})


@project.command("add-asset")
@click.argument("extra", nargs=-1)
@handle_error
def project_add_asset(extra: tuple[str, ...]) -> None:
    """Connect an image, character, or rig asset (`path=<file> character=<name>|background=<name>`)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    sess.snapshot()
    emit(sess.execute("project.add_asset", kv, bridge=_BRIDGE, dry_run=_DRY_RUN))


@project.command("list-assets")
@click.argument("extra", nargs=-1)
@handle_error
def project_list_assets(extra: tuple[str, ...]) -> None:
    """List connected project assets and the active asset."""
    sess = get_session()
    kv = parse_kv_args(extra)
    emit(sess.execute("project.list_assets", kv, bridge=_BRIDGE))


# ── STORYBOARD GROUP ─────────────────────────────────────────────────────────


@cli.group()
def storyboard() -> None:
    """Read, create, update, delete, open, and render native DeckCraft storyboards."""


@storyboard.command("create")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_create(extra: tuple[str, ...]) -> None:
    """Create a storyboard scene (`scene=<N> action=<text> ...`)."""
    sess = get_session()
    EmitRes = sess.execute("storyboard.create", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN)
    emit(EmitRes)


@storyboard.command("read")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_read(extra: tuple[str, ...]) -> None:
    """Read storyboard scenes, pages, and history."""
    sess = get_session()
    emit(sess.execute("storyboard.read", parse_kv_args(extra), bridge=_BRIDGE))


@storyboard.command("update")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_update(extra: tuple[str, ...]) -> None:
    """Update a storyboard scene field, split/combine pages, or restore history."""
    sess = get_session()
    emit(sess.execute("storyboard.update", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@storyboard.command("delete")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_delete(extra: tuple[str, ...]) -> None:
    """Delete a storyboard scene (`scene=<N>`)."""
    sess = get_session()
    emit(sess.execute("storyboard.delete", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@storyboard.command("edit")
@click.argument("kind", type=str)
@click.argument("scene_num", type=int)
@click.argument("text", type=str)
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_edit(kind: str, scene_num: int, text: str, extra: tuple[str, ...]) -> None:
    """Edit a storyboard scene (`magic storyboard edit scene <N> <text> [field=action]`)."""
    if kind != "scene":
        raise ValueError("Usage: magic storyboard edit scene <N> <text> [field=action]")
    sess = get_session()
    kv = parse_kv_args(extra)
    kv.setdefault("field", "action")
    kv["scene"] = scene_num
    kv["value"] = text
    emit(sess.execute("storyboard.update", kv, bridge=_BRIDGE, dry_run=_DRY_RUN))


@storyboard.command("open")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_open(extra: tuple[str, ...]) -> None:
    """Inspect and open the project's `.deckcraft` storyboard deck."""
    sess = get_session()
    res = sess.execute("workshop.storyboard", parse_kv_args(extra), bridge=_BRIDGE)
    deck = res.get("deck")
    if deck and Path(deck).is_file():
        try:
            subprocess.Popen(
                ["xdg-open", deck],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except OSError:
            pass
    emit(res)


@storyboard.command("build")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_build(extra: tuple[str, ...]) -> None:
    """Build the native `.deckcraft` storyboard deck from the project's scenes."""
    sess = get_session()
    emit(sess.execute("workshop.build_storyboard", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@storyboard.command("render")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_render(extra: tuple[str, ...]) -> None:
    """Render storyboard pages to preview PNGs (`storyboard/.preview/page-*.png`)."""
    sess = get_session()
    emit(sess.execute("storyboard.render", parse_kv_args(extra), bridge=_BRIDGE))


@storyboard.command("doctor")
@click.argument("extra", nargs=-1)
@handle_error
def storyboard_doctor(extra: tuple[str, ...]) -> None:
    """Run read-only storyboard section-boundary diagnostics."""
    sess = get_session()
    emit(sess.execute("storyboard.doctor", parse_kv_args(extra), bridge=_BRIDGE))


# ── CHARACTER GROUP ──────────────────────────────────────────────────────────


@cli.group()
def character() -> None:
    """Import, configure, slice, and rig 2D/3D characters."""


@character.command("list")
@click.argument("extra", nargs=-1)
@handle_error
def character_list(extra: tuple[str, ...]) -> None:
    """List character assets, sheets, angles, and rigs."""
    sess = get_session()
    emit(sess.execute("character.assets", parse_kv_args(extra), bridge=_BRIDGE))


@character.command("import")
@click.argument("extra", nargs=-1)
@handle_error
def character_import(extra: tuple[str, ...]) -> None:
    """Import character artwork (`character=<id> path=<file>`)."""
    sess = get_session()
    emit(sess.execute("character.import", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@character.command("configure")
@click.argument("extra", nargs=-1)
@handle_error
def character_configure(extra: tuple[str, ...]) -> None:
    """Configure active angle or pose for a character (`character=<id> angle=<angle>`)."""
    sess = get_session()
    emit(sess.execute("character.configure", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@character.command("cut-sheet")
@click.argument("extra", nargs=-1)
@handle_error
def character_cut_sheet(extra: tuple[str, ...]) -> None:
    """Slice a turnaround or pose sheet into transparent PNG parts."""
    sess = get_session()
    emit(sess.execute("asset.cut_sheet", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@character.command("build-rig")
@click.argument("extra", nargs=-1)
@handle_error
def character_build_rig(extra: tuple[str, ...]) -> None:
    """Build a 2D articulated cutout rig from sliced parts."""
    sess = get_session()
    emit(sess.execute("asset.build_cutout_rig", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


# ── MOTION GROUP ─────────────────────────────────────────────────────────────


@cli.group()
def motion() -> None:
    """Inspect and author character motion tracks, Theatre keyframes, and paths."""


@motion.command("tracks")
@click.argument("extra", nargs=-1)
@handle_error
def motion_tracks(extra: tuple[str, ...]) -> None:
    """List motion tracks and FPS for a character."""
    sess = get_session()
    emit(sess.execute("motion.tracks", parse_kv_args(extra), bridge=_BRIDGE))


@motion.command("clips")
@click.option("--convert", is_flag=True, help="Convert clip sheets before listing")
@click.argument("extra", nargs=-1)
@handle_error
def motion_clips(convert: bool, extra: tuple[str, ...]) -> None:
    """List each clip's Theatre sheet, duration, and conversion status."""
    sess = get_session()
    kv = parse_kv_args(extra)
    if convert:
        sess.execute("motion.clips.convert", dict(kv), bridge=_BRIDGE, dry_run=_DRY_RUN)
    emit(sess.execute("motion.clips", kv, bridge=_BRIDGE))


@motion.command("sample")
@click.argument("extra", nargs=-1)
@handle_error
def motion_sample(extra: tuple[str, ...]) -> None:
    """Sample evaluated part transforms at a given time (`character=<id> track=<id> time=<sec>`)."""
    sess = get_session()
    emit(sess.execute("motion.sample", parse_kv_args(extra), bridge=_BRIDGE))


@motion.command("keyframe")
@click.argument("extra", nargs=-1)
@handle_error
def motion_keyframe(extra: tuple[str, ...]) -> None:
    """Set or update a Theatre motion keyframe."""
    sess = get_session()
    emit(sess.execute("motion.edit.keyframe", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@motion.command("undo")
@click.argument("extra", nargs=-1)
@handle_error
def motion_undo(extra: tuple[str, ...]) -> None:
    """Undo the last Theatre motion keyframe edit."""
    sess = get_session()
    emit(sess.execute("motion.edit.undo", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@motion.command("path")
@click.argument("extra", nargs=-1)
@handle_error
def motion_path(extra: tuple[str, ...]) -> None:
    """Apply an authored motion path to a character or part."""
    sess = get_session()
    emit(sess.execute("motion.path", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@motion.command("undo-path")
@click.argument("extra", nargs=-1)
@handle_error
def motion_undo_path(extra: tuple[str, ...]) -> None:
    """Undo the last authored motion path on a character or part."""
    sess = get_session()
    emit(sess.execute("motion.undo_path", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


# ── VIDEO GROUP ──────────────────────────────────────────────────────────────


@cli.group()
def video() -> None:
    """Manage the project's HyperFrames website or Remotion video studio."""


@video.command("init")
@click.argument("extra", nargs=-1)
@handle_error
def video_init(extra: tuple[str, ...]) -> None:
    """Scaffold the project's HyperFrames website (`hyperframes/index.html`) or Remotion studio."""
    sess = get_session()
    emit(sess.execute("video.init", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@video.command("status")
@click.argument("extra", nargs=-1)
@handle_error
def video_status(extra: tuple[str, ...]) -> None:
    """Check readiness of the project's video target (`hyperframes/index.html`)."""
    sess = get_session()
    emit(sess.execute("workshop.status", parse_kv_args(extra), bridge=_BRIDGE))


@video.command("snapshot")
@click.argument("extra", nargs=-1)
@handle_error
def video_snapshot(extra: tuple[str, ...]) -> None:
    """Render a still PNG frame from the project's video composition (`sec=<time> out=<png>`)."""
    sess = get_session()
    emit(sess.execute("video.snapshot", parse_kv_args(extra), bridge=_BRIDGE))


@video.command("render")
@click.argument("extra", nargs=-1)
@handle_error
def video_render(extra: tuple[str, ...]) -> None:
    """Render the full video composition to MP4."""
    sess = get_session()
    emit(sess.execute("video.render", parse_kv_args(extra), bridge=_BRIDGE))


@video.command("place-track")
@click.argument("extra", nargs=-1)
@handle_error
def video_place_track(extra: tuple[str, ...]) -> None:
    """Place a rendered transparent animation track into the video composition."""
    sess = get_session()
    emit(sess.execute("video.place_track", parse_kv_args(extra), bridge=_BRIDGE, dry_run=_DRY_RUN))


@video.command("server-start")
@click.argument("extra", nargs=-1)
@handle_error
def video_server_start(extra: tuple[str, ...]) -> None:
    """Start the project's HyperFrames / Remotion Studio preview server."""
    sess = get_session()
    emit(sess.execute("video.server_start", parse_kv_args(extra), bridge=_BRIDGE))


@video.command("server-stop")
@click.argument("extra", nargs=-1)
@handle_error
def video_server_stop(extra: tuple[str, ...]) -> None:
    """Stop the project's HyperFrames / Remotion Studio preview server."""
    sess = get_session()
    emit(sess.execute("video.server_stop", parse_kv_args(extra), bridge=_BRIDGE))


# ── UI GROUP ─────────────────────────────────────────────────────────────────


@cli.group()
def ui() -> None:
    """Inspect and drive the live Magic Studio GUI window."""


@ui.command("state")
@click.argument("extra", nargs=-1)
@handle_error
def ui_state(extra: tuple[str, ...]) -> None:
    """Read live Studio window state (project, view, animation subject, track, playhead)."""
    sess = get_session()
    emit(sess.execute("ui.state", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("feedback")
@click.argument("extra", nargs=-1)
@handle_error
def ui_feedback(extra: tuple[str, ...]) -> None:
    """Read canonical Studio identity, readiness, unsaved state, and DOM acknowledgement."""
    sess = get_session()
    emit(sess.execute("ui.feedback", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("elements")
@click.argument("extra", nargs=-1)
@handle_error
def ui_elements(extra: tuple[str, ...]) -> None:
    """List visible clickable elements with IDs, labels, and bounding boxes."""
    sess = get_session()
    emit(sess.execute("ui.elements", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("components")
@click.argument("extra", nargs=-1)
@handle_error
def ui_components(extra: tuple[str, ...]) -> None:
    """Inspect bounded DOM geometry, computed styles, and component state."""
    sess = get_session()
    emit(sess.execute("ui.components", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("controls")
@click.argument("extra", nargs=-1)
@handle_error
def ui_controls(extra: tuple[str, ...]) -> None:
    """List interactive controls and their bound engine commands."""
    sess = get_session()
    emit(sess.execute("ui.controls", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("click")
@click.argument("target_id", required=False)
@click.argument("extra", nargs=-1)
@handle_error
def ui_click(target_id: str | None, extra: tuple[str, ...]) -> None:
    """Click a visible UI element by ID or aria label."""
    sess = get_session()
    kv = parse_kv_args(extra)
    if target_id and "=" in target_id:
        kv.update(parse_kv_args((target_id,)))
    elif target_id:
        kv.setdefault("id", target_id)
    emit(sess.execute("ui.click", kv, bridge=_BRIDGE))


@ui.command("set")
@click.argument("extra", nargs=-1)
@handle_error
def ui_set(extra: tuple[str, ...]) -> None:
    """Set an input or select control value (`id=<element_id> value=<val>`)."""
    sess = get_session()
    emit(sess.execute("ui.set", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("key")
@click.argument("extra", nargs=-1)
@handle_error
def ui_key(extra: tuple[str, ...]) -> None:
    """Send a key press to the live Studio window (`key=<key> [id=<element_id>]`)."""
    sess = get_session()
    emit(sess.execute("ui.key", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("drag")
@click.argument("extra", nargs=-1)
@handle_error
def ui_drag(extra: tuple[str, ...]) -> None:
    """Drag a control or canvas point in the live Studio window."""
    sess = get_session()
    emit(sess.execute("ui.drag", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("look")
@click.argument("extra", nargs=-1)
@handle_error
def ui_look(extra: tuple[str, ...]) -> None:
    """Optionally click an element, capture a PNG of the window/region, and return state + elements."""
    sess = get_session()
    emit(sess.execute("ui.look", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("screenshot")
@click.argument("extra", nargs=-1)
@handle_error
def ui_screenshot(extra: tuple[str, ...]) -> None:
    """Save a PNG screenshot of the live Studio window (`[output=<png>]`)."""
    sess = get_session()
    emit(sess.execute("ui.screenshot", parse_kv_args(extra), bridge=_BRIDGE))


@ui.command("focus")
@handle_error
def ui_focus() -> None:
    """Bring the bound Magic Studio window to the front."""
    sess = get_session()
    emit(sess.execute("ui.focus", {}, bridge=_BRIDGE))


@ui.command("eval")
@click.argument("js_code", required=False)
@click.argument("extra", nargs=-1)
@handle_error
def ui_eval(js_code: str | None, extra: tuple[str, ...]) -> None:
    """Evaluate JavaScript in the project webview (`js=<code_str>`)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    if js_code and not js_code.startswith("js="):
        kv["js"] = js_code
    elif js_code:
        kv.update(parse_kv_args((js_code,)))
    emit(sess.execute("ui.eval", kv, bridge=_BRIDGE))


# ── EXPORT GROUP ─────────────────────────────────────────────────────────────


@cli.group()
def export() -> None:
    """Render and export storyboards, transparent motion clips, and final videos."""


@export.command("presets")
@handle_error
def export_presets() -> None:
    """List available export presets."""
    emit({"ok": True, "presets": export_mod.list_presets()})


@export.command("render")
@click.option("-p", "--preset", type=str, default="storyboard_png", help="Export preset name")
@click.option("-o", "--output", "output_path", type=str, default=None, help="Output file or folder path")
@click.argument("extra", nargs=-1)
@handle_error
def export_render(preset: str, output_path: str | None, extra: tuple[str, ...]) -> None:
    """Render output using the selected export preset."""
    sess = get_session()
    kv = parse_kv_args(extra)
    emit(export_mod.render(sess, preset=preset, output_path=output_path, extra_params=kv, bridge=_BRIDGE))


# ── SESSION GROUP ────────────────────────────────────────────────────────────


@cli.group()
def session() -> None:
    """Inspect session binding and invoke domain-specific undo/redo."""


@session.command("status")
@handle_error
def session_status() -> None:
    """Show the currently bound project path and live Studio endpoint."""
    sess = get_session()
    live = (
        backend.studio_for_project(sess.project_path)
        if sess.project_path and Path(sess.project_path).exists()
        else None
    )
    emit(
        {
            "ok": True,
            "session_file": str(sess.session_file),
            "project_path": sess.project_path,
            "studio_endpoint": sess.studio_endpoint or (live.get("endpoint") if live else None),
            "studio_instance": sess.studio_instance or (live.get("instance") if live else None),
            "live_window_open": bool(live or sess.studio_endpoint),
            "modified": sess._modified,
        }
    )


@session.command("undo")
@click.option("--domain", type=str, default=None, help="Domain to undo (motion, path, part, skills)")
@click.argument("extra", nargs=-1)
@handle_error
def session_undo(domain: str | None, extra: tuple[str, ...]) -> None:
    """Undo within a specific engine domain or local project state."""
    sess = get_session()
    kv = parse_kv_args(extra)
    emit(sess.undo(domain=domain, **kv))


@session.command("redo")
@handle_error
def session_redo() -> None:
    """Redo the last undone local project state mutation."""
    sess = get_session()
    emit(sess.redo())


@session.command("clear")
@handle_error
def session_clear() -> None:
    """Clear the persisted session binding."""
    sess = get_session()
    sess.clear()
    emit({"ok": True, "cleared": True})


# ── SETTINGS / APP / EXEC / COMMANDS / MCP ───────────────────────────────────


@cli.command("settings")
@click.argument("extra", nargs=-1)
@handle_error
def settings_cmd(extra: tuple[str, ...]) -> None:
    """Inspect or update the production project root (`magic settings [root=<path>]`)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    pos = next((x for x in extra if "=" not in x), None)
    if pos and "root" not in kv:
        kv["root"] = pos
    emit(sess.execute("project.root", kv, bridge=_BRIDGE, dry_run=_DRY_RUN))


@cli.group()
def app() -> None:
    """Application-level lifecycle commands (`magic app open`, `magic app quit`, `magic app doctor`)."""


@app.command("open")
@click.argument("project_target", required=False)
@handle_error
def app_open_cmd(project_target: str | None) -> None:
    """Open Magic Studio window and attach session."""
    sess = get_session()
    resolved = str(proj_mod.resolve_project_path(project_target)) if project_target else None
    live = backend.open_studio_window(resolved)
    sess.bind_studio(live)
    emit(
        {
            "ok": True,
            "active_project": live.get("active_project"),
            "endpoint": live.get("endpoint"),
            "instance": live.get("instance"),
            "identity": live.get("identity"),
            "ui_ready": live.get("ui_ready"),
        }
    )


@app.command("quit")
@handle_error
def app_quit_cmd() -> None:
    """Quit the bound Magic Studio window."""
    sess = get_session()
    res = sess.execute("app.quit", {"_origin": "CLI"}, bridge=_BRIDGE)
    sess.studio_endpoint = None
    sess.studio_instance = None
    sess.save_session()
    emit(res)


@app.command("doctor")
@click.argument("extra", nargs=-1)
@handle_error
def app_doctor_cmd(extra: tuple[str, ...]) -> None:
    """Check Craft CLI tools and running Studio instances."""
    sess = get_session()
    res = sess.execute("app.doctor", parse_kv_args(extra), bridge=_BRIDGE)
    discovery = backend.discover_studios()
    res["studios"] = discovery["sessions"]
    res["studio_errors"] = discovery["errors"]
    emit(res)


@cli.command("exec")
@click.argument("cmd_id", type=str)
@click.argument("extra", nargs=-1)
@handle_error
def exec_cmd(cmd_id: str, extra: tuple[str, ...]) -> None:
    """Execute any registered engine or UI command (`magic exec <id> [key=value ...]`)."""
    sess = get_session()
    kv = parse_kv_args(extra)
    if cmd_id in {"project.open", "project.identity"} and "path" not in kv and "project" in kv:
        kv["path"] = kv.pop("project")
        kv.pop("_expected_project", None)
    res = sess.execute(cmd_id, kv, bridge=_BRIDGE, dry_run=_DRY_RUN)
    if cmd_id in {"project.open", "project.new", "project.intake"} and res.get("project") and not _DRY_RUN:
        sess.project_path = str(Path(res["project"]).resolve())
        sess.save_session()
    emit(res)


@cli.command("commands")
@click.argument("filter_str", required=False)
@handle_error
def commands_cmd(filter_str: str | None) -> None:
    """List registered engine commands."""
    specs = backend.list_engine_commands()
    if filter_str:
        f = filter_str.lower()
        specs = [
            s
            for s in specs
            if f in s["id"].lower() or f in s["label"].lower()
        ]
    if _USE_JSON:
        emit({"ok": True, "commands": specs})
    else:
        for s in specs:
            shortcut = s.get("shortcut") or "-"
            click.echo(f"{s['id']:<36} {s['label']:<24} {shortcut:<14} {s.get('params', '{}')}")


@cli.command("mcp")
def mcp_cmd() -> None:
    """Run the Magic Studio MCP server over stdio."""
    sess = get_session()
    mcp_server.run_mcp_stdio(sess, bridge=_BRIDGE)


@cli.command("run")
def run_cmd() -> None:
    """Execute JSONL command batches from stdin in one persistent engine session."""
    sess = get_session()
    target = sess.resolve_live_target(_BRIDGE)
    if target:
        for line in sys.stdin:
            text = line.strip()
            if not text or text.startswith("#") or text.startswith("//"):
                continue
            item = json.loads(text)
            cmd_id = item.get("command") or item.get("id")
            params = item.get("params") or {}
            res = sess.execute(cmd_id, params, bridge=_BRIDGE, dry_run=_DRY_RUN)
            click.echo(json.dumps(res))
    else:
        proc = subprocess.Popen(
            [str(backend.find_studio_shell()), "--headless", "run"],
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
        )
        raise SystemExit(proc.wait())


@cli.command("doctor")
@click.argument("extra", nargs=-1)
@handle_error
def doctor_top_cmd(extra: tuple[str, ...]) -> None:
    """Check Craft CLI tools and running Studio instances (`magic doctor`)."""
    sess = get_session()
    res = sess.execute("app.doctor", parse_kv_args(extra), bridge=_BRIDGE)
    discovery = backend.discover_studios()
    res["studios"] = discovery["sessions"]
    res["studio_errors"] = discovery["errors"]
    emit(res)


def _preprocess_argv(argv: list[str]) -> list[str]:
    """Normalize shorthand `magic studio <project>` into `magic studio open <project>`."""
    out = list(argv)
    for i, token in enumerate(out):
        if token == "studio" and i + 1 < len(out):
            nxt = out[i + 1]
            if nxt not in {"new", "open", "list", "status", "unpin", "quit", "--help", "-h"} and not nxt.startswith("-"):
                out.insert(i + 1, "open")
            break
    return out


def main() -> None:
    """Entry point for `magic` and `magic`."""
    sys.argv[1:] = _preprocess_argv(sys.argv[1:])
    cli()


if __name__ == "__main__":
    main()
