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
  const path = fileURLToPath(new URL(`../../desktop/tests/fixtures/telemetry/${name}`, import.meta.url));
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

describe("environment row mapping", () => {
  function envEnvelope(environment: Record<string, unknown>): Envelope {
    return {
      schema: 1,
      kind: "session",
      id: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
      session: "s_1a2b3c4d",
      app: { version: "0.9.4" },
      sent_at: "2026-06-25T14:32:08Z",
      streams: { environment: environment as unknown as Envelope["streams"]["environment"] },
    };
  }

  // env-row bound params: ... os(12), os_version(13), arch(14), window_size(15),
  // screen_size(16), locale(17), app_language(18), event_at(19).
  it("binds window_size + screen_size from a new payload", () => {
    const sink: Recorded[] = [];
    buildStatements(
      stubDb(sink),
      envEnvelope({
        os: "macos",
        os_version: "15",
        arch: "aarch64",
        window_size: "2560x1440",
        screen_size: "3440x1440",
        locale: "en",
      }),
    );

    expect(sink).toHaveLength(1);
    expect(sink[0].params[15]).toBe("2560x1440");
    expect(sink[0].params[16]).toBe("3440x1440");
  });

  it("coalesces a legacy display into window_size and nulls screen_size", () => {
    const sink: Recorded[] = [];
    buildStatements(
      stubDb(sink),
      envEnvelope({
        os: "macos",
        os_version: "15",
        arch: "aarch64",
        display: "2560x1440",
        locale: "en",
      }),
    );

    expect(sink[0].params[15]).toBe("2560x1440");
    expect(sink[0].params[16]).toBeNull();
  });

  it("binds app_language from a new payload", () => {
    const sink: Recorded[] = [];
    buildStatements(
      stubDb(sink),
      envEnvelope({
        os: "macos",
        os_version: "15",
        arch: "aarch64",
        window_size: "2560x1440",
        screen_size: "3440x1440",
        locale: "en",
        app_language: "de",
      }),
    );

    expect(sink[0].params[18]).toBe("de");
  });

  it("nulls app_language when an old client omits it", () => {
    const sink: Recorded[] = [];
    buildStatements(
      stubDb(sink),
      envEnvelope({
        os: "macos",
        os_version: "15",
        arch: "aarch64",
        window_size: "2560x1440",
        screen_size: "3440x1440",
        locale: "en",
      }),
    );

    expect(sink[0].params[18]).toBeNull();
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
