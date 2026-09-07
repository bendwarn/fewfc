-- Better Auth 1.7.3 不再寫入 issuer。重建 SQLite 資料表，保留歷史欄位資料，
-- 同時允許新資料列省略它。

CREATE TABLE account_with_provider_key (
  id text PRIMARY KEY NOT NULL,
  issuer text,
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

-- 在複製前建立唯一性防護；舊資料若有重複 identity，遷移會在原表仍完整
-- 時失敗，不會先刪除原有帳號資料。
CREATE UNIQUE INDEX account_provider_account_idx ON account_with_provider_key (provider_id, account_id);

INSERT INTO account_with_provider_key (
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
FROM account;

DROP INDEX IF EXISTS account_issuer_account_idx;
DROP TABLE account;
ALTER TABLE account_with_provider_key RENAME TO account;

CREATE INDEX account_user_id_idx ON account (user_id);
