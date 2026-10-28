"""Magic's terminal adapter. Command vocabulary comes from the application registry."""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import click

from cli_anything.magic import __version__
from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend


def parse_params(tokens: tuple[str, ...], raw: str | None = None) -> dict[str, Any]:
    params = json.loads(raw) if raw else {}
    if not isinstance(params, dict):
        raise click.BadParameter("--params must be a JSON object")
    for token in tokens:
        if "=" not in token:
            raise click.BadParameter(f"Expected key=value, got {token!r}; see --help")
        key, value = token.split("=", 1)
        if not key:
            raise click.BadParameter("Parameter name cannot be empty")
        try:
            params[key] = json.loads(value)
        except ValueError:
            params[key] = value
    return params


def emit(value: Any) -> None:
    click.echo(json.dumps(value, indent=2))


def execute(ctx: click.Context, command: str, params: dict[str, Any]) -> Any:
    params.setdefault("_origin", "CLI")
    return ctx.obj["session"].execute(command, params, dry_run=ctx.obj["dry_run"], bridge=ctx.obj["bridge"])


class RegistryGroup(click.Group):
    """Lazily derive `domain verb` (underscores become hyphens) from command IDs."""
    def __init__(self, *args: Any, prefix: str = "", **kwargs: Any):
        super().__init__(*args, **kwargs)
        self.prefix = prefix
        self._specs: list[dict[str, Any]] | None = None

    def specs(self) -> list[dict[str, Any]]:
        if self._specs is None:
            self._specs = backend.list_engine_commands()
        return self._specs

    def list_commands(self, ctx: click.Context) -> list[str]:
        names = set(super().list_commands(ctx))
        for spec in self.specs():
            if spec["id"].startswith(self.prefix):
                names.add(spec["id"][len(self.prefix):].split(".")[0].replace("_", "-"))
        return sorted(names)

    def get_command(self, ctx: click.Context, name: str) -> click.Command | None:
        explicit = super().get_command(ctx, name)
        if explicit:
            return explicit
        command_id = self.prefix + name.replace("-", "_")
        spec = next((s for s in self.specs() if s["id"] == command_id), None)
        children = any(s["id"].startswith(command_id + ".") for s in self.specs())
        if not spec and not children:
            return None
        def run(**args: Any) -> None:
            current = click.get_current_context()
            if current.invoked_subcommand:
                return
            if spec:
                emit(execute(current, command_id, parse_params(args.get("arguments", ()), args.get("params"))))
            else:
                click.echo(current.get_help())
        help_text = f'{spec["label"]}. Parameters: {spec["params"]}' if spec else f'{name} commands from the application registry.'
        options = [click.Option(["--params"], help="JSON parameter object")]
        # Groups may also be commands (e.g. motion.path); use --params for the parent.
        if not children:
            options.append(click.Argument(["arguments"], nargs=-1))
        if children:
            group = RegistryGroup(name, prefix=command_id + ".", callback=run,
                                  params=options, help=help_text, invoke_without_command=True)
            group._specs = self.specs()
            return group
        return click.Command(name, callback=run, params=options, help=help_text)


@click.group(cls=RegistryGroup, invoke_without_command=True)
@click.option("--json", "use_json", is_flag=True, help="JSON output (also the default)")
@click.option("--project", type=click.Path(), help="Select the intended .magic project")
@click.option("--session-file", type=click.Path(), envvar="MAGIC_SESSION_FILE", help="Separate caller session binding")
@click.option("--dry-run", is_flag=True, help="Preview mutations without applying them")
@click.option("--bridge", hidden=True, help="Internal explicit window endpoint")
@click.version_option(__version__, prog_name="magic-cli")
@click.pass_context
def cli(ctx: click.Context, use_json: bool, project: str | None, session_file: str | None, dry_run: bool, bridge: str | None) -> None:
    """Magic Studio commands. Launch the graphical application with `magic`.

    Commands and parameter documentation come from the shared application registry.
    Use key=value parameters or --params JSON; no REPL or per-operation ports.
    """
    session = Session(session_file)
    ctx.obj = {"session": session, "dry_run": dry_run, "bridge": bridge}
    if project:
        session.select_project(project)
    if ctx.invoked_subcommand is None:
        click.echo(ctx.get_help())


@cli.command("commands")
@click.argument("filter", required=False)
@click.pass_context
def commands(ctx: click.Context, filter: str | None) -> None:
    """Discover registered command IDs and their parameter documentation."""
    emit(execute(ctx, "command.list", {"filter": filter or ""}))


@cli.command("exec")
@click.argument("command")
@click.argument("arguments", nargs=-1)
@click.option("--params", help="JSON parameter object")
@click.pass_context
def exec_command(ctx: click.Context, command: str, arguments: tuple[str, ...], params: str | None) -> None:
    """Execute a registry ID directly."""
    emit(execute(ctx, command, parse_params(arguments, params)))


@cli.command("run")
@click.argument("file", type=click.File("r"), default="-")
@click.pass_context
def run(ctx: click.Context, file: Any) -> None:
    """Run JSONL {id, params} steps in order; stop on error. Earlier steps remain applied."""
    for number, line in enumerate(file, 1):
        if not line.strip() or line.lstrip().startswith(("#", "//")):
            continue
        item = json.loads(line)
        result = execute(ctx, item.get("id") or item["command"], item.get("params", {}))
        click.echo(json.dumps({"line": number, "ok": True, "result": result}))


@cli.command("mcp")
@click.pass_context
def mcp(ctx: click.Context) -> None:
    """Serve Magic commands over MCP stdio using the official Python SDK."""
    from cli_anything.magic.core.mcp_server import run_mcp_stdio
    run_mcp_stdio(ctx.obj["session"], bridge=ctx.obj["bridge"], dry_run=ctx.obj["dry_run"])


@cli.group(cls=RegistryGroup, prefix="studio.")
def studio() -> None:
    """Select and inspect live Magic Studio windows."""


@studio.command("open")
@click.argument("project", required=False)
@click.pass_context
def studio_open(ctx: click.Context, project: str | None) -> None:
    """Open/focus a project window, or a blank window if omitted."""
    if ctx.obj["dry_run"]:
        emit({"dry_run": True, "project": project})
        return
    from cli_anything.magic.core.project import resolve_project_path
    live = backend.open_studio_window(str(resolve_project_path(project)) if project else None)
    ctx.obj["session"].bind_studio(live)
    emit(live)


@studio.command("list")
def studio_list() -> None:
    """List responsive windows without changing the current selection."""
    emit(backend.discover_studios())


@cli.group()
def session() -> None:
    """Inspect or clear this caller's project/window binding."""


@session.command("status")
@click.pass_context
def session_status(ctx: click.Context) -> None:
    sess = ctx.obj["session"]
    emit({"project": sess.project_path, "endpoint": sess.studio_endpoint, "instance": sess.studio_instance})


@session.command("clear")
@click.pass_context
def session_clear(ctx: click.Context) -> None:
    ctx.obj["session"].clear()
    emit({"ok": True})


def main() -> None:
    try:
        cli(prog_name="magic-cli")
    except (RuntimeError, OSError, ValueError) as exc:
        click.echo(f"magic-cli: {exc}", err=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
