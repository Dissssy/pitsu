-- CREATE TABLE Access (
--     uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
--     repository_uuid UUID NOT NULL REFERENCES Repositories(uuid) ON DELETE CASCADE,
--     user_uuid UUID NOT NULL REFERENCES Users(uuid) ON DELETE CASCADE,
--     access_level TEXT NOT NULL CHECK (access_level IN ('R', 'RW', 'RW+')),
--     created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
--     updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
-- );

--! create
INSERT INTO Access (repository_uuid, user_uuid, access_level)
    VALUES (:repository_uuid, :user_uuid, :access_level);

--! delete_by_user_uuid_and_repository_uuid
DELETE FROM Access
    WHERE user_uuid = :user_uuid AND repository_uuid = :repository_uuid
    RETURNING *;

-- If the user is the owner listed in the Repositories table, they have "RW+" access, otherwise check Access table and return the access level, "N" if no access
--! user_has_access
SELECT
    CASE
        WHEN r.owner_uuid = :user_uuid THEN 'RW+'
        ELSE COALESCE(a.access_level, 'N')
    END AS access_level
FROM Repositories r
LEFT JOIN Access a ON r.uuid = a.repository_uuid AND a.user_uuid = :user_uuid
WHERE r.uuid = :repository_uuid;

--! get_by_user
SELECT * FROM Access WHERE user_uuid = :user_uuid;

-- Return the Owner's UUID with "RW+" access alongside all users with access to the repository, including their access level
-- Also get the usernames from the Users table for better context
--! get_all_users_with_access
SELECT * FROM (
    SELECT
        r.owner_uuid AS user_uuid,
        'RW+' AS access_level,
        u.username
    FROM Repositories r
    JOIN Users u ON r.owner_uuid = u.uuid
    WHERE r.uuid = :repository_uuid

    UNION ALL

    SELECT
        a.user_uuid,
        a.access_level,
        u.username
    FROM Access a
    JOIN Users u ON a.user_uuid = u.uuid
    WHERE a.repository_uuid = :repository_uuid
) AS access_info;