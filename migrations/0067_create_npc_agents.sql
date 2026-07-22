CREATE TABLE IF NOT EXISTS agent_types (
  id   INTEGER PRIMARY KEY NOT NULL,
  name TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS npc_corporation_divisions (
  id   INTEGER PRIMARY KEY NOT NULL,
  name TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS npc_agents (
  id              INTEGER PRIMARY KEY NOT NULL,
  -- DEFERRABLE: the seed upserts agents and their parent corporations/stations/types in catalog order that is not guaranteed to satisfy these references row-by-row; defer the FKs to the end of the transaction.
  agent_type_id   INTEGER REFERENCES agent_types(id) DEFERRABLE INITIALLY DEFERRED,
  corporation_id  INTEGER REFERENCES corporations(id) DEFERRABLE INITIALLY DEFERRED,
  division_id     INTEGER REFERENCES npc_corporation_divisions(id) DEFERRABLE INITIALLY DEFERRED,
  location_id     INTEGER REFERENCES stations(id) DEFERRABLE INITIALLY DEFERRED,
  is_locator      INTEGER NOT NULL DEFAULT 0,
  level           INTEGER,
  name            TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS npc_agent_skills (
  agent_id      INTEGER NOT NULL REFERENCES npc_agents(id) ON DELETE CASCADE,
  skill_type_id INTEGER NOT NULL REFERENCES item_types(id) DEFERRABLE INITIALLY DEFERRED,
  PRIMARY KEY (agent_id, skill_type_id)
);

CREATE INDEX IF NOT EXISTS idx_npc_agents_agent_type_id      ON npc_agents(agent_type_id);
CREATE INDEX IF NOT EXISTS idx_npc_agents_corporation_id     ON npc_agents(corporation_id);
CREATE INDEX IF NOT EXISTS idx_npc_agents_division_id        ON npc_agents(division_id);
CREATE INDEX IF NOT EXISTS idx_npc_agents_level              ON npc_agents(level);
CREATE INDEX IF NOT EXISTS idx_npc_agents_location_id        ON npc_agents(location_id);
CREATE INDEX IF NOT EXISTS idx_npc_agent_skills_agent_id     ON npc_agent_skills(agent_id);
CREATE INDEX IF NOT EXISTS idx_npc_agent_skills_skill_type_id ON npc_agent_skills(skill_type_id);
