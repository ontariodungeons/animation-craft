#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { closeSync, existsSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

const [action, ...args] = process.argv.slice(2);

function commonGitDir() {
  const value = execFileSync("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"], { encoding: "utf8" }).trim();
  return path.resolve(value);
}

function statusPath(pid, start) {
  return path.join(commonGitDir(), "commit-checks", `${pid}-${start}.json`);
}

function atomicWrite(file, data) {
  const temp = `${file}.${process.pid}.${Date.now()}.tmp`;
  let fd;
  try {
    fd = openSync(temp, "wx", 0o600);
    writeFileSync(fd, `${JSON.stringify(data, null, 2)}\n`, "utf8");
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    renameSync(temp, file);
  } catch (error) {
    if (fd !== undefined) closeSync(fd);
    rmSync(temp, { force: true });
    throw error;
  }
}

if (action === "start") {
  const [rawPid, branch, headBefore] = args;
  const pid = Number(rawPid);
  if (!Number.isSafeInteger(pid) || pid <= 0 || !branch || !/^[0-9a-f]{40,64}$/i.test(headBefore ?? "")) {
    throw new Error("usage: commit_status.mjs start <pid> <branch> <head-before>");
  }
  const directory = path.join(commonGitDir(), "commit-checks");
  mkdirSync(directory, { recursive: true });
  let start = Date.now();
  let file = path.join(directory, `${pid}-${start}.json`);
  while (true) {
    if (existsSync(file)) {
      start += 1;
      file = path.join(directory, `${pid}-${start}.json`);
      continue;
    }
    const claim = `${file}.claim`;
    let fd;
    let ownsClaim = false;
    try {
      fd = openSync(claim, "wx", 0o600);
      ownsClaim = true;
      closeSync(fd);
      fd = undefined;
      atomicWrite(file, {
        pid,
        state: "running",
        step: "hook setup",
        exit_code: null,
        started_at: new Date(start).toISOString(),
        updated_at: new Date().toISOString(),
        branch,
        head_before: headBefore,
      });
      rmSync(claim, { force: true });
      break;
    } catch (error) {
      if (fd !== undefined) closeSync(fd);
      if (ownsClaim) rmSync(claim, { force: true });
      if (error.code !== "EEXIST") throw error;
      start += 1;
      file = path.join(directory, `${pid}-${start}.json`);
    }
  }
  process.stdout.write(String(start));
} else if (action === "update") {
  const [rawPid, rawStart, state, step, rawExitCode] = args;
  const pid = Number(rawPid);
  const start = Number(rawStart);
  const exitCode = rawExitCode === "-" ? null : Number(rawExitCode);
  if (!Number.isSafeInteger(pid) || pid <= 0 || !Number.isSafeInteger(start) || start <= 0
      || !["running", "failed", "passed"].includes(state) || !step
      || (exitCode !== null && (!Number.isInteger(exitCode) || exitCode < 0 || exitCode > 255))
      || (state === "running" && exitCode !== null) || (state !== "running" && exitCode === null)) {
    throw new Error("usage: commit_status.mjs update <pid> <start-ms> <running|failed|passed> <step> <-|exit-code>");
  }
  const file = statusPath(pid, start);
  const status = JSON.parse(readFileSync(file, "utf8"));
  if (status.pid !== pid || Date.parse(status.started_at) !== start) throw new Error(`status identity mismatch: ${file}`);
  status.state = state;
  status.step = step;
  status.exit_code = exitCode;
  status.updated_at = new Date().toISOString();
  atomicWrite(file, status);
} else {
  throw new Error("usage: commit_status.mjs start|update ...");
}
