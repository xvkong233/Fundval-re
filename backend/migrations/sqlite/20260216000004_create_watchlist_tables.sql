CREATE TABLE IF NOT EXISTS watchlist (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES auth_user(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT watchlist_user_name_unique UNIQUE (user_id, name)
);

CREATE INDEX IF NOT EXISTS watchlist_user_id_idx ON watchlist(user_id);

CREATE TABLE IF NOT EXISTS watchlist_item (
  id TEXT PRIMARY KEY,
  watchlist_id TEXT NOT NULL REFERENCES watchlist(id) ON DELETE CASCADE,
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,
  "order" INTEGER NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT watchlist_item_watchlist_fund_unique UNIQUE (watchlist_id, fund_id)
);

CREATE INDEX IF NOT EXISTS watchlist_item_watchlist_id_idx ON watchlist_item(watchlist_id);
CREATE INDEX IF NOT EXISTS watchlist_item_fund_id_idx ON watchlist_item(fund_id);

