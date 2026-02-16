import { runHealth } from "./cases/health.js";
import { runBootstrap } from "./cases/bootstrap.js";

type Case = {
  name: string;
  run: (goldenBase: string, candidateBase: string) => Promise<void>;
};

const goldenBase = process.env.GOLDEN_BASE ?? "http://localhost:8000";
const candidateBase = process.env.CANDIDATE_BASE ?? "http://localhost:8001";

const cases: Case[] = [
  { name: "health", run: runHealth },
  { name: "bootstrap", run: runBootstrap },
];

async function main() {
  const failures: { name: string; error: unknown }[] = [];

  for (const testCase of cases) {
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
