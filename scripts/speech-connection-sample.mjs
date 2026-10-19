// Samples the network connections of the process tree of an INSTALLED copy of the application, for the
// offline proof of the speech module (docs/SPEECH.md, section 12; the method of SpeechLab's M7).
//
// The tree is every process whose executable lies in the install folder, and all their descendants. A
// process of the install folder is the application's own code (the application, both speech workers,
// Tesseract); any other process of the tree (the system web view runtime) is reported apart, because it
// is not the application's code. Every sample reads `netstat -ano` once; a connection counts when its
// remote address is not loopback. It prints names, states and addresses of the machine's own connections
// and nothing else: no text, no content.
//
//   node scripts/speech-connection-sample.mjs "C:/Users/elise/AppData/Local/Assistant Cabinet AI" [seconds] [interval ms]
//
// Exit code 1 when a program of the install folder held a connection outside the machine, else 0.
import { execFileSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";

const LOOPBACK = /^(127\.|\[?::1\]?$|\[?::ffff:127\.)/;

/** The rows of `netstat -ano` as { protocol, local, remote, state, pid }. */
export function parseNetstat(output) {
  const rows = [];
  for (const line of output.split(/\r?\n/)) {
    const parts = line.trim().split(/\s+/);
    if (parts[0] === "TCP" && parts.length === 5) {
      rows.push({ protocol: "TCP", local: parts[1], remote: parts[2], state: parts[3], pid: Number(parts[4]) });
    } else if (parts[0] === "UDP" && parts.length === 4) {
      rows.push({ protocol: "UDP", local: parts[1], remote: parts[2], state: "", pid: Number(parts[3]) });
    }
  }
  return rows;
}

/** True for an address that stays on the machine (or has no peer at all: a listening socket). */
export function isLocal(address) {
  const host = address.replace(/:\d+$|:\*$/, "");
  return host === "*" || host === "0.0.0.0" || host === "[::]" || host === "" || LOOPBACK.test(host);
}

/** The pids of the roots (processes under `folder`) and of all their descendants, with their names. */
export function treeOf(processes, folder) {
  const prefix = path.resolve(folder).toLowerCase() + path.sep;
  const roots = processes.filter((p) => p.path && path.resolve(p.path).toLowerCase().startsWith(prefix));
  const members = new Map(roots.map((p) => [p.pid, { name: p.name, own: true }]));
  let grew = true;
  while (grew) {
    grew = false;
    for (const p of processes) {
      if (!members.has(p.pid) && members.has(p.parent)) {
        members.set(p.pid, { name: p.name, own: false });
        grew = true;
      }
    }
  }
  return members;
}

function readProcesses() {
  const script =
    "Get-CimInstance Win32_Process | ForEach-Object { [pscustomobject]@{ pid = [int]$_.ProcessId; parent = [int]$_.ParentProcessId; name = $_.Name; path = $_.ExecutablePath } } | ConvertTo-Json -Compress";
  const raw = execFileSync("powershell.exe", ["-NoProfile", "-Command", script], { encoding: "utf8", maxBuffer: 1 << 26 });
  const parsed = JSON.parse(raw);
  return Array.isArray(parsed) ? parsed : [parsed];
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main() {
  const [folder, secondsArg, intervalArg] = process.argv.slice(2);
  if (!folder) {
    console.error('usage: node scripts/speech-connection-sample.mjs "<install folder>" [seconds] [interval ms]');
    process.exit(2);
  }
  const seconds = Number(secondsArg ?? 40);
  const interval = Number(intervalArg ?? 500);
  const seen = new Map(); // "own|name|protocol|state|remote" -> count
  const names = new Set();
  let samples = 0;
  const end = Date.now() + seconds * 1000;
  while (Date.now() < end) {
    const tree = treeOf(readProcesses(), folder);
    for (const member of tree.values()) names.add(`${member.own ? "own" : "system"}:${member.name}`);
    const rows = parseNetstat(execFileSync("netstat", ["-ano"], { encoding: "utf8", maxBuffer: 1 << 26 }));
    for (const row of rows) {
      const member = tree.get(row.pid);
      if (!member || isLocal(row.remote)) continue;
      const key = `${member.own ? "own" : "system"}|${member.name}|${row.protocol}|${row.state}|${row.remote}`;
      seen.set(key, (seen.get(key) ?? 0) + 1);
    }
    samples += 1;
    await sleep(interval);
  }

  console.log(`samples: ${samples} over ${seconds} s`);
  console.log(`processes seen: ${[...names].sort().join(", ") || "none (is the application running?)"}`);
  const own = [...seen].filter(([key]) => key.startsWith("own|"));
  const system = [...seen].filter(([key]) => key.startsWith("system|"));
  console.log(`connections outside the machine from the application's own programs: ${own.length === 0 ? "none" : ""}`);
  for (const [key, count] of own) console.log(`  ${key.split("|").slice(1).join(" ")}  x${count}`);
  console.log(`connections outside the machine from the other processes of the tree (the system web view runtime): ${system.length === 0 ? "none" : ""}`);
  for (const [key, count] of system) console.log(`  ${key.split("|").slice(1).join(" ")}  x${count}`);
  process.exit(own.length === 0 ? 0 : 1);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  await main();
}
