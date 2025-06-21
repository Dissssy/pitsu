-- CREATE TABLE Repositories (
--     uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
--     name TEXT NOT NULL,
--     owner_uuid UUID NOT NULL REFERENCES Users(uuid) ON DELETE CASCADE,
--     file_hashes JSONB NOT NULL DEFAULT '{}', -- Updated when someone with write access pushes changes
--     created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
-- );

--! create
INSERT INTO Repositories (name, owner_uuid)
    VALUES (:name, :owner_uuid);

--! delete_by_uuid
DELETE FROM Repositories
    WHERE uuid = :uuid
    RETURNING *;

--! get_by_uuid
SELECT * FROM Repositories
    WHERE uuid = :uuid;

--! get_by_name_and_owner
SELECT * FROM Repositories
    WHERE name = :name AND owner_uuid = :owner_uuid;

--! get_by_owner
SELECT * FROM Repositories
    WHERE owner_uuid = :owner_uuid
    ORDER BY created_at DESC;

--! get_all
SELECT * FROM Repositories
    ORDER BY created_at DESC;

--! update_file_hashes_by_uuid
UPDATE Repositories
    SET file_hashes = :file_hashes
    WHERE uuid = :uuid
    RETURNING *;

--! update_metadata_by_uuid
UPDATE Repositories
    SET name = :name, updated_at = CURRENT_TIMESTAMP
    WHERE uuid = :uuid
    RETURNING *;