CREATE INDEX IF NOT EXISTS idx_character_assets_location_id   ON character_assets(location_id, type_id);
CREATE INDEX IF NOT EXISTS idx_corporation_assets_location_id ON corporation_assets(location_id, type_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_char_tx ON character_wallet_transaction(character_id, transaction_id);
