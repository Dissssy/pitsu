-- CREATE TABLE Users (
--     uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
--     username TEXT NOT NULL UNIQUE,
--     api_key TEXT NOT NULL UNIQUE DEFAULT gen_random_uuid(),
--     created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
-- );

--! create
INSERT INTO Users (username)
    VALUES (:username);

--! delete_by_uuid
DELETE FROM Users
    WHERE uuid = :uuid
    RETURNING *;

--! get_by_uuid
SELECT * FROM Users
    WHERE uuid = :uuid;

--! get_by_username
SELECT * FROM Users
    WHERE username = :username;

--! get_by_api_key
SELECT * FROM Users
    WHERE api_key = :api_key;

--! get_all
SELECT * FROM Users
    ORDER BY created_at DESC;