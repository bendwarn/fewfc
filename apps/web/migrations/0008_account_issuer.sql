CREATE TABLE account_with_issuer (
  id text PRIMARY KEY NOT NULL,
  issuer text NOT NULL,
  account_id text NOT NULL,
  provider_id text NOT NULL,
  user_id text NOT NULL,
  access_token text,
  refresh_token text,
  id_token text,
  access_token_expires_at integer,
  refresh_token_expires_at integer,
  scope text,
  password text,
  created_at integer NOT NULL,
  updated_at integer NOT NULL,
  FOREIGN KEY (user_id) REFERENCES user(id) ON UPDATE no action ON DELETE cascade
);

-- 本專案目前唯一會建立 account 記錄的登入方式是本機帳密；若資料庫出現
-- 未知 provider，讓 NOT NULL 約束中止遷移，避免替外部身分寫入不可信的 issuer。
INSERT INTO account_with_issuer (
  id,
  issuer,
  account_id,
  provider_id,
  user_id,
  access_token,
  refresh_token,
  id_token,
  access_token_expires_at,
  refresh_token_expires_at,
  scope,
  password,
  created_at,
  updated_at
)
SELECT
  id,
  CASE provider_id WHEN 'credential' THEN 'local:credential' END,
  CASE provider_id WHEN 'credential' THEN user_id END,
  provider_id,
  user_id,
  access_token,
  refresh_token,
  id_token,
  access_token_expires_at,
  refresh_token_expires_at,
  scope,
  password,
  created_at,
  updated_at
FROM account;

DROP TABLE account;
ALTER TABLE account_with_issuer RENAME TO account;

CREATE INDEX account_user_id_idx ON account (user_id);
CREATE UNIQUE INDEX account_issuer_account_idx ON account (issuer, account_id);
