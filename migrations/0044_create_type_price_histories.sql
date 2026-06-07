CREATE TABLE IF NOT EXISTS type_price_histories (
  type_id INTEGER NOT NULL,
  date    TEXT    NOT NULL,
  open    REAL    NOT NULL,
  high    REAL    NOT NULL,
  low     REAL    NOT NULL,
  close   REAL    NOT NULL,
  PRIMARY KEY (type_id, date)
);
