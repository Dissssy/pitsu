-- CREATE TABLE Files (
--     uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
--     repository_uuid UUID NOT NULL REFERENCES Repositories(uuid) ON DELETE CASCADE,
--     file_path TEXT NOT NULL,
--     aws_s3_object_key TEXT NOT NULL,
--     created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
-- );

--! create
INSERT INTO Files (repository_uuid, file_path, aws_s3_object_key)
    VALUES (:repository_uuid, :file_path, :aws_s3_object_key)
    RETURNING *;

--! get
SELECT * FROM Files WHERE file_path = :file_path AND repository_uuid = :repository_uuid;

--! get_all_in
SELECT * FROM Files WHERE repository_uuid = :repository_uuid;

--! get_all
SELECT * FROM Files;

--! update_or_create
WITH upsert AS (
    UPDATE Files
    SET aws_s3_object_key = :aws_s3_object_key, updated_at = CURRENT_TIMESTAMP
    WHERE file_path = :file_path AND repository_uuid = :repository_uuid
    RETURNING *
)
INSERT INTO Files (repository_uuid, file_path, aws_s3_object_key)
SELECT :repository_uuid, :file_path, :aws_s3_object_key
WHERE NOT EXISTS (SELECT * FROM upsert);

--! delete
DELETE FROM Files WHERE file_path = :file_path AND repository_uuid = :repository_uuid RETURNING *;