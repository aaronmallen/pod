CREATE TABLE IF NOT EXISTS captains_log_prompt_config (
  id         INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
  version    INTEGER NOT NULL,
  document   TEXT    NOT NULL,
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);

INSERT INTO captains_log_prompt_config (id, version, document, created_at, updated_at)
VALUES (
  1,
  2,
  '{"version":2,"sections":[{"id":"core","kind":"free","label":"","i18n_key":"captains_log.wizard.group_core","questions":[{"id":"goal","kind":"text","label":"","i18n_key":"captains_log.wizard.goal_label","placeholder":"","required":true},{"id":"remember","kind":"text","label":"","i18n_key":"captains_log.wizard.remember_label","placeholder":"","required":false},{"id":"blocked","kind":"text","label":"","i18n_key":"captains_log.wizard.blocked_label","placeholder":"","required":false}]},{"id":"conditional","kind":"conditional","label":"","i18n_key":"captains_log.wizard.group_conditional","triggers":{"combat":true,"build":true,"skill":true}},{"id":"forward","kind":"free","label":"","i18n_key":"captains_log.wizard.group_forward","questions":[{"id":"next","kind":"text","label":"","i18n_key":"captains_log.wizard.next_label","placeholder":"","required":false},{"id":"research","kind":"text","label":"","i18n_key":"captains_log.wizard.research_label","placeholder":"","required":false}]}]}',
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS captains_log_answer (
  date        TEXT NOT NULL,
  question_id TEXT NOT NULL,
  value       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  PRIMARY KEY (date, question_id)
);

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'goal', goal, created_at, updated_at FROM captains_log WHERE goal IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'remember', remember, created_at, updated_at FROM captains_log WHERE remember IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'blocked', blocked, created_at, updated_at FROM captains_log WHERE blocked IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'combat', combat, created_at, updated_at FROM captains_log WHERE combat IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'build', build, created_at, updated_at FROM captains_log WHERE build IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'skill', skill, created_at, updated_at FROM captains_log WHERE skill IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'next', next, created_at, updated_at FROM captains_log WHERE next IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;

INSERT INTO captains_log_answer (date, question_id, value, created_at, updated_at)
SELECT date, 'research', research, created_at, updated_at FROM captains_log WHERE research IS NOT NULL
ON CONFLICT (date, question_id) DO NOTHING;
