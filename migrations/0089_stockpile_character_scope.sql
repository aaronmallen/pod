ALTER TABLE stockpiles ADD COLUMN character_scope TEXT;

UPDATE stockpiles
SET character_scope = 'name:"' || (SELECT name FROM characters WHERE characters.id = stockpiles.character_id) || '"'
WHERE character_id IS NOT NULL
  AND EXISTS (SELECT 1 FROM characters WHERE characters.id = stockpiles.character_id);

DROP INDEX IF EXISTS idx_stockpiles_character_id;

ALTER TABLE stockpiles DROP COLUMN character_id;
