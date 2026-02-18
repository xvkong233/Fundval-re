import { runHealth } from "./cases/health.js";
import { runBootstrap } from "./cases/bootstrap.js";
import { runAuth } from "./cases/auth.js";
import { runUsers } from "./cases/users.js";
import { runSources } from "./cases/sources.js";
import { runFunds } from "./cases/funds.js";
import { runAccounts } from "./cases/accounts.js";
import { runPositions } from "./cases/positions.js";
import { runPositionsHistory } from "./cases/positions_history.js";
import { runWatchlists } from "./cases/watchlists.js";
import { runNavHistory } from "./cases/nav_history.js";
import { runSourcesAccuracy } from "./cases/sources_accuracy.js";
import { seedDatabases } from "./seed.js";

type Case = {
  name: string;
  run: (goldenBase: string, candidateBase: string) => Promise<void>;
};

const goldenBase = process.env.GOLDEN_BASE ?? "http://localhost:8000";
const candidateBase = process.env.CANDIDATE_BASE ?? "http://localhost:8001";

const cases: Case[] = [
  { name: "health", run: runHealth },
  { name: "bootstrap", run: runBootstrap },
  { name: "auth", run: runAuth },
  { name: "users", run: runUsers },
  { name: "sources", run: runSources },
  { name: "sources_accuracy", run: runSourcesAccuracy },
  { name: "accounts", run: runAccounts },
  { name: "funds", run: runFunds },
  { name: "positions", run: runPositions },
  { name: "positions_history", run: runPositionsHistory },
  { name: "watchlists", run: runWatchlists },
  { name: "nav_history", run: runNavHistory },
];

function parseRequestedCases(): Set<string> {
  const requested = new Set<string>();

  const env = process.env.CASE_FILTER?.trim();
  if (env) {
    for (const part of env.split(/[,\s]+/g)) {
      if (part) requested.add(part);
    }
  }

  for (const arg of process.argv.slice(2)) {
    if (!arg || arg.startsWith("-")) continue;
    requested.add(arg);
  }

  return requested;
}

async function main() {
  const failures: { name: string; error: unknown }[] = [];
  const requested = parseRequestedCases();

  await seedDatabases();

  if (process.argv.slice(2).includes("list")) {
    for (const c of cases) process.stdout.write(`${c.name}\n`);
    return;
  }

  const selectedCases =
    requested.size > 0 ? cases.filter((c) => requested.has(c.name)) : cases;

  if (requested.size > 0 && selectedCases.length === 0) {
    throw new Error(
      `未匹配到任何用例: ${Array.from(requested).join(", ")}（可用：${cases
        .map((c) => c.name)
        .join(", ")}）`
    );
  }

  for (const testCase of selectedCases) {
    const start = Date.now();
    try {
      await testCase.run(goldenBase, candidateBase);
      const ms = Date.now() - start;
      process.stdout.write(`PASS ${testCase.name} (${ms}ms)\n`);
    } catch (error) {
      const ms = Date.now() - start;
      process.stdout.write(`FAIL ${testCase.name} (${ms}ms)\n`);
      failures.push({ name: testCase.name, error });
    }
  }

  if (failures.length > 0) {
    for (const f of failures) {
      process.stderr.write(`\n[${f.name}] ${String((f.error as any)?.message ?? f.error)}\n`);
    }
    process.exitCode = 1;
  }
}

main().catch((e) => {
  process.stderr.write(String(e) + "\n");
  process.exit(1);
});
