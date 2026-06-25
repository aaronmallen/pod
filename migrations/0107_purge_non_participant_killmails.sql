-- One-time purge of character_killmails rows the owning character was never on,
-- left behind before the participant guard in the character killmails sync job.
-- During an ESI outage the zKill character fallback could answer with a global
-- "recent kills" firehose, and every mail in it was filed under the syncing
-- character with no check that the character actually participated.
--
-- A row is removed only when the character is neither the victim nor a
-- participating attacker AND the killmail has attacker detail to judge against;
-- rows with no killmail_attackers detail at all are conservatively retained
-- (we cannot prove non-participation without it). The three tables have no FK
-- cascade between them, so child rows are deleted explicitly. The purge set is
-- materialised in a temp table first so deleting attacker child rows cannot
-- shift the predicate mid-run. Re-running is a no-op once clean.

CREATE TEMP TABLE _killmail_purge AS
SELECT ck.character_id AS character_id, ck.killmail_id AS killmail_id
FROM character_killmails ck
WHERE ck.victim_id IS NOT ck.character_id
  AND EXISTS (
    SELECT 1 FROM killmail_attackers ka
    WHERE ka.character_id = ck.character_id
      AND ka.killmail_id = ck.killmail_id
  )
  AND NOT EXISTS (
    SELECT 1 FROM killmail_attackers ka
    WHERE ka.character_id = ck.character_id
      AND ka.killmail_id = ck.killmail_id
      AND ka.attacker_character_id = ck.character_id
  );

DELETE FROM killmail_items
WHERE (character_id, killmail_id) IN (
  SELECT character_id, killmail_id FROM _killmail_purge
);

DELETE FROM killmail_attackers
WHERE (character_id, killmail_id) IN (
  SELECT character_id, killmail_id FROM _killmail_purge
);

DELETE FROM character_killmails
WHERE (character_id, killmail_id) IN (
  SELECT character_id, killmail_id FROM _killmail_purge
);

DROP TABLE _killmail_purge;
