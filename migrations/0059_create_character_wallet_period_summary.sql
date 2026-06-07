CREATE VIEW IF NOT EXISTS character_wallet_period_summary AS
SELECT
  character_id,
  substr(date, 1, 7)                                          AS period,
  SUM(CASE WHEN amount >= 0 THEN amount ELSE 0.0 END)         AS income,
  SUM(CASE WHEN amount <  0 THEN -amount ELSE 0.0 END)        AS spend,
  SUM(CASE WHEN amount >= 0 THEN amount ELSE 0.0 END)
    - SUM(CASE WHEN amount < 0 THEN -amount ELSE 0.0 END)     AS net
FROM character_wallet_journal
WHERE date IS NOT NULL AND length(date) >= 7
GROUP BY character_id, substr(date, 1, 7);
