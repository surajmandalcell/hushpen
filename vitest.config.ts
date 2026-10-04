import { existsSync, mkdirSync } from "node:fs";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Keep test temp files on the data drive, not the system drive.
const dataDriveTmp = "/Volumes/External1TB/data/_custom/asrpro-mission/tmp";
if (existsSync("/Volumes/External1TB") && !process.env.TMPDIR?.startsWith("/Volumes/External1TB")) {
  mkdirSync(dataDriveTmp, { recursive: true });
  process.env.TMPDIR = dataDriveTmp;
}

// node:sqlite emits an ExperimentalWarning; suppress it only in the node and
// integration projects, which are the only ones allowed to import it.
const sqliteWarningSuppression = ["--disable-warning=ExperimentalWarning"];

export default defineConfig({
  plugins: [react()],
  test: {
    testTimeout: 10000,
    maxWorkers: 6,
    projects: [
      {
        extends: true,
        test: {
          name: "node",
          environment: "node",
          include: [
            "electron/**/*.test.{ts,cjs}",
            "shared/**/*.test.ts",
            "tests/unit/**/*.test.ts",
          ],
          execArgv: sqliteWarningSuppression,
        },
      },
      {
        extends: true,
        test: {
          name: "dom",
          environment: "jsdom",
          include: ["src/**/*.test.{ts,tsx}"],
          setupFiles: ["src/test/setup.ts"],
        },
      },
      {
        extends: true,
        test: {
          name: "integration",
          environment: "node",
          include: ["tests/integration/**/*.test.ts"],
          testTimeout: 120_000,
          execArgv: sqliteWarningSuppression,
        },
      },
    ],
  },
});
