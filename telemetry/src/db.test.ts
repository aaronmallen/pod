// db.ts mapping tests. A tiny D1 stub records the prepared SQL + bound params so
// we can assert the envelope -> rows mapping (§6.2) without a real database.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { validateEnvelope, type Envelope } from "./contract";
import { buildStatements } from "./db";

interface Recorded {
  sql: string;
  params: unknown[];
}

function stubDb(sink: Recorded[]) {
  return {
    prepare(sql: string) {
      return {
        bind(...params: unknown[]) {
          const rec: Recorded = { sql, params };
          sink.push(rec);
          return rec;
        },
      };
    },
  } as unknown as Parameters<typeof buildStatements>[0];
}

function fixtureEnvelope(name: string): Envelope {
  const path = fileURLToPath(new URL(`../../test/fixtures/telemetry/${name}`, import.meta.url));
  const r = validateEnvelope(JSON.parse(readFileSync(path, "utf8")), 1);
  if (!r.ok) throw new Error(`fixture ${name} did not validate: ${r.reason}`);
  return r.envelope;
}

describe("session envelope mapping", () => {
  it("emits usage + performance + one environment row, no IP", () => {
    const sink: Recorded[] = [];
    buildStatements(stubDb(sink), fixtureEnvelope("session_all_streams.json"));

    const streams = sink
      .map((r) => r.params[5]) // 6th bound param is `stream`
      .filter((s) => typeof s === "string");
    // 2 usage events + 1 perf view + 1 environment row.
    expect(streams).toEqual(["usage", "usage", "performance", "environment"]);

    // feature_toggle row carries toggle_on = 1; view_open carries null.
    const usageRows = sink.filter((r) => r.params[5] === "usage");
    expect(usageRows[0].params[8]).toBeNull(); // view_open -> toggle_on null
    expect(usageRows[1].params[8]).toBe(1); // feature_toggle on:true -> 1

    // No bound value mentions an IP-ish header anywhere.
    for (const r of sink) {
      expect(JSON.stringify(r.params)).not.toMatch(/connecting-ip|request\.cf/i);
    }
  });
});

describe("crash envelope mapping", () => {
  it("emits one crashes row with JSON-serialized arrays", () => {
    const sink: Recorded[] = [];
    buildStatements(stubDb(sink), fixtureEnvelope("crash_batch.json"));
    expect(sink).toHaveLength(1);
    expect(sink[0].sql).toMatch(/INSERT INTO crashes/);
    // backtrace (param index 7) is a JSON array string.
    expect(JSON.parse(sink[0].params[7] as string)).toBeInstanceOf(Array);
    // crashed_at is the report's timestamp, not wall-clock-now.
    expect(sink[0].params[9]).toBe("2026-06-24T22:14:03Z");
  });
});
