// Worker contract tests. These load the SAME golden fixtures the Rust contract
// crate pins (../../desktop/tests/fixtures/telemetry/*.json), so any drift in the wire shape
// fails both suites.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  crashFreeTextViolation,
  validateEnvelope,
  type Envelope,
} from "./contract";

const MAX_SCHEMA = 1;

function fixture(name: string): unknown {
  const path = fileURLToPath(new URL(`../../desktop/tests/fixtures/telemetry/${name}`, import.meta.url));
  return JSON.parse(readFileSync(path, "utf8"));
}

const SESSION = fixture("session_all_streams.json");
const CRASH = fixture("crash_batch.json");

function deepClone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

describe("golden fixtures validate", () => {
  it("accepts the session_all_streams golden fixture", () => {
    const r = validateEnvelope(SESSION, MAX_SCHEMA);
    expect(r.ok).toBe(true);
  });

  it("accepts the crash_batch golden fixture", () => {
    const r = validateEnvelope(CRASH, MAX_SCHEMA);
    expect(r.ok).toBe(true);
  });

  it("the crash fixture passes the §5.6 free-text reject", () => {
    const r = validateEnvelope(CRASH, MAX_SCHEMA);
    expect(r.ok).toBe(true);
    if (r.ok) expect(crashFreeTextViolation(r.envelope)).toBeNull();
  });
});

describe("tampered envelopes are rejected", () => {
  it("rejects an unknown top-level key", () => {
    const bad = deepClone(SESSION) as Record<string, unknown>;
    bad.extra = true;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects an unknown stream key", () => {
    const bad = deepClone(SESSION) as { streams: Record<string, unknown> };
    bad.streams.bogus = {};
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects crashes inside a session batch", () => {
    const bad = deepClone(SESSION) as { streams: Record<string, unknown> };
    bad.streams.crashes = { reports: [] };
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects a null stream value", () => {
    const bad = deepClone(SESSION) as { streams: Record<string, unknown> };
    bad.streams.usage = null;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects a bad anon id", () => {
    const bad = deepClone(SESSION) as Record<string, unknown>;
    bad.id = "NOTHEX";
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects a bad session tag", () => {
    const bad = deepClone(CRASH) as Record<string, unknown>;
    bad.session = "s_PREV9f2a"; // uppercase + non-hex (the original blocker)
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects schema above MAX_SCHEMA", () => {
    const bad = deepClone(SESSION) as Record<string, unknown>;
    bad.schema = 2;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects a usage kind not in the allow-list", () => {
    const bad = deepClone(SESSION) as { streams: { usage: { events: Record<string, unknown>[] } } };
    bad.streams.usage.events[0].kind = "view_close";
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects feature_toggle without on", () => {
    const bad = deepClone(SESSION) as { streams: { usage: { events: Record<string, unknown>[] } } };
    delete bad.streams.usage.events[1].on;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects on present on a non-feature_toggle event", () => {
    const bad = deepClone(SESSION) as { streams: { usage: { events: Record<string, unknown>[] } } };
    bad.streams.usage.events[0].on = true;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects a non-integer load_ms", () => {
    const bad = deepClone(SESSION) as { streams: { performance: { views: Record<string, unknown>[] } } };
    bad.streams.performance.views[0].load_ms = 1.5;
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });

  it("rejects pod_version in environment (§5.3)", () => {
    const bad = deepClone(SESSION) as { streams: { environment: Record<string, unknown> } };
    bad.streams.environment.pod_version = "0.9.4";
    expect(validateEnvelope(bad, MAX_SCHEMA).ok).toBe(false);
  });
});

describe("environment window_size / screen_size / app_language contract", () => {
  function withEnvironment(environment: Record<string, unknown>) {
    const env = deepClone(SESSION) as { streams: { environment: Record<string, unknown> } };
    env.streams.environment = environment;
    return validateEnvelope(env, MAX_SCHEMA);
  }

  it("accepts a new payload with window_size + screen_size", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      screen_size: "3440x1440",
      locale: "en",
    });
    expect(r.ok).toBe(true);
  });

  it("accepts a legacy payload using display (aliased to window_size)", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      display: "2560x1440",
      locale: "en",
    });
    expect(r.ok).toBe(true);
  });

  it("accepts a payload that omits screen_size", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      locale: "en",
    });
    expect(r.ok).toBe(true);
  });

  it("rejects a non-string screen_size", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      screen_size: 1440,
      locale: "en",
    });
    expect(r.ok).toBe(false);
  });

  it("rejects an environment missing window_size and its display alias", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      locale: "en",
    });
    expect(r.ok).toBe(false);
  });

  it("accepts a payload carrying app_language", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      screen_size: "3440x1440",
      locale: "en",
      app_language: "de",
    });
    expect(r.ok).toBe(true);
  });

  it("accepts a payload that omits app_language", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      screen_size: "3440x1440",
      locale: "en",
    });
    expect(r.ok).toBe(true);
  });

  it("rejects a non-string app_language", () => {
    const r = withEnvironment({
      os: "macos",
      os_version: "15",
      arch: "aarch64",
      window_size: "2560x1440",
      screen_size: "3440x1440",
      locale: "en",
      app_language: 42,
    });
    expect(r.ok).toBe(false);
  });
});

describe("§5.6 crash free-text reject", () => {
  function crashWith(message: string): Envelope {
    const env = deepClone(CRASH) as Envelope;
    env.streams.crashes!.reports[0].message = message;
    env.streams.crashes!.reports[0].backtrace = undefined;
    env.streams.crashes!.reports[0].context_log = undefined;
    return env;
  }

  it("rejects an email", () => {
    expect(crashFreeTextViolation(crashWith("contact me@example.com"))).not.toBeNull();
  });

  it("rejects an absolute home path", () => {
    expect(crashFreeTextViolation(crashWith("at /Users/alice/secret"))).not.toBeNull();
  });

  it("rejects a URL with a path", () => {
    expect(crashFreeTextViolation(crashWith("see https://evil.test/leak"))).not.toBeNull();
  });

  it("rejects a 7+ digit run", () => {
    expect(crashFreeTextViolation(crashWith("entity 12345678 exploded"))).not.toBeNull();
  });

  it("accepts a clean message", () => {
    expect(crashFreeTextViolation(crashWith("clean panic, no PII"))).toBeNull();
  });
});
